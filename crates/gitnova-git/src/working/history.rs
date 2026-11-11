//! One-level undo/redo of an unpublished tip. Only refs move: the index and
//! checkout are retained byte-for-byte, including for an unborn branch.
use super::{digest, index_bytes, run_git_write};
use crate::{GitCli, RepositoryReader, StatusReader};
use gitnova_domain::{AppError, CommitHistoryPreview, Result};
use std::path::Path;

fn history_slot(git_dir: &Path, branch: &str) -> String {
    let mut key = git_dir.as_os_str().as_encoded_bytes().to_vec();
    key.push(0);
    key.extend_from_slice(branch.as_bytes());
    digest(&key)
}

impl GitCli {
    pub fn preview_commit_history(&self, root: &Path, redo: bool) -> Result<CommitHistoryPreview> {
        // The toolbar probes Redo when the branch changes. An absent slot must
        // return before reading a potentially large index or scanning checkout files.
        let discovery = self.discover(root)?;
        let branch = self
            .optional_text(root, &["symbolic-ref", "--quiet", "HEAD"])?
            .and_then(|reference| reference.strip_prefix("refs/heads/").map(str::to_owned))
            .ok_or_else(|| {
                AppError::new(
                    "history_blocked",
                    "Check out a local branch before Undo or Redo.",
                )
            })?;
        self.valid_name(root, &branch, "heads")?;
        let slot = format!(
            "refs/gitnova/redo/{}",
            history_slot(&discovery.git_dir, &branch)
        );
        if self
            .optional_text(root, &["symbolic-ref", "--quiet", &slot])?
            .is_some()
        {
            return Err(AppError::new(
                "history_blocked",
                "The Redo ref is symbolic. Inspect it with Git.",
            ));
        }
        let saved = self.optional_text(root, &["rev-parse", "--verify", "--quiet", &slot])?;
        if redo && saved.is_none() {
            return Err(AppError::new(
                "history_blocked",
                "No commit is available to Redo on this branch.",
            ));
        }
        let (_, state) = self.commit_state(root)?;
        let status = self.status(root)?;
        if status.branch.as_ref() != Some(&branch) {
            return Err(AppError::new(
                "stale_preview",
                "HEAD or staging changed. Review Undo or Redo again.",
            ));
        }
        let oid = if redo {
            saved.clone()
        } else {
            status.head.clone()
        }
        .ok_or_else(|| {
            AppError::new(
                "history_blocked",
                if redo {
                    "No commit is available to Redo on this branch."
                } else {
                    "There is no commit to Undo."
                },
            )
        })?;
        crate::validate_oid(&oid)?;
        let parents = self.text(
            root,
            &[
                "show",
                "-s",
                "--no-show-signature",
                "--no-notes",
                "--format=%P",
                &oid,
                "--",
            ],
        )?;
        let parent_oid = parents.split_whitespace().next().map(str::to_owned);
        let subject = self
            .text(
                root,
                &[
                    "show",
                    "-s",
                    "--no-show-signature",
                    "--no-notes",
                    "--format=%s",
                    &oid,
                    "--",
                ],
            )?
            .trim_end_matches('\n')
            .to_owned();
        let mut blocked = None;
        if self
            .text(root, &["rev-parse", "--is-shallow-repository"])?
            .trim()
            == "true"
        {
            blocked = Some("Undo and Redo require a complete repository history.".into());
        }
        if discovery.common_dir.join("info/grafts").exists()
            || !self
                .text(
                    root,
                    &["for-each-ref", "--format=%(refname)", "refs/replace/"],
                )?
                .is_empty()
        {
            blocked = Some(
                "Undo and Redo do not rewrite replaced or grafted history. Inspect it with Git."
                    .into(),
            );
        }
        if self.multiple_branch_worktrees(root, &branch)? {
            blocked = Some(
                "This branch is open in multiple worktrees. Close the other checkout first.".into(),
            );
        }
        let referenced = self.text(
            root,
            &[
                "for-each-ref",
                &format!("--contains={oid}"),
                "--format=%(refname)",
                "refs/remotes/",
                "refs/tags/",
            ],
        )?;
        if !redo && !referenced.trim().is_empty() {
            blocked = Some("This commit is referenced by a remote branch or tag. Use Revert to preserve published history.".into());
        }
        // Compare the full index tree and checkout without materializing patches.
        // Untracked files also block: Redo never overwrites new work.
        let unstaged = self.text(
            root,
            &[
                "diff",
                "--name-only",
                "--no-ext-diff",
                "--no-textconv",
                "--ignore-submodules=none",
                "--",
            ],
        )?;
        let untracked = self.text(root, &["ls-files", "--others", "--exclude-standard"])?;
        let staged_matches = self
            .text(
                root,
                &[
                    "diff",
                    "--cached",
                    "--quiet",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--ignore-submodules=none",
                    &oid,
                    "--",
                ],
            )
            .is_ok();
        if !unstaged.is_empty() || !untracked.is_empty() || !staged_matches {
            blocked = Some(if redo {
                "Files or staging have changed. Redo is unavailable; your edits are preserved."
            } else {
                "Commit or stash existing changes before Undo. Your files will not be discarded."
            }.into());
        }
        if redo && status.head != parent_oid {
            blocked = Some(
                "The branch has moved since Undo. Redo is unavailable; your history is preserved."
                    .into(),
            );
        }
        let recovery_ref = format!("refs/gitnova/recovery/undo-{oid}");
        if self
            .optional_text(root, &["symbolic-ref", "--quiet", &recovery_ref])?
            .is_some()
        {
            return Err(AppError::new(
                "history_blocked",
                "The Undo recovery ref is symbolic. Inspect it with Git.",
            ));
        }
        let recovery =
            self.optional_text(root, &["rev-parse", "--verify", "--quiet", &recovery_ref])?;
        if recovery.as_ref().is_some_and(|value| value != &oid) {
            blocked = Some("The Undo recovery ref has changed. Inspect it with Git.".into());
        }
        if self.commit_state(root)?.1 != state {
            return Err(AppError::new(
                "stale_preview",
                "HEAD or staging changed. Review Undo or Redo again.",
            ));
        }
        let token = digest(
            &serde_json::to_vec(&(
                redo,
                &branch,
                &oid,
                &parent_oid,
                &state,
                &saved,
                &referenced,
                &unstaged,
                &untracked,
                staged_matches,
                &recovery,
                &blocked,
            ))
            .map_err(|e| AppError::new("snapshot", e.to_string()))?,
        );
        Ok(CommitHistoryPreview {
            redo,
            branch,
            commit_oid: oid,
            parent_oid,
            subject,
            recovery_ref,
            token,
            blocked_reason: blocked,
        })
    }

