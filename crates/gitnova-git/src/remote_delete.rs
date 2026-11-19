//! One explicit remote branch deletion, with an exact server-side lease and local recovery.
use crate::{
    working::{run_fetch, run_ref_transaction},
    BranchReader, GitCli, RepositoryReader,
};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};
impl GitCli {
    fn remote_delete_tracking(
        &self,
        path: &Path,
        request: &RemoteDeleteRequest,
        url: &str,
    ) -> Result<(String, bool, bool)> {
        let tracking = format!("refs/remotes/{}/{}", request.remote, request.branch);
        self.text(path, &["check-ref-format", &tracking])?;
        let fetch = self.text(path, &["remote", "get-url", "--all", "--", &request.remote])?;
        let mapping = self
            .optional_text(
                path,
                &[
                    "config",
                    "--get-all",
                    &format!("remote.{}.fetch", request.remote),
                ],
            )?
            .unwrap_or_default();
        let normal = mapping.lines().count() == 1
            && mapping.lines().any(|line| {
                let line = line.trim_start_matches('+');
                line == format!("refs/heads/*:refs/remotes/{}/*", request.remote)
                    || line == format!("refs/heads/{}:{tracking}", request.branch)
            });
        let same_destination = fetch.lines().count() == 1 && fetch.trim_end_matches('\n') == url;
        Ok((tracking, normal, same_destination))
    }

    /// Refresh exactly one tracking ref from the destination used for the delete review.
    /// A missing remote branch never triggers a push or a general prune.
    pub fn refresh_remote_delete_tracking(
        &self,
        path: &Path,
        request: &RemoteDeleteRequest,
    ) -> Result<()> {
        self.valid_name(path, &request.branch, "heads")?;
        crate::validate_oid(&request.oid)?;
        self.write_state(path)?;
        let config = self.config_snapshot(path)?;
        let url = self.push_destination(path, &request.remote)?;
        let (tracking, normal, same_destination) =
            self.remote_delete_tracking(path, request, &url)?;
        if !normal || !same_destination {
            return Err(AppError::new(
                "remote_delete_blocked",
                "Refresh here requires matching fetch and push destinations and a standard fetch mapping.",
            ));
        }
        let desired = self.destination_head(path, &url, &request.branch)?;
        if let Some(oid) = &desired {
            if oid != &request.oid {
                let full = format!("refs/heads/{}", request.branch);
                // Fetch the objects without updating tracking refs, tags or FETCH_HEAD.
                let args: Vec<OsString> = [
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
                    &url,
                    &full,
                ]
                .into_iter()
                .map(OsString::from)
                .collect();
                run_fetch(path, &args)?;
                if self.text(path, &["cat-file", "-t", oid])?.trim() != "commit" {
                    return Err(AppError::new(
                        "stale_preview",
                        "The remote branch changed during refresh. Check it again.",
                    ));
                }
            }
        }
        if self.destination_head(path, &url, &request.branch)? != desired
            || self.config_snapshot(path)? != config
            || self.push_destination(path, &request.remote)? != url
        {
            return Err(AppError::new(
                "stale_preview",
                "The remote branch changed during refresh. Check it again.",
            ));
        }
        self.reconcile_tracking_ref(
            path,
            &tracking,
            Some(&request.oid),
            desired.as_deref(),
            request.oid.len(),
        )
    }

