//! Selected-remote fetch and reviewed, nonrecursive fast-forward integration.
use crate::{
    working::{run_fetch, run_write},
    BranchReader, GitCli, StatusReader,
};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

pub trait RemoteReader {
    fn remote_branches(&self, path: &Path) -> Result<RemoteBranches>;
    fn preview_remote_switch(
        &self,
        path: &Path,
        remote: &str,
        branch: &str,
    ) -> Result<RemoteSwitchPreview>;
}
pub trait RemoteWriter {
    fn fetch_remote(&self, path: &Path, remote: &str, branch: Option<&str>) -> Result<()>;
    fn switch_remote_branch(
        &self,
        path: &Path,
        remote: &str,
        branch: &str,
        token: &str,
    ) -> Result<()>;
}
impl GitCli {
    /// Same explicit remote-tracking-only fetch, with a noninteractive budget.
    pub fn fetch_remote_background(&self, path: &Path, remote: &str) -> Result<()> {
        self.fetch_remote_impl(path, remote, None, true)
    }
    fn fetch_remote_impl(
        &self,
        path: &Path,
        remote: &str,
        branch: Option<&str>,
        background: bool,
    ) -> Result<()> {
        self.validate_remote_selection(path, remote, branch)?;
        let refspec = match branch {
            Some(branch) => format!("+refs/heads/{branch}:refs/remotes/{remote}/{branch}"),
            None => format!("+refs/heads/*:refs/remotes/{remote}/*"),
        };
        let mut args: Vec<OsString> = [
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.file.allow=always",
            "-c",
            "protocol.https.allow=always",
            "-c",
            "protocol.http.allow=always",
            "-c",
            "protocol.ssh.allow=always",
            "-c",
            "protocol.git.allow=always",
            "-c",
            "submodule.recurse=false",
            "fetch",
            "--no-tags",
            "--no-prune",
            "--no-prune-tags",
            "--no-recurse-submodules",
            "--no-write-fetch-head",
            "--no-auto-maintenance",
            "--no-write-commit-graph",
            "--refmap=",
            "--",
            remote,
            &refspec,
        ]
        .iter()
        .map(OsString::from)
        .collect();
        if background {
            args.splice(
                0..0,
                [
                    OsString::from("-c"),
                    OsString::from("gitnova.backgroundFetch=true"),
                ],
            );
        }
        run_fetch(path, &args)
    }
    /// Read the fetch destination without requiring a historical commit to be a branch tip.
    pub fn pipeline_remote(&self, path: &Path, remote: &str, oid: &str) -> Result<String> {
        self.validate_remote_selection(path, remote, None)?;
        crate::validate_oid(oid)?;
        if self.text(path, &["cat-file", "-t", oid])?.trim() != "commit" {
            return Err(AppError::new(
                "hosting",
                "Choose a commit object for CI or sharing.",
            ));
        }
        Ok(self
            .text(path, &["remote", "get-url", remote])?
            .trim()
            .to_owned())
    }
    fn remote_config_snapshot(&self, path: &Path) -> Result<Vec<u8>> {
        match self.read(
            path,
            &[
                OsStr::new("config"),
                OsStr::new("--null"),
                OsStr::new("--get-regexp"),
                OsStr::new(r"^(remote|branch)\."),
            ],
            1024 * 1024,
        ) {
            Ok(out) => Ok(out.bytes),
            Err(e) if e.code == "git_exit_1" => Ok(vec![]),
            Err(e) => Err(e),
        }
    }
    fn validate_remote_selection(
        &self,
        path: &Path,
        remote: &str,
        branch: Option<&str>,
    ) -> Result<()> {
        let configured = self.text(path, &["remote"])?;
        if remote.starts_with('-') || !configured.lines().any(|name| name == remote) {
            return Err(AppError::new(
                "remote",
                "Choose a configured remote from this repository.",
            ));
        }
        if let Some(branch) = branch {
            if branch.starts_with('-') || branch == "HEAD" {
                return Err(AppError::new(
                    "branch",
                    "This remote branch cannot be used as a local branch name.",
                ));
            }
            self.text(path, &["check-ref-format", &format!("refs/heads/{branch}")])?;
        }
        Ok(())
    }
    fn upstream(&self, path: &Path, branch: &str) -> Result<String> {
        Ok(self
            .text(
                path,
                &[
                    "for-each-ref",
                    "--format=%(upstream)",
                    &format!("refs/heads/{branch}"),
                ],
            )?
            .trim_end()
            .to_owned())
    }
}
impl RemoteReader for GitCli {
    fn remote_branches(&self, path: &Path) -> Result<RemoteBranches> {
        let mut remotes: Vec<_> = self
            .text(path, &["remote"])?
            .lines()
            .map(str::to_owned)
            .collect();
        // A remote's name may itself contain '/'. Match the longest configured prefix.
        remotes.sort_by_key(|name| std::cmp::Reverse(name.len()));
        let raw = self
            .read(
                path,
                &[
                    OsStr::new("for-each-ref"),
                    OsStr::new("--format=%(refname)%00%(objectname)%00%(symref)%00"),
                    OsStr::new("refs/remotes/"),
                ],
                4 * 1024 * 1024,
            )?
            .bytes;
        let mut branches = vec![];
        let mut fields = raw.split(|byte| *byte == 0);
        while let Some(name) = fields.next() {
            let name = name.strip_prefix(b"\n").unwrap_or(name);
            if name.is_empty() {
                continue;
            }
            let oid = fields
                .next()
                .ok_or_else(|| AppError::new("git_parse", "Missing remote OID"))?;
            let symbolic = fields
                .next()
                .ok_or_else(|| AppError::new("git_parse", "Missing remote ref kind"))?;
            if !symbolic.is_empty() {
                continue;
            }
            let name = std::str::from_utf8(name)
                .map_err(|_| AppError::new("encoding", "Remote ref is not UTF-8"))?;
            let Some(name) = name.strip_prefix("refs/remotes/") else {
                continue;
            };
            let Some(remote) = remotes
                .iter()
                .find(|remote| name.starts_with(&format!("{remote}/")))
            else {
                continue;
            };
            let branch = &name[remote.len() + 1..];
            let oid = std::str::from_utf8(oid)
                .map_err(|_| AppError::new("encoding", "Remote OID is not UTF-8"))?
                .to_owned();
            crate::validate_oid(&oid)?;
            branches.push(RemoteBranch {
                remote: remote.clone(),
                name: branch.into(),
                oid,
            });
        }
        remotes.sort();
        branches.sort_by(|a, b| (&a.remote, &a.name).cmp(&(&b.remote, &b.name)));
        // Configuration-only, bounded and credential-free display metadata.
        let mut urls = std::collections::BTreeMap::<String, Vec<String>>::new();
        if !remotes.is_empty() {
            for record in self.remote_config_snapshot(path)?.split(|b| *b == 0) {
                let Some(separator) = record.iter().position(|b| *b == b'\n') else {
                    continue;
                };
                let (key, value) = (&record[..separator], &record[separator + 1..]);
                let (Ok(key), Ok(value)) = (std::str::from_utf8(key), std::str::from_utf8(value))
                else {
                    continue;
                };
                for name in &remotes {
                    if key == format!("remote.{name}.url") {
                        if let Ok(safe) = crate::clone::validate_clone_source(value) {
                            urls.entry(name.clone()).or_default().push(safe);
                        }
                    }
                }
            }
        }
        Ok(RemoteBranches {
            urls,
            remotes,
            branches,
        })
    }
    fn preview_remote_switch(
        &self,
        path: &Path,
        remote: &str,
        branch: &str,
    ) -> Result<RemoteSwitchPreview> {
        self.preview_remote_target(path, remote, branch, branch)
    }
}
impl GitCli {
    fn preview_remote_target(
        &self,
        path: &Path,
        remote: &str,
        branch: &str,
        local_branch: &str,
    ) -> Result<RemoteSwitchPreview> {
        self.validate_remote_selection(path, remote, Some(branch))?;
        let config = self.remote_config_snapshot(path)?;
        let listing = self.remote_branches(path)?;
        let remote_branch = listing
            .branches
            .iter()
            .find(|b| b.remote == remote && b.name == branch)
            .ok_or_else(|| {
                AppError::new(
                    "remote_branch",
                    "Fetch this remote to discover its branches first.",
                )
            })?;
        let local = self
            .local_branches(path)?
            .into_iter()
            .find(|b| b.name == local_branch);
        let create = local.is_none();
        let (ahead, behind) = if let Some(local) = &local {
            let counts = self.text(
                path,
                &[
                    "rev-list",
                    "--left-right",
                    "--count",
                    &format!("{}...{}", local.oid, remote_branch.oid),
                    "--",
                ],
            )?;
            let counts = counts
                .split_whitespace()
                .map(str::parse::<usize>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| AppError::new("git_parse", "Invalid ahead/behind counts"))?;
            if counts.len() != 2 {
                return Err(AppError::new("git_parse", "Missing ahead/behind counts"));
            }
            (counts[0], counts[1])
        } else {
            (0, 0)
        };
        let target_oid = if ahead > 0 {
            local
                .as_ref()
                .expect("ahead requires a local branch")
                .oid
                .clone()
        } else {
            remote_branch.oid.clone()
        };
        let status = self.status(path)?;
        let target = LocalBranch {
            name: local_branch.into(),
            oid: target_oid,
            current: status.branch.as_deref() == Some(local_branch),
            checked_out_at: local.as_ref().and_then(|b| {
                if b.current {
                    None
                } else {
                    b.checked_out_at.clone()
                }
            }),
        };
        let mut preview = self.branch_target_preview(path, &target, true)?;
        let upstream = self.upstream(path, local_branch)?;
        if !upstream.is_empty() && upstream != format!("refs/remotes/{remote}/{branch}") {
            preview.blocked_reason = Some(format!("Local {local_branch} already tracks {upstream}. Its tracking configuration will not be replaced."));
        }
        if ahead > 0 && behind > 0 {
            preview.blocked_reason = Some(format!("Local and remote histories diverged: {ahead} local and {behind} remote commits. Review a merge or rebase with Git; no commits will be discarded."));
        }
        let mut hash = Sha256::new();
        hash.update(preview.token.as_bytes());
        hash.update([0]);
        hash.update(remote.as_bytes());
        hash.update([0]);
        hash.update(remote_branch.oid.as_bytes());
        hash.update([0]);
        hash.update(&config);
        preview.token = format!("{:x}", hash.finalize());
        if config != self.remote_config_snapshot(path)? {
            return Err(AppError::new(
                "stale_preview",
                "Remote configuration changed. Review again.",
            ));
        }
        Ok(RemoteSwitchPreview {
            remote: remote.into(),
            remote_branch: branch.into(),
            remote_oid: remote_branch.oid.clone(),
            create,
            local_oid: local.as_ref().map(|b| b.oid.clone()),
            ahead,
            behind,
            switch: preview,
        })
    }
}
impl RemoteWriter for GitCli {
    fn fetch_remote(&self, path: &Path, remote: &str, branch: Option<&str>) -> Result<()> {
        self.fetch_remote_impl(path, remote, branch, false)
    }
    fn switch_remote_branch(
        &self,
        path: &Path,
        remote: &str,
        branch: &str,
        token: &str,
    ) -> Result<()> {
        self.switch_remote_target(path, remote, branch, branch, token)
    }
}
impl GitCli {
    fn switch_remote_target(
        &self,
        path: &Path,
        remote: &str,
        remote_branch: &str,
        branch: &str,
        token: &str,
    ) -> Result<()> {
        let preview = self.preview_remote_target(path, remote, remote_branch, branch)?;
        if preview.switch.token != token {
            return Err(AppError::new("stale_preview", "Local state, fetched remote revision or tracking configuration changed. Fetch and review again."));
        }
        if let Some(reason) = preview.switch.blocked_reason {
            return Err(AppError::new("switch_blocked", reason));
        }
        let confirmed_config = self.remote_config_snapshot(path)?;
        let local_ref = format!("refs/heads/{branch}");
        let remote_ref = format!("refs/remotes/{remote}/{remote_branch}");
        if preview.create {
            // Start from the reviewed immutable OID, not a mutable remote ref or DWIM expression.
            let args: Vec<OsString> = [
                "switch",
                "--no-guess",
                "--no-recurse-submodules",
                "--no-overwrite-ignore",
                "--no-track",
                "-c",
                branch,
                &preview.switch.target_oid,
            ]
            .iter()
            .map(OsString::from)
            .collect();
            run_write(path, &args, None)?;
        } else if preview.switch.source_branch.as_deref() != Some(branch) {
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
        }
        // A checkout hook may have moved HEAD or changed files. Reinspect before integration.
        let status = self.status(path)?;
        if confirmed_config != self.remote_config_snapshot(path)? {
            return Err(AppError::new("operation_unknown", "Checked out the local branch, but remote/tracking configuration changed. Inspect it before retrying the pull."));
        }
        let expected_before_merge = if preview.create {
            preview.switch.target_oid.clone()
        } else {
            preview
                .local_oid
                .clone()
                .expect("existing local branch has an OID")
        };
        if status.branch.as_deref() != Some(branch)
            || status.head.as_deref() != Some(&expected_before_merge)
        {
            return Err(AppError::new(
                "operation_unknown",
                "Checkout changed HEAD unexpectedly. Inspect the repository before retrying.",
            ));
        }
        if !preview.create && preview.behind > 0 {
            // Recheck dirty/hidden state after checkout hooks, allowing the branch's current HEAD.
            let target = LocalBranch {
                name: branch.into(),
                oid: preview.switch.target_oid.clone(),
                current: true,
                checked_out_at: None,
            };
            let check = self.branch_target_preview(path, &target, true)?;
            if let Some(reason) = check.blocked_reason {
                return Err(AppError::new(
                    "operation_unknown",
                    format!("Checked out {branch}, but stopped before pull: {reason}"),
                ));
            }
            let args: Vec<OsString> = [
                "-c",
                "submodule.recurse=false",
                "merge",
                "--ff-only",
                "--no-edit",
                "--no-autostash",
                "--no-overwrite-ignore",
                &preview.switch.target_oid,
            ]
            .iter()
            .map(OsString::from)
            .collect();
            run_write(path, &args, None)?;
        }
        if self.upstream(path, branch)?.is_empty() {
            let args: Vec<OsString> = [
                "branch",
                &format!("--set-upstream-to={remote}/{remote_branch}"),
                "--",
                branch,
            ]
            .iter()
            .map(OsString::from)
            .collect();
            run_write(path, &args, None)?;
        }
        let after = self.status(path)?;
        if after.branch.as_deref() != Some(branch)
            || after.head.as_deref() != Some(&preview.switch.target_oid)
            || self.upstream(path, branch)? != remote_ref
            || self
                .optional_text(path, &["rev-parse", "--verify", &local_ref])?
                .as_deref()
                != Some(&preview.switch.target_oid)
        {
            return Err(AppError::new("operation_unknown", "Remote integration completed with an unexpected HEAD or upstream. Inspect Git state; no rollback was attempted."));
        }
        Ok(())
    }
}

