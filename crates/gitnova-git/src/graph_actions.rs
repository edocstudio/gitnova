//! Graph-scoped checkout and ref creation; no revision expressions or implicit reset.
use crate::{working::run_offline_write, GitCli, RefReader, StatusReader};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{ffi::OsString, path::Path};
impl GitCli {
    fn validate_graph_action(&self, path: &Path, action: &GraphAction) -> Result<()> {
        crate::validate_oid(&action.oid)?;
        if self.text(path, &["cat-file", "-t", &action.oid])?.trim() != "commit" {
            return Err(AppError::new(
                "graph_action",
                "Choose an exact commit object.",
            ));
        }
        if action.kind == GraphActionKind::CheckoutCommit {
            if !action.name.is_empty() {
                return Err(AppError::new(
                    "graph_action",
                    "Detached checkout does not accept a branch name.",
                ));
            }
        } else {
            if action.name.is_empty()
                || action.name.len() > 256
                || action.name.starts_with('-')
                || action.name == "HEAD"
            {
                return Err(AppError::new(
                    "graph_action",
                    "Enter a valid branch or tag name (up to 256 bytes).",
                ));
            }
            let namespace = if action.kind == GraphActionKind::CreateBranch {
                "heads"
            } else {
                "tags"
            };
            self.text(
                path,
                &[
                    "check-ref-format",
                    &format!("refs/{namespace}/{}", action.name),
                ],
            )
            .map_err(|_| {
                AppError::new(
                    "graph_action",
                    "This branch or tag name is not valid in Git.",
                )
            })?;
        }
        Ok(())
    }
    pub fn preview_graph_action(
        &self,
        path: &Path,
        action: &GraphAction,
    ) -> Result<GraphActionPreview> {
        self.validate_graph_action(path, action)?;
        let (discovery, state) = self.write_state(path)?;
        let refs = self.refs(path)?;
        let status = self.status(path)?;
        let mut blocked = None;
        let namespace = if action.kind == GraphActionKind::CreateBranch {
            "heads"
        } else {
            "tags"
        };
        if action.kind != GraphActionKind::CheckoutCommit
            && refs
                .iter()
                .any(|r| r.name == format!("refs/{namespace}/{}", action.name))
        {
            blocked =
                Some("That name already exists. Existing refs will never be overwritten.".into());
        }
        let switch = if action.kind == GraphActionKind::CreateTag {
            None
        } else {
            let target = LocalBranch {
                name: if action.kind == GraphActionKind::CheckoutCommit {
                    format!("Detached {}", &action.oid[..8])
                } else {
                    action.name.clone()
                },
                oid: action.oid.clone(),
                current: false,
                checked_out_at: None,
            };
            let review = self.branch_target_preview_with_preservation(
                path,
                &target,
                false,
                action.kind == GraphActionKind::CreateBranch
                    && status.head.as_ref() == Some(&action.oid),
            )?;
            if review.blocked_reason.is_some() {
                blocked = review.blocked_reason.clone();
            }
            Some(review)
        };
        if discovery.git_dir.join("index.lock").exists()
            || discovery.git_dir.join("HEAD.lock").exists()
        {
            blocked = Some(
                "Another Git operation holds an index or HEAD lock. Its locks will not be removed."
                    .into(),
            );
        }
        let bytes = serde_json::to_vec(&(action, &state, &refs, &status, &switch))
            .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        Ok(GraphActionPreview {
            action: action.clone(),
            switch,
            token: format!("{:x}", Sha256::digest(bytes)),
            blocked_reason: blocked,
        })
    }
    pub fn apply_graph_action(&self, path: &Path, action: &GraphAction, token: &str) -> Result<()> {
        let review = self.preview_graph_action(path, action)?;
        if review.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Repository or selected ref changed. Review this graph action again.",
            ));
        }
        if let Some(reason) = review.blocked_reason {
            return Err(AppError::new("graph_blocked", reason));
        }
        let args: Vec<OsString> = match action.kind {
            GraphActionKind::CheckoutCommit => vec![
                "switch".into(),
                "--detach".into(),
                "--no-guess".into(),
                "--no-recurse-submodules".into(),
                "--no-overwrite-ignore".into(),
                action.oid.clone().into(),
            ],
            GraphActionKind::CreateBranch => vec![
                "switch".into(),
                "--no-guess".into(),
                "--no-track".into(),
                "--no-recurse-submodules".into(),
                "--no-overwrite-ignore".into(),
                "-c".into(),
                action.name.clone().into(),
                action.oid.clone().into(),
            ],
            // Compare-and-swap creation with an all-zero old OID cannot overwrite a racing tag.
            GraphActionKind::CreateTag => vec![
                "update-ref".into(),
                "-m".into(),
                "GitNova: create lightweight tag".into(),
                format!("refs/tags/{}", action.name).into(),
                action.oid.clone().into(),
                "0".repeat(action.oid.len()).into(),
            ],
        };
        let before = self.write_state(path)?.1;
        run_offline_write(path, &args)?;
        let after = self.status(path)?;
        let valid = match action.kind {
            GraphActionKind::CheckoutCommit => {
                after.branch.is_none() && after.head.as_ref() == Some(&action.oid)
            }
            GraphActionKind::CreateBranch => {
                after.branch.as_ref() == Some(&action.name)
                    && after.head.as_ref() == Some(&action.oid)
            }
            GraphActionKind::CreateTag => {
                before == self.write_state(path)?.1
                    && self.refs(path)?.iter().any(|r| {
                        r.name == format!("refs/tags/{}", action.name) && r.oid == action.oid
                    })
            }
        };
        if !valid {
            return Err(AppError::new("operation_unknown", "Git completed but the resulting checkout/ref differs from the reviewed target. Refresh and inspect before retrying; no rollback was attempted."));
        }
        Ok(())
    }
}
