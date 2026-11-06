//! Resolve real unmerged index entries without inventing a merge/stash session.
use super::{candidate, digest, hidden_entries, recovery, safe_path, Scratch};
use crate::{GitCli, RepositoryReader};
use gitnova_domain::{AppError, MergeAction, MergeActionPreview, MergeState, NativePath, Result};
use std::path::Path;

pub(super) fn preview(
    git: &GitCli,
    root: &Path,
    action: &MergeAction,
    state: MergeState,
) -> Result<MergeActionPreview> {
    if action.kind != "resolve" || !action.message.is_empty() {
        return Err(AppError::new(
            "merge_blocked",
            "Only file resolution is available for standalone conflicts.",
        ));
    }
    let path = action
        .path
        .as_ref()
        .ok_or_else(|| AppError::new("merge_resolve", "Select a conflict path."))?;
    let conflict = state
        .conflicts
        .iter()
        .find(|c| c.path == *path)
        .ok_or_else(|| {
            AppError::new(
                "stale_preview",
                "Select an unresolved conflict path, then review again.",
            )
        })?;
    safe_path(root, path)?;
    let discovery = git.discover(root)?;
    let paths = vec![path.clone()];
    let mut blocked = recovery::backup_blocker(git, root, &paths)?;
    if hidden_entries(git, root)? || conflict.stages.iter().any(|s| s.mode == "160000") {
        blocked = Some("Hidden entries or submodule conflicts need separate inspection.".into());
    }
    for name in ["index.lock", "HEAD.lock", "sequencer", "MERGE_AUTOSTASH"] {
        if discovery.git_dir.join(name).exists() {
            blocked =
                Some("Another Git operation holds a native lock. Wait and review again.".into());
        }
    }
    if discovery.common_dir.join("config.lock").exists() {
        blocked = Some("Another Git operation holds a native lock. Wait and review again.".into());
    }
    let (kind, mode, bytes) = recovery::work_entry(root, path)?;
    if kind == "directory" {
        return Err(AppError::new(
            "merge_resolve",
            "Directory conflicts need manual inspection.",
        ));
    }
    let scratch = Scratch::new(git, root)?;
    let before = scratch.blob_tree(
        &path.to_path()?,
        if kind == "symlink" {
            "120000"
        } else if mode & 0o111 != 0 {
            "100755"
        } else {
            "100644"
        },
        bytes.as_deref(),
    )?;
    let (result_mode, result) = candidate(git, root, conflict, action)?;
    let after = scratch.blob_tree(&path.to_path()?, &result_mode, result.as_deref())?;
    let diff = scratch.diff(&before, &after)?;
    if diff.len() > 512 * 1024 {
        return Err(AppError::new(
            "safety_budget",
            "Operation patch exceeds the complete 512 KiB review budget.",
        ));
    }
    let token = digest(
        &serde_json::to_vec(&(action, &state, &before, &after, &kind, mode))
            .map_err(|e| AppError::new("snapshot", e.to_string()))?,
    );
    if git.merge_state(root)?.token != state.token {
        return Err(AppError::new(
            "stale_preview",
            "Conflict versions changed while reading. Select the file again.",
        ));
    }
    let recovery_dir = Some(NativePath::new(
        &discovery.git_dir.join(format!("gitnova-conflict-{token}")),
    ));
    Ok(MergeActionPreview {
        action: action.clone(),
        state,
        diff,
        paths,
        recovery_dir,
        expected_tree: Some(after),
        token,
        blocked_reason: blocked,
        truncated: false,
    })
}

pub(super) fn apply(
    git: &GitCli,
    root: &Path,
    action: &MergeAction,
    review: MergeActionPreview,
) -> Result<()> {
    let directory = git.discover(root)?.git_dir;
    recovery::save_at(git, root, &directory, &review)?;
    let latest = git.preview_merge_action(root, action)?;
    if latest.token != review.token || latest.blocked_reason.is_some() {
        return Err(AppError::new(
            "stale_preview",
            "Files changed while capturing recovery data. Nothing was replaced; review again.",
        ));
    }
    git.apply_conflict_resolution(root, action, true)
}