impl GitCli {
    pub fn pull_upstream(&self, path: &Path) -> Result<BranchTracking> {
        let status = self.status(path)?;
        let branch = status
            .branch
            .ok_or_else(|| AppError::new("branch", "Check out a local branch before pulling."))?;
        let tracking = self
            .branch_tracking(path)?
            .into_iter()
            .find(|item| item.branch == branch)
            .ok_or_else(|| {
                AppError::new(
                    "branch",
                    "Current branch tracking is unavailable. Refresh and review again.",
                )
            })?;
        let remote = tracking
            .remote
            .as_deref()
            .filter(|remote| *remote != ".")
            .ok_or_else(|| {
                AppError::new(
                    "remote",
                    "Configure a remote upstream for this branch before pulling.",
                )
            })?;
        let remote_branch = tracking.remote_branch.as_deref().ok_or_else(|| {
            AppError::new(
                "remote_branch",
                "Configure a remote upstream for this branch before pulling.",
            )
        })?;
        self.validate_remote_selection(path, remote, Some(remote_branch))?;
        Ok(tracking)
    }
    pub fn preview_pull_current(&self, path: &Path) -> Result<RemoteSwitchPreview> {
        let tracking = self.pull_upstream(path)?;
        let preview = self.preview_remote_target(
            path,
            tracking.remote.as_deref().unwrap(),
            tracking.remote_branch.as_deref().unwrap(),
            &tracking.branch,
        )?;
        if preview.switch.source_branch.as_deref() != Some(&tracking.branch) {
            return Err(AppError::new(
                "stale_preview",
                "Branch or upstream changed. Close this review and try again.",
            ));
        }
        Ok(preview)
    }
    pub fn pull_current(&self, path: &Path, token: &str) -> Result<()> {
        let review = self.preview_pull_current(path)?;
        if review.create {
            return Err(AppError::new(
                "branch",
                "The current local branch no longer exists.",
            ));
        }
        self.switch_remote_target(
            path,
            &review.remote,
            &review.remote_branch,
            &review.switch.branch,
            token,
        )
    }
}