    pub fn preview_remote_delete(
        &self,
        path: &Path,
        request: &RemoteDeleteRequest,
    ) -> Result<RemoteDeletePreview> {
        self.valid_name(path, &request.branch, "heads")?;
        crate::validate_oid(&request.oid)?;
        let (discovery, state) = self.write_state(path)?;
        let config = self.config_snapshot(path)?;
        let url = self.push_destination(path, &request.remote)?;
        let (tracking, normal, same_destination) =
            self.remote_delete_tracking(path, request, &url)?;
        let remote_oid = self.destination_head(path, &url, &request.branch)?;
        if remote_oid.as_ref() != Some(&request.oid) {
            let message = if !normal || !same_destination {
                "The push destination differs from the selected branch. Check the remote endpoints and fetch mapping before deleting."
            } else if remote_oid.is_none() {
                "This branch no longer exists on the remote. Refresh to remove its outdated local label."
            } else {
                "This branch points to a different commit on the remote. Refresh and review the new tip before deleting."
            };
            return Err(AppError::new("stale_preview", message));
        }
        let mut blocked = None;
        if self
            .text(path, &["cat-file", "-t", &request.oid])
            .ok()
            .as_deref()
            .map(str::trim)
            != Some("commit")
        {
            blocked = Some(
                "Fetch this branch before deleting it so its commits can be retained locally."
                    .into(),
            );
        }
        let local_path = if let Some(file) = url.strip_prefix("file://") {
            if !file.starts_with('/') || file.contains('%') {
                return Err(AppError::new("remote_delete_blocked", "Use a local path remote instead of an encoded or non-local file URL for deletion."));
            }
            Some(path.join(file))
        } else {
            let candidate = path.join(&url);
            candidate.exists().then_some(candidate)
        };
        if let Some(destination) = local_path {
            if self
                .discover(&destination)
                .is_ok_and(|d| d.common_dir == discovery.common_dir)
            {
                blocked=Some("The destination is this same repository. Choose an independent remote repository.".into());
            }
        }
        if self
            .optional_text(
                path,
                &[
                    "config",
                    "--bool",
                    "--get",
                    &format!("remote.{}.mirror", request.remote),
                ],
            )?
            .as_deref()
            == Some("true")
        {
            blocked = Some("Mirror remotes require a separately reviewed workflow.".into());
        }
        if self
            .optional_text(path, &["config", "--get-all", "push.pushOption"])?
            .is_some_and(|v| !v.is_empty())
        {
            blocked = Some(
                "Configured server push options require a separately reviewed workflow.".into(),
            );
        }
        // Read-only discovery: no push hooks run during preview.
        let args = ["ls-remote", "--symref", "--", url.as_str(), "HEAD"];
        let bytes = self
            .execute_transport(
                path,
                &args.iter().map(OsStr::new).collect::<Vec<_>>(),
                65536,
                false,
                "file:http:https:ssh:git",
            )
            .map_err(|_| {
                AppError::new(
                    "remote_delete_blocked",
                    "Cannot inspect the remote default branch. Check access and review again.",
                )
            })?
            .bytes;
        let text = String::from_utf8(bytes).map_err(|_| {
            AppError::new(
                "remote_delete_blocked",
                "Invalid remote default branch response.",
            )
        })?;
        if text
            .lines()
            .any(|line| line == format!("ref: refs/heads/{}\tHEAD", request.branch))
        {
            blocked=Some("The remote default branch cannot be deleted here. Change its default on the host first.".into());
        }
        if !normal {
            blocked = Some(
                "Remote deletion requires a standard fetch mapping for the selected branch.".into(),
            );
        }
        let tracking_ref = (same_destination && normal).then_some(tracking);
        let tracking_oid = if let Some(reference) = &tracking_ref {
            if self
                .optional_text(path, &["symbolic-ref", "--quiet", reference])?
                .is_some()
            {
                return Err(AppError::new(
                    "remote_delete_blocked",
                    "Symbolic tracking refs require separate inspection.",
                ));
            }
            self.optional_text(path, &["rev-parse", "--verify", "--quiet", reference])?
        } else {
            None
        };
        let identity = serde_json::to_vec(&(&request.remote, &request.branch, &url, &request.oid))
            .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        let recovery_ref = format!(
            "refs/gitnova/recovery/remote-delete-{:x}",
            Sha256::digest(identity)
        );
        if self
            .optional_text(path, &["symbolic-ref", "--quiet", &recovery_ref])?
            .is_some()
        {
            return Err(AppError::new(
                "remote_delete_blocked",
                "The recovery ref requires separate inspection.",
            ));
        }
        let recovery_oid =
            self.optional_text(path, &["rev-parse", "--verify", "--quiet", &recovery_ref])?;
        if recovery_oid.as_ref().is_some_and(|v| v != &request.oid) {
            blocked = Some(
                "The recovery ref is occupied by another object. Inspect it before deleting."
                    .into(),
            );
        }
        let branches = self.local_branches(path)?;
        let local_branches = branches.iter().map(|b| b.name.clone()).collect();
        let token = serde_json::to_vec(&(
            request,
            &state,
            &config,
            &url,
            &remote_oid,
            &text,
            &tracking_ref,
            &tracking_oid,
            &branches,
            &recovery_oid,
        ))
        .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        Ok(RemoteDeletePreview {
            request: request.clone(),
            destination: crate::push::display_destination(&url),
            recovery_ref,
            tracking_ref,
            tracking_oid,
            local_branches,
            token: format!("{:x}", Sha256::digest(token)),
            blocked_reason: blocked,
        })
    }
    pub fn apply_remote_delete(
        &self,
        path: &Path,
        request: &RemoteDeleteRequest,
        token: &str,
    ) -> Result<()> {
        let review = self.preview_remote_delete(path, request)?;
        if review.token != token {
            return Err(AppError::new(
                "stale_preview",
                "The remote branch, checkout or configuration changed. Review deletion again.",
            ));
        }
        if let Some(reason) = review.blocked_reason {
            return Err(AppError::new("remote_delete_blocked", reason));
        }
        let before = self.head_index_snapshot(path)?;
        let branches = self.text(
            path,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/heads/",
            ],
        )?;
        let old = self.optional_text(
            path,
            &["rev-parse", "--verify", "--quiet", &review.recovery_ref],
        )?;
        let save = if old.is_some() {
            format!("verify {} {}\n", review.recovery_ref, request.oid)
        } else {
            format!("create {} {}\n", review.recovery_ref, request.oid)
        };
        run_ref_transaction(path, &format!("start\n{save}prepare\ncommit\n"))?;
        let url = self.push_destination(path, &request.remote)?;
        let full = format!("refs/heads/{}", request.branch);
        let lease = format!("--force-with-lease={full}:{}", request.oid);
        let spec = format!(":{full}");
        // The lease is an exact compare-and-delete guard, not an unrestricted force push.
        let args: Vec<OsString> = [
            "push",
            "--porcelain",
            "--no-force",
            "--no-force-with-lease",
            &lease,
            "--no-mirror",
            "--no-follow-tags",
            "--recurse-submodules=no",
            "--",
            &url,
            &spec,
        ]
        .iter()
        .map(OsString::from)
        .collect();
        run_fetch(path,&args).map_err(|_|AppError::new("operation_unknown","Remote deletion was not verified. Inspect the remote before retrying; the recovery ref is retained."))?;
        if self
            .destination_head(path, &url, &request.branch)?
            .is_some()
        {
            return Err(AppError::new(
                "operation_unknown",
                "The remote branch is still present or was recreated. Inspect it before retrying.",
            ));
        }
        if let Some(reference) = &review.tracking_ref {
            self.reconcile_tracking_ref(
                path,
                reference,
                review.tracking_oid.as_deref(),
                None,
                request.oid.len(),
            )?;
        }
        if self.head_index_snapshot(path)? != before
            || self.text(
                path,
                &[
                    "for-each-ref",
                    "--format=%(refname) %(objectname)",
                    "refs/heads/",
                ],
            )? != branches
            || self
                .optional_text(
                    path,
                    &["rev-parse", "--verify", "--quiet", &review.recovery_ref],
                )?
                .as_ref()
                != Some(&request.oid)
        {
            return Err(AppError::new("operation_unknown","The remote was deleted but local state changed. Inspect both sides; deletion was not rolled back."));
        }
        Ok(())
    }
}
