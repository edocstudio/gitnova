//! Reviewed branch metadata operations and recoverable local ref deletion.
use crate::{
    working::{run_offline_write, run_ref_transaction},
    BranchReader, GitCli, RemoteReader, RepositoryReader, StatusReader,
};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::path::Path;
fn index_snapshot(path: &Path) -> Result<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}
impl GitCli {
    pub(crate) fn config_snapshot(&self, path: &Path) -> Result<Vec<u8>> {
        Ok(self
            .read(
                path,
                &["config".as_ref(), "--null".as_ref(), "--list".as_ref()],
                1024 * 1024,
            )?
            .bytes)
    }
    pub(crate) fn valid_name(&self, path: &Path, name: &str, namespace: &str) -> Result<()> {
        if name.is_empty() || name.len() > 256 || name.starts_with('-') || name == "HEAD" {
            return Err(AppError::new(
                "ref_operation",
                "Choose a valid branch or tag name, up to 256 bytes.",
            ));
        }
        self.text(
            path,
            &["check-ref-format", &format!("refs/{namespace}/{name}")],
        )
        .map_err(|_| AppError::new("ref_operation", "Invalid Git ref name."))?;
        Ok(())
    }
    pub(crate) fn branch_upstream(&self, path: &Path, name: &str) -> Result<Option<String>> {
        let value = self.text(
            path,
            &[
                "for-each-ref",
                "--format=%(upstream)",
                &format!("refs/heads/{name}"),
            ],
        )?;
        Ok((!value.trim().is_empty()).then(|| value.trim().to_owned()))
    }
    pub(crate) fn multiple_branch_worktrees(&self, path: &Path, name: &str) -> Result<bool> {
        let raw = self
            .read(
                path,
                &[
                    "worktree".as_ref(),
                    "list".as_ref(),
                    "--porcelain".as_ref(),
                    "-z".as_ref(),
                ],
                1024 * 1024,
            )?
            .bytes;
        let expected = format!("branch refs/heads/{name}");
        Ok(raw
            .split(|b| *b == 0)
            .filter(|record| *record == expected.as_bytes())
            .count()
            > 1)
    }
    pub(crate) fn inherited_tracking(&self, path: &Path, name: &str) -> Result<bool> {
        for field in ["remote", "merge"] {
            let key = format!("branch.{name}.{field}");
            if self.optional_text(path, &["config", "--get-all", &key])?
                != self.optional_text(
                    path,
                    &["config", "--local", "--no-includes", "--get-all", &key],
                )?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(crate) fn set_exact_upstream(
        &self,
        path: &Path,
        name: &str,
        remote: &str,
        branch: &str,
    ) -> Result<()> {
        run_offline_write(
            path,
            &[
                "config".into(),
                "--local".into(),
                "--replace-all".into(),
                format!("branch.{name}.remote").into(),
                remote.into(),
            ],
        )?;
        run_offline_write(
            path,
            &[
                "config".into(),
                "--local".into(),
                "--replace-all".into(),
                format!("branch.{name}.merge").into(),
                format!("refs/heads/{branch}").into(),
            ],
        )?;
        if self
            .optional_text(
                path,
                &["config", "--get-all", &format!("branch.{name}.remote")],
            )?
            .as_deref()
            != Some(remote)
            || self
                .optional_text(
                    path,
                    &["config", "--get-all", &format!("branch.{name}.merge")],
                )?
                .as_deref()
                != Some(&format!("refs/heads/{branch}"))
        {
            return Err(AppError::new("operation_unknown","Tracking settings differ from the requested remote and branch. Inspect configuration before retrying."));
        }
        Ok(())
    }
    pub fn preview_ref_operation(
        &self,
        path: &Path,
        action: &RefOperation,
    ) -> Result<RefOperationPreview> {
        let tag = action.kind == RefOperationKind::DeleteTag;
        let namespace = if tag { "tags" } else { "heads" };
        self.valid_name(path, &action.name, namespace)?;
        crate::validate_oid(&action.oid)?;
        if action.kind != RefOperationKind::RenameBranch && !action.new_name.is_empty()
            || action.kind != RefOperationKind::SetUpstream
                && (!action.remote.is_empty() || !action.remote_branch.is_empty())
        {
            return Err(AppError::new(
                "ref_operation",
                "Unexpected fields for the selected ref operation.",
            ));
        }
        let full = format!("refs/{namespace}/{}", action.name);
        if self
            .optional_text(path, &["symbolic-ref", "--quiet", &full])?
            .is_some()
        {
            return Err(AppError::new(
                "ref_operation",
                "Symbolic refs need separate inspection before changing them.",
            ));
        }
        let object_oid = self
            .text(path, &["rev-parse", "--verify", &full])?
            .trim()
            .to_owned();
        crate::validate_oid(&object_oid)?;
        let commit = self.text(
            path,
            &["rev-parse", "--verify", &format!("{full}^{{commit}}")],
        )?;
        if commit.trim() != action.oid {
            return Err(AppError::new(
                "stale_preview",
                "The selected ref moved. Refresh the graph before reviewing it.",
            ));
        }
        let (d, state) = self.write_state(path)?;
        let config = self.config_snapshot(path)?;
        let branches = self.local_branches(path)?;
        let status = self.status(path)?;
        let branch = branches.iter().find(|b| b.name == action.name);
        let mut blocked = None;
        if let Some(b) = branch.filter(|_| !tag) {
            if b.checked_out_at.is_some()
                && !(b.current
                    && matches!(
                        action.kind,
                        RefOperationKind::RenameBranch
                            | RefOperationKind::SetUpstream
                            | RefOperationKind::UnsetUpstream
                    ))
            {
                blocked=Some("This branch is checked out in a worktree. Switch that worktree before changing or deleting it.".into());
            }
        }
        if self
            .optional_text(
                path,
                &["config", "--bool", "--get", "extensions.worktreeConfig"],
            )?
            .as_deref()
            == Some("true")
            && !tag
        {
            blocked=Some("Worktree-specific branch configuration needs separate review before this operation.".into());
        }
        if d.git_dir.join("HEAD.lock").exists()
            || d.git_dir.join("index.lock").exists()
            || d.common_dir.join("config.lock").exists()
        {
            blocked = Some(
                "Another Git operation holds a native lock. Its locks will not be removed.".into(),
            );
        }
        if !tag
            && matches!(
                action.kind,
                RefOperationKind::RenameBranch
                    | RefOperationKind::SetUpstream
                    | RefOperationKind::UnsetUpstream
            )
            && self.inherited_tracking(path, &action.name)?
        {
            blocked=Some("Tracking settings from included/global configuration need separate review before this operation.".into());
        }
        if !tag && self.multiple_branch_worktrees(path, &action.name)? {
            blocked=Some("This branch is checked out in multiple worktrees. Review those worktrees before changing shared branch metadata.".into());
        }
        let upstream_before = if tag {
            None
        } else {
            self.branch_upstream(path, &action.name)?
        };
        let mut upstream_after = upstream_before.clone();
        match action.kind {
            RefOperationKind::RenameBranch => {
                self.valid_name(path, &action.new_name, "heads")?;
                if branches.iter().any(|b| b.name == action.new_name)
                    || self
                        .optional_text(
                            path,
                            &[
                                "symbolic-ref",
                                "--quiet",
                                &format!("refs/heads/{}", action.new_name),
                            ],
                        )?
                        .is_some()
                {
                    blocked = Some(
                        "The destination branch already exists. Rename will not overwrite it."
                            .into(),
                    );
                }
            }
            RefOperationKind::SetUpstream => {
                let list = self.remote_branches(path)?;
                let target = list
                    .branches
                    .iter()
                    .find(|b| b.remote == action.remote && b.name == action.remote_branch)
                    .ok_or_else(|| {
                        AppError::new(
                            "ref_operation",
                            "Fetch and choose an existing configured remote branch first.",
                        )
                    })?;
                let mapping = self
                    .optional_text(
                        path,
                        &[
                            "config",
                            "--get-all",
                            &format!("remote.{}.fetch", action.remote),
                        ],
                    )?
                    .unwrap_or_default();
                if mapping.lines().count() != 1
                    || !mapping.lines().any(|line| {
                        line.trim_start_matches('+')
                            == format!("refs/heads/*:refs/remotes/{}/*", action.remote)
                            || line.trim_start_matches('+')
                                == format!(
                                    "refs/heads/{}:refs/remotes/{}/{}",
                                    action.remote_branch, action.remote, action.remote_branch
                                )
                    })
                {
                    blocked = Some(
                        "This upstream requires a standard, unambiguous fetch mapping.".into(),
                    );
                }
                upstream_after = Some(format!("refs/remotes/{}/{}", target.remote, target.name));
                if upstream_after == upstream_before {
                    blocked = Some("This upstream is already configured.".into());
                }
            }
            RefOperationKind::UnsetUpstream => {
                upstream_after = None;
                if upstream_before.is_none() {
                    blocked = Some("This branch has no upstream.".into());
                }
            }
            _ => {}
        }
        let deleting = matches!(
            action.kind,
            RefOperationKind::DeleteBranch | RefOperationKind::DeleteTag
        );
        let recovery_ref = deleting.then(|| {
            format!(
                "refs/gitnova/recovery/{:x}",
                Sha256::digest(format!("{full}\0{object_oid}"))
            )
        });
        let recovery_oid = if let Some(ref backup) = recovery_ref {
            if self
                .optional_text(path, &["symbolic-ref", "--quiet", backup])?
                .is_some()
            {
                return Err(AppError::new(
                    "ref_operation",
                    "The recovery destination is symbolic. Inspect it with Git before deleting.",
                ));
            }
            self.optional_text(path, &["rev-parse", "--verify", "--quiet", backup])?
        } else {
            None
        };
        if recovery_oid.as_ref().is_some_and(|old| old != &object_oid) {
            blocked=Some("The recovery ref is occupied by another object. Inspect it with Git before deleting.".into());
        }
        let annotation =
            if tag && self.text(path, &["cat-file", "-t", &object_oid])?.trim() == "tag" {
                Some(self.text(path, &["cat-file", "-p", &object_oid])?)
            } else {
                None
            };
        let retained_by = if deleting {
            self.text(
                path,
                &[
                    "for-each-ref",
                    &format!("--contains={}", action.oid),
                    "--format=%(refname)",
                    "refs/heads/",
                ],
            )?
            .lines()
            .filter(|r| *r != full)
            .map(str::to_owned)
            .collect()
        } else {
            vec![]
        };
        let data = serde_json::to_vec(&(
            action,
            &object_oid,
            &state,
            &config,
            &branches,
            &status,
            &upstream_before,
            &upstream_after,
            &recovery_oid,
        ))
        .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        Ok(RefOperationPreview {
            action: action.clone(),
            object_oid,
            recovery_ref,
            annotation,
            upstream_before,
            upstream_after,
            retained_by,
            token: format!("{:x}", Sha256::digest(data)),
            blocked_reason: blocked,
        })
    }
    pub fn apply_ref_operation(
        &self,
        path: &Path,
        action: &RefOperation,
        token: &str,
    ) -> Result<()> {
        let review = self.preview_ref_operation(path, action)?;
        if review.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Ref, worktree or configuration changed. Review again.",
            ));
        }
        if let Some(reason) = review.blocked_reason {
            return Err(AppError::new("ref_blocked", reason));
        }
        let d = self.discover(path)?;
        let index = index_snapshot(&d.git_dir.join("index"))?;
        let before = self.status(path)?;
        match action.kind {
            RefOperationKind::RenameBranch => run_offline_write(
                path,
                &[
                    "branch".into(),
                    "-m".into(),
                    "--".into(),
                    action.name.clone().into(),
                    action.new_name.clone().into(),
                ],
            )?,
            RefOperationKind::SetUpstream => {
                self.set_exact_upstream(path, &action.name, &action.remote, &action.remote_branch)?
            }
            RefOperationKind::UnsetUpstream => run_offline_write(
                path,
                &[
                    "branch".into(),
                    "--unset-upstream".into(),
                    "--".into(),
                    action.name.clone().into(),
                ],
            )?,
            RefOperationKind::DeleteBranch | RefOperationKind::DeleteTag => {
                let backup = review
                    .recovery_ref
                    .as_deref()
                    .ok_or_else(|| AppError::new("ref_operation", "Missing recovery ref"))?;
                let old =
                    self.optional_text(path, &["rev-parse", "--verify", "--quiet", backup])?;
                let save = if old.is_some() {
                    format!("verify {backup} {}\n", review.object_oid)
                } else {
                    format!("create {backup} {}\n", review.object_oid)
                };
                let ns = if action.kind == RefOperationKind::DeleteTag {
                    "tags"
                } else {
                    "heads"
                };
                run_ref_transaction(
                    path,
                    &format!(
                        "start\n{save}delete refs/{ns}/{} {}\nprepare\ncommit\n",
                        action.name, review.object_oid
                    ),
                )?;
                if !self
                    .optional_text(path, &["rev-parse", "--verify", "--quiet", backup])?
                    .is_some_and(|oid| oid == review.object_oid)
                {
                    return Err(AppError::new(
                        "operation_unknown",
                        "Recovery ref needs inspection before retrying.",
                    ));
                }
                if !matches!(action.kind, RefOperationKind::DeleteTag) {
                    let escaped = action
                        .name
                        .chars()
                        .map(|c| {
                            if ".*+?()[]{}^$|\\".contains(c) {
                                format!("\\{c}")
                            } else {
                                c.to_string()
                            }
                        })
                        .collect::<String>();
                    if self
                        .optional_text(
                            path,
                            &[
                                "config",
                                "--local",
                                "--get-regexp",
                                &format!("^branch\\.{escaped}\\.[^.]+$"),
                            ],
                        )?
                        .is_some()
                    {
                        run_offline_write(
                            path,
                            &[
                                "config".into(),
                                "--local".into(),
                                "--remove-section".into(),
                                format!("branch.{}", action.name).into(),
                            ],
                        )?;
                    }
                }
            }
        }
        let after = self.status(path)?;
        let refs = self.local_branches(path)?;
        let valid = match action.kind {
            RefOperationKind::RenameBranch => {
                !refs.iter().any(|b| b.name == action.name)
                    && refs
                        .iter()
                        .any(|b| b.name == action.new_name && b.oid == action.oid)
                    && self.branch_upstream(path, &action.new_name)? == review.upstream_before
                    && after.branch
                        == if before.branch.as_ref() == Some(&action.name) {
                            Some(action.new_name.clone())
                        } else {
                            before.branch.clone()
                        }
            }
            RefOperationKind::DeleteBranch => {
                !refs.iter().any(|b| b.name == action.name) && after.branch == before.branch
            }
            RefOperationKind::DeleteTag => {
                self.optional_text(
                    path,
                    &[
                        "rev-parse",
                        "--verify",
                        "--quiet",
                        &format!("refs/tags/{}", action.name),
                    ],
                )?
                .is_none()
                    && after.branch == before.branch
            }
            _ => {
                self.branch_upstream(path, &action.name)? == review.upstream_after
                    && after.branch == before.branch
            }
        };
        if !valid || after.head != before.head || index_snapshot(&d.git_dir.join("index"))? != index
        {
            return Err(AppError::new("operation_unknown","Ref/configuration operation completed but the result differs from the review. Inspect refs, checkout and the recorded recovery ref; no automatic rollback was attempted."));
        }
        Ok(())
    }
}
