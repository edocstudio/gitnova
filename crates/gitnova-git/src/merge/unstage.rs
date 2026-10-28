//! Index-only operation membership changes. Never reset HEAD or the working tree.
use super::*;
use crate::working::{preserve_index_timestamp, publish_reviewed_index, run_write};
pub(super) fn preview(
    git: &GitCli,
    root: &Path,
    action: &MergeAction,
    own_lock: bool,
) -> Result<MergeActionPreview> {
    if action.choice.is_some() || action.content.is_some() || !action.message.is_empty() {
        return Err(AppError::new(
            "merge",
            "Unexpected fields for this merge action.",
        ));
    }
    let state = git.merge_state(root)?;
    if state.phase != "pending" {
        return Err(AppError::new(
            "merge_blocked",
            state
                .blocked_reason
                .clone()
                .unwrap_or_else(|| "No owned merge is ready for this action.".into()),
        ));
    }
    let marker = load_marker(git, root)?
        .ok_or_else(|| AppError::new("merge_metadata", "Missing owned operation"))?;
    let d = git.discover(root)?;
    let changes = git.working_changes(root)?;
    let files: Vec<_> = changes
        .files
        .iter()
        .filter(|f| f.staged && action.path.as_ref().is_none_or(|p| p == &f.path))
        .collect();
    if files.is_empty() {
        return Err(AppError::new(
            "stale_preview",
            "No staged paths match this unstage selection. Refresh and review again.",
        ));
    }
    let mut blocked = None;
    if fs::symlink_metadata(d.git_dir.join("index"))?
        .file_type()
        .is_symlink()
    {
        blocked = Some("A symlink index needs manual inspection before unstaging.".into());
    }
    if !state.conflicts.is_empty() {
        blocked = Some("Resolve conflicts before unstaging operation results.".into());
    }
    if hidden_entries(git, root)? || files.iter().any(|f| f.blocked_reason.is_some()) {
        blocked = Some(
            "Hidden index entries or submodule pointers need separate review before unstaging."
                .into(),
        );
    }
    if (!own_lock && d.git_dir.join("index.lock").exists())
        || d.git_dir.join("HEAD.lock").exists()
        || d.common_dir.join("config.lock").exists()
    {
        blocked = Some("Another Git operation holds a native lock. Wait and review again.".into());
    }
    for name in ["rebase-apply", "rebase-merge", "sequencer"] {
        if d.git_dir.join(name).exists() {
            blocked = Some(
                "Another Git operation is active. Inspect it before changing this operation."
                    .into(),
            );
        }
    }
    let paths: Vec<_> = files.into_iter().map(|f| f.path.clone()).collect();
    let mut args: Vec<OsString> = vec![
        "diff".into(),
        "--cached".into(),
        "--no-ext-diff".into(),
        "--no-textconv".into(),
        "--no-renames".into(),
        marker.original_oid.clone().into(),
        "--".into(),
    ];
    for p in &paths {
        safe_path(root, p)?;
        args.push(p.to_path()?.into_os_string());
    }
    let output = git.execute(
        root,
        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
        512 * 1024,
        true,
    )?;
    let tree = if state.conflicts.is_empty() {
        Some(index_tree(git, root)?.1)
    } else {
        None
    };
    let token = digest(
        &serde_json::to_vec(&(action, &state, &paths, &tree, changes.token))
            .map_err(|e| AppError::new("snapshot", e.to_string()))?,
    );
    let recovery_dir = Some(NativePath::new(
        &marker
            .session_dir
            .to_path()?
            .join(format!("unstage-{token}")),
    ));
    Ok(MergeActionPreview {
        action: action.clone(),
        state,
        diff: String::from_utf8_lossy(&output.bytes).into_owned(),
        paths,
        recovery_dir,
        expected_tree: tree,
        token,
        truncated: output.truncated,
        blocked_reason: blocked,
    })
}
pub(super) fn apply(git: &GitCli, root: &Path, action: &MergeAction, token: &str) -> Result<()> {
    let p = preview(git, root, action, false)?;
    if p.token != token {
        return Err(AppError::new(
            "stale_preview",
            "Operation index, files or configuration changed. Review again.",
        ));
    }
    if let Some(reason) = p.blocked_reason {
        return Err(AppError::new("merge_blocked", reason));
    }
    let d = git.discover(root)?;
    let original = read_file(&d.git_dir.join("index"), 64 * 1024 * 1024)?;
    let temporary = tempfile::NamedTempFile::new_in(&d.git_dir)?.into_temp_path();
    fs::write(&temporary, &original)?;
    preserve_index_timestamp(&d.git_dir.join("index"), &temporary)?;
    let directory = p
        .recovery_dir
        .as_ref()
        .ok_or_else(|| AppError::new("merge_metadata", "Missing recovery directory"))?
        .to_path()?;
    recovery::private_dir(&directory)?;
    recovery::private_file(&directory.join("index"), &original)?;
    recovery::private_file(
        &directory.join("preview.json"),
        &serde_json::to_vec(&p).map_err(|e| AppError::new("snapshot", e.to_string()))?,
    )?;
    let tree = p.expected_tree.as_deref().ok_or_else(|| {
        AppError::new(
            "merge_blocked",
            "Resolve conflicts before unstaging operation results.",
        )
    })?;
    let reference = format!("refs/gitnova/recovery/unstage-{token}");
    let prefix = || {
        vec![
            OsString::from("-c"),
            OsString::from("core.hooksPath=/dev/null"),
        ]
    };
    let mut args = prefix();
    args.push("write-tree".into());
    run_write(root, &args, Some(&temporary))?;
    let mut args = prefix();
    args.extend([
        "update-ref".into(),
        reference.clone().into(),
        tree.into(),
        "".into(),
    ]);
    run_write(root, &args, None)?;
    recovery::private_file(&directory.join("recovery-ref"), reference.as_bytes())?;
    let mut args = prefix();
    args.extend([
        "reset".into(),
        "--quiet".into(),
        "--no-refresh".into(),
        p.state
            .original_oid
            .as_ref()
            .ok_or_else(|| AppError::new("merge_metadata", "Missing original HEAD"))?
            .into(),
        "--".into(),
    ]);
    for path in &p.paths {
        args.push(path.to_path()?.into_os_string());
    }
    run_write(root, &args, Some(&temporary))?;
    publish_reviewed_index(git, root, &original, &temporary, || {
        let latest = preview(git, root, action, true)?;
        if latest.token != token || latest.blocked_reason.is_some() {
            return Err(AppError::new(
                "stale_preview",
                "Operation index, files or configuration changed. Review again.",
            ));
        }
        Ok(())
    })
}