    pub fn apply_commit_history(&self, root: &Path, redo: bool, token: &str) -> Result<()> {
        let discovery = self.discover(root)?;
        // Git checkout needs this index lock. The application also holds the
        // common GitNova operation lock; update-ref locks the branch and HEAD
        // together itself, and compares the exact reviewed branch OID.
        let _index = self.acquire_index_guard(root)?;
        let p = self.preview_commit_history(root, redo)?;
        if p.token != token {
            return Err(AppError::new(
                "stale_preview",
                "HEAD, files or staging changed. Review Undo or Redo again.",
            ));
        }
        if let Some(reason) = p.blocked_reason {
            return Err(AppError::new("history_blocked", reason));
        }
        let before_index = index_bytes(&discovery.git_dir)?;
        let branch_ref = format!("refs/heads/{}", p.branch);
        let slot = format!(
            "refs/gitnova/redo/{}",
            history_slot(&discovery.git_dir, &p.branch)
        );
        let saved = self.optional_text(root, &["rev-parse", "--verify", "--quiet", &slot])?;
        let recovery =
            self.optional_text(root, &["rev-parse", "--verify", "--quiet", &p.recovery_ref])?;
        let mut transaction = String::from("start\n");
        transaction.push_str(&if recovery.is_some() {
            format!("verify {} {}\n", p.recovery_ref, p.commit_oid)
        } else {
            format!("create {} {}\n", p.recovery_ref, p.commit_oid)
        });
        if redo {
            transaction.push_str(&format!("delete {slot} {}\n", p.commit_oid));
            transaction.push_str(&if let Some(parent) = &p.parent_oid {
                format!("update {branch_ref} {} {parent}\n", p.commit_oid)
            } else {
                format!("create {branch_ref} {}\n", p.commit_oid)
            });
        } else {
            transaction.push_str(&if let Some(saved) = saved {
                format!("update {slot} {} {saved}\n", p.commit_oid)
            } else {
                format!("create {slot} {}\n", p.commit_oid)
            });
            transaction.push_str(&if let Some(parent) = &p.parent_oid {
                format!("update {branch_ref} {parent} {}\n", p.commit_oid)
            } else {
                format!("delete {branch_ref} {}\n", p.commit_oid)
            });
        }
        transaction.push_str("prepare\ncommit\n");
        // Ref transactions normally invoke reference-transaction hooks. Undo is
        // a local metadata movement: use a private empty hooks directory.
        let hooks = tempfile::tempdir()?;
        let mut hooks_config = std::ffi::OsString::from("core.hooksPath=");
        hooks_config.push(crate::process::git_path(hooks.path())?);
        run_git_write(
            root,
            &[
                "-c".into(),
                hooks_config,
                "update-ref".into(),
                "--no-deref".into(),
                "--stdin".into(),
            ],
            None,
            Some(""),
            Some(transaction.as_bytes()),
        )?;
        let status = self.status(root)?;
        let expected = if redo {
            Some(&p.commit_oid)
        } else {
            p.parent_oid.as_ref()
        };
        if status.head.as_ref() != expected
            || status.branch.as_ref() != Some(&p.branch)
            || index_bytes(&discovery.git_dir)? != before_index
        {
            return Err(AppError::new("operation_unknown", "Inspect the Undo recovery ref and current branch. The result could not be verified; nothing was rolled back."));
        }
        Ok(())
    }
}
