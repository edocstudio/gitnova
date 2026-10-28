//! Explicitly previewed parent branch switching; submodule checkouts stay in place.
use crate::{path_from_bytes, working::run_write, GitCli, StatusReader, WorkingTreeReader};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};
pub trait BranchReader {
    fn local_branches(&self, path: &Path) -> Result<Vec<LocalBranch>>;
    fn preview_branch_switch(&self, path: &Path, branch: &str) -> Result<BranchSwitchPreview>;
}
pub trait BranchWriter {
    fn switch_branch(&self, path: &Path, branch: &str, token: &str) -> Result<()>;
}
impl BranchReader for GitCli {
    fn local_branches(&self, path: &Path) -> Result<Vec<LocalBranch>> {
        let raw = self
            .read(
                path,
                &[
                    OsStr::new("for-each-ref"),
                    OsStr::new("--format=%(refname)%00%(objectname)%00%(worktreepath)%00"),
                    OsStr::new("refs/heads/"),
                ],
                4 * 1024 * 1024,
            )?
            .bytes;
        let current = self.optional_text(path, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        let mut fields: Vec<_> = raw.split(|b| *b == 0).collect();
        if matches!(fields.last(),Some(last) if *last==b"\n" || last.is_empty()) {
            fields.pop();
        }
        if fields.is_empty() {
            return Ok(vec![]);
        }
        if !fields.len().is_multiple_of(3) {
            return Err(AppError::new("git_parse", "Invalid local branch record"));
        }
        fields
            .as_chunks::<3>()
            .0
            .iter()
            .map(|record| {
                let name = std::str::from_utf8(record[0].strip_prefix(b"\n").unwrap_or(record[0]))
                    .map_err(|_| AppError::new("encoding", "Branch name is not UTF-8"))?
                    .strip_prefix("refs/heads/")
                    .ok_or_else(|| AppError::new("git_parse", "Unexpected ref namespace"))?
                    .to_owned();
                let oid = std::str::from_utf8(record[1])
                    .map_err(|_| AppError::new("git_parse", "Invalid branch OID"))?
                    .to_owned();
                crate::validate_oid(&oid)?;
                Ok(LocalBranch {
                    current: current.as_ref() == Some(&name),
                    name,
                    oid,
                    checked_out_at: if record[2].is_empty() {
                        None
                    } else {
                        Some(NativePath::new(&path_from_bytes(record[2].to_vec())?))
                    },
                })
            })
            .collect()
    }
    fn preview_branch_switch(&self, path: &Path, branch: &str) -> Result<BranchSwitchPreview> {
        let branches = self.local_branches(path)?;
        let target=branches.iter().find(|b|b.name==branch).ok_or_else(||AppError::new("branch","Choose an existing local branch; remote guessing and revision expressions are not supported"))?;
        self.branch_target_preview(path, target, false)
    }
}
impl GitCli {
    pub(crate) fn branch_target_preview(
        &self,
        path: &Path,
        target: &LocalBranch,
        allow_current: bool,
    ) -> Result<BranchSwitchPreview> {
        self.branch_target_preview_with_preservation(path, target, allow_current, false)
    }
    pub(crate) fn branch_target_preview_with_preservation(
        &self,
        path: &Path,
        target: &LocalBranch,
        allow_current: bool,
        preserves_detached: bool,
    ) -> Result<BranchSwitchPreview> {
        let branch = &target.name;
        let branches = self.local_branches(path)?;
        let status = self.status(path)?;
        let (discovery, state) = self.write_state(path)?;
        let detached_retained = if !preserves_detached && status.branch.is_none() {
            if let Some(head) = &status.head {
                !self
                    .text(
                        path,
                        &[
                            "for-each-ref",
                            &format!("--contains={head}"),
                            "--format=%(refname)",
                            "refs/heads/",
                        ],
                    )?
                    .trim()
                    .is_empty()
            } else {
                false
            }
        } else {
            true
        };
        let mut blocked_reason = if status.dirty && !self.only_clean_gitlink_mismatches(path)? {
            Some("Commit or preserve your uncommitted changes with Git before switching. Nothing will be discarded or stashed automatically.".into())
        } else if target.current && !allow_current {
            Some("This branch is already checked out.".into())
        } else if let Some(worktree) = &target.checked_out_at {
            Some(format!(
                "This branch is in use by another worktree: {}",
                worktree.display
            ))
        } else if status.head.is_none() {
            Some("Create the first commit before switching branches.".into())
        } else if !detached_retained {
            Some("Preserve detached HEAD on a local branch with Git before switching away.".into())
        } else {
            None
        };
        if discovery.git_dir.join("index.lock").exists()
            || discovery.git_dir.join("HEAD.lock").exists()
        {
            blocked_reason = Some(
                "Another Git operation holds an index or HEAD lock. GitNova will not remove it."
                    .into(),
            );
        }
        let entries = self
            .read(
                path,
                &[OsStr::new("ls-files"), OsStr::new("-v"), OsStr::new("-z")],
                4 * 1024 * 1024,
            )?
            .bytes;
        if entries
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .any(|entry| entry[0].is_ascii_lowercase() || entry[0] == b'S')
        {
            blocked_reason=Some("Review assume-unchanged or skip-worktree entries with Git before switching. Hidden working-file edits cannot be safely assessed here.".into());
        }
        let mut files = vec![];
        let mut submodules = vec![];
        if let Some(source) = &status.head {
            let raw = self
                .read(
                    path,
                    &[
                        OsStr::new("diff"),
                        OsStr::new("--raw"),
                        OsStr::new("--no-abbrev"),
                        OsStr::new("--no-ext-diff"),
                        OsStr::new("--no-textconv"),
                        OsStr::new("--no-renames"),
                        OsStr::new("-z"),
                        OsStr::new(source),
                        OsStr::new(&target.oid),
                        OsStr::new("--"),
                    ],
                    4 * 1024 * 1024,
                )?
                .bytes;
            let mut records = raw.split(|b| *b == 0).filter(|r| !r.is_empty());
            while let Some(header) = records.next() {
                let name = records
                    .next()
                    .ok_or_else(|| AppError::new("git_parse", "Missing branch diff path"))?;
                let header = std::str::from_utf8(header)
                    .map_err(|_| AppError::new("git_parse", "Invalid branch diff header"))?;
                let parts: Vec<_> = header.split_whitespace().collect();
                if parts.len() != 5 {
                    return Err(AppError::new("git_parse", "Invalid branch diff record"));
                }
                if parts[0] == ":160000" || parts[1] == "160000" {
                    if parts[0] != ":160000" || parts[1] != "160000" {
                        blocked_reason=Some("This branch adds, removes or replaces a submodule. Review that repository layout change with Git before switching.".into());
                    } else {
                        let relative = path_from_bytes(name.to_vec())?;
                        let child = path.join(&relative);
                        let child_status = if child.join(".git").exists() {
                            Some(self.status(&child)?)
                        } else {
                            None
                        };
                        if child_status.as_ref().is_some_and(|s| s.dirty) {
                            blocked_reason = Some(format!("Submodule {} contains uncommitted work. Preserve it before switching.", relative.display()));
                        }
                        submodules.push(BranchSubmoduleChange {
                            path: NativePath::new(&relative),
                            source_oid: parts[2].into(),
                            target_oid: parts[3].into(),
                            checked_out_oid: child_status.as_ref().and_then(|s| s.head.clone()),
                            dirty: child_status.map(|s| s.dirty),
                        });
                    }
                }
                if files.len() >= 2000 {
                    return Err(AppError::new(
                        "safety_budget",
                        "Branch preview exceeds 2,000 changed paths",
                    ));
                }
                files.push(BranchFileChange {
                    path: NativePath::new(&path_from_bytes(name.to_vec())?),
                    status: parts[4].into(),
                });
            }
        }
        let mut hash = Sha256::new();
        hash.update(state);
        hash.update([0]);
        hash.update(branch.as_bytes());
        hash.update([0]);
        hash.update(target.oid.as_bytes());
        for item in &branches {
            hash.update(item.name.as_bytes());
            hash.update([0]);
            hash.update(item.oid.as_bytes());
            if let Some(root) = &item.checked_out_at {
                hash.update(root.hex.as_bytes());
            }
            hash.update([0]);
        }
        for submodule in &submodules {
            hash.update(submodule.path.hex.as_bytes());
            hash.update([0]);
            hash.update(
                submodule
                    .checked_out_oid
                    .as_deref()
                    .unwrap_or("")
                    .as_bytes(),
            );
            hash.update([u8::from(submodule.dirty.unwrap_or(false))]);
        }
        Ok(BranchSwitchPreview {
            branch: branch.clone(),
            source_branch: status.branch,
            source_oid: status.head,
            target_oid: target.oid.clone(),
            files,
            submodules,
            token: format!("{:x}", hash.finalize()),
            blocked_reason,
        })
    }
    fn only_clean_gitlink_mismatches(&self, path: &Path) -> Result<bool> {
        let changes = self.working_changes(path)?;
        let raw = self
            .read(
                path,
                &[
                    OsStr::new("ls-files"),
                    OsStr::new("--stage"),
                    OsStr::new("-z"),
                ],
                4 * 1024 * 1024,
            )?
            .bytes;
        let mut links = std::collections::HashSet::new();
        for entry in raw
            .split(|b| *b == 0)
            .filter(|entry| entry.starts_with(b"160000 "))
        {
            if let Some(tab) = entry.iter().position(|b| *b == b'\t') {
                links.insert(path_from_bytes(entry[tab + 1..].to_vec())?);
            }
        }
        for file in changes.files {
            let relative = file.path.to_path()?;
            if file.index_status != " " || file.worktree_status != "M" || !links.contains(&relative)
            {
                return Ok(false);
            }
            let child = path.join(relative);
            if !child.join(".git").exists() || self.status(&child)?.dirty {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
impl BranchWriter for GitCli {
    fn switch_branch(&self, path: &Path, branch: &str, token: &str) -> Result<()> {
        let preview = self.preview_branch_switch(path, branch)?;
        if preview.token != token {
            return Err(AppError::new("stale_preview","HEAD, index, target branch or linked worktree changed. Review the branch switch again."));
        }
        if let Some(reason) = preview.blocked_reason {
            return Err(AppError::new("switch_blocked", reason));
        }
        // Native Git owns its index/ref locks and rejects overwrites. Never opt into
        // force, merge-on-checkout, automatic stash, ignored-file overwrite or recursion.
        let args: Vec<OsString> = [
            "switch",
            "--no-guess",
            "--no-recurse-submodules",
            "--no-overwrite-ignore",
            "--",
            branch,
        ]
        .iter()
        .map(OsString::from)
        .collect();
        run_write(path, &args, None)?;
        let after = self.status(path)?;
        if after.branch.as_deref() != Some(branch)
            || after.head.as_deref() != Some(preview.target_oid.as_str())
        {
            return Err(AppError::new("operation_unknown","Git completed but HEAD differs from the reviewed target. Inspect the repository before retrying; no rollback was attempted."));
        }
        Ok(())
    }
}
