//! Reviewed move of a clean, non-current linked checkout, preserving its identity.
use crate::{
    working::{run_offline_write, run_ref_transaction},
    GitCli, RefReader, RepositoryReader, StatusReader,
};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path};
fn err(s: &str) -> AppError {
    AppError::new("worktree_request", s)
}
fn nested(dir: &Path, root: &Path, count: &mut usize) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        *count += 1;
        if *count > 20000 {
            return Err(err("Worktree move inspection exceeds its file budget. Inspect the checkout separately."));
        }
        let entry = entry?;
        if entry.file_name() == ".git" {
            if dir != root {
                return Err(err("Nested repositories need a separately reviewed move."));
            }
            continue;
        }
        if entry.file_type()?.is_dir() {
            nested(&entry.path(), root, count)?;
        }
    }
    Ok(())
}
impl GitCli {
    pub(crate) fn preview_move_worktree(
        &self,
        root: &Path,
        r: &WorktreeMaintenanceRequest,
    ) -> Result<WorktreeMaintenancePreview> {
        if r.kind != "move" || !r.reason.is_empty() {
            return Err(err(
                "Choose a linked worktree and a new destination folder.",
            ));
        }
        let from = r.path.to_path()?;
        if fs::symlink_metadata(&from)?.file_type().is_symlink() {
            return Err(err("Move the actual checkout folder, not a symbolic link."));
        }
        let from = from.canonicalize()?;
        let input = Path::new(&r.destination);
        if !input.is_absolute()
            || input.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err(err(
                "Choose an absolute destination without parent-directory components.",
            ));
        }
        let parent = input
            .parent()
            .ok_or_else(|| err("Choose a destination inside an existing parent folder."))?
            .canonicalize()?;
        let destination = parent.join(
            input
                .file_name()
                .ok_or_else(|| err("Choose a new destination folder."))?,
        );
        if match fs::symlink_metadata(&destination) {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.into()),
        } {
            return Err(err(
                "The move destination already exists. Existing folders are never replaced.",
            ));
        }
        let (d, state) = self.write_state(root)?;
        let selected = self.discover(&from)?;
        if selected.common_dir != d.common_dir
            || selected.git_dir == d.common_dir
            || d.root.as_ref() == Some(&from)
            || selected.root.as_ref() != Some(&from)
        {
            return Err(err(
                "Move a non-current linked checkout from another worktree of the same repository.",
            ));
        }
        let inside = match self.discover(&parent) {
            Ok(_) => true,
            Err(e)
                if e.code == "git_exit_128"
                    && e.message.starts_with("fatal: not a git repository (or any") =>
            {
                false
            }
            Err(e) => return Err(e),
        };
        if parent.starts_with(&from) || parent.starts_with(&d.common_dir) || inside {
            return Err(err("Choose a move destination outside Git repositories and administrative directories."));
        }
        let trees = self.linked_worktrees(root)?;
        let tree = trees
            .iter()
            .find(|t| {
                t.path
                    .to_path()
                    .ok()
                    .and_then(|p| p.canonicalize().ok())
                    .as_ref()
                    == Some(&from)
            })
            .cloned()
            .ok_or_else(|| err("Select an existing registered linked worktree."))?;
        if tree.locked || tree.prunable || tree.missing || tree.bare {
            return Err(err(
                "Unlock and repair this linked worktree before moving it.",
            ));
        }
        let status = self.status(&from)?;
        if status.dirty {
            return Err(err(
                "Preserve or commit all work in the selected worktree before moving it.",
            ));
        }
        nested(&from, &from, &mut 0)?;
        let selected_state = self.head_index_snapshot(&from)?;
        let raw = self
            .read(
                &from,
                &[
                    "ls-tree".as_ref(),
                    "-r".as_ref(),
                    "-z".as_ref(),
                    "HEAD".as_ref(),
                ],
                4 * 1024 * 1024,
            )?
            .bytes;
        if raw.split(|b| *b == 0).any(|p| p.starts_with(b"160000 ")) {
            return Err(err(
                "Worktrees containing submodules need a separately reviewed move.",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if fs::metadata(&from)?.dev() != fs::metadata(&parent)?.dev() {
                return Err(err(
                    "Move within the same filesystem. Cross-device moves need separate recovery.",
                ));
            }
        }
        let mut items = Vec::new();
        crate::worktree_prune::collect(&selected.git_dir, &selected.git_dir, &mut items, &mut 0)?;
        let metadata = items
            .iter()
            .map(|(p, b)| (NativePath::new(p), format!("{:x}", Sha256::digest(b))))
            .collect::<Vec<_>>();
        let actual = WorktreeMaintenanceRequest {
            kind: "move".into(),
            path: NativePath::new(&from),
            destination: destination
                .to_str()
                .ok_or_else(|| err("Move destination must be UTF-8."))?
                .into(),
            reason: String::new(),
        };
        let token = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    &actual,
                    state,
                    selected_state,
                    &trees,
                    metadata,
                    self.refs(root)?,
                    self.config_snapshot(root)?
                ))
                .map_err(|e| err(&e.to_string()))?
            )
        );
        let recovery = d.common_dir.join(format!("gitnova-worktree-move-{token}"));
        Ok(WorktreeMaintenancePreview{request:actual,tree:tree.clone(),affected:vec![tree],token,recovery_dir:Some(NativePath::new(&recovery)),blocked_reason:recovery.exists().then(||"Move recovery metadata already exists. Inspect the earlier result before retrying.".into())})
    }
    pub(crate) fn move_worktree(
        &self,
        root: &Path,
        r: &WorktreeMaintenanceRequest,
        token: &str,
    ) -> Result<()> {
        let p = self.preview_move_worktree(root, r)?;
        if p.token != token {
            return Err(err("Worktree move state changed. Review the move again."));
        }
        if let Some(reason) = p.blocked_reason {
            return Err(err(&reason));
        }
        let from = p.request.path.to_path()?;
        let to = Path::new(&p.request.destination);
        let d = self.discover(&from)?;
        let state = self.head_index_snapshot(root)?;
        let selected_state = self.head_index_snapshot(&from)?;
        let refs = serde_json::to_vec(&self.refs(root)?).map_err(|e| err(&e.to_string()))?;
        let mut items = Vec::new();
        crate::worktree_prune::collect(&d.git_dir, &d.git_dir, &mut items, &mut 0)?;
        let recovery = p
            .recovery_dir
            .ok_or_else(|| err("Missing move recovery location."))?
            .to_path()?;
        fs::create_dir(&recovery)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&recovery, fs::Permissions::from_mode(0o700))?;
        }
        for (path, bytes) in items {
            let target = recovery.join(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(target)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        // Recovery creation intentionally invalidates its own preview blocker; the token
        // remains an exact snapshot and must still match before the native move.
        if self.preview_move_worktree(root, &p.request)?.token != token {
            return Err(err("Worktree move state changed. Review the move again."));
        }
        if let Some(head) = &p.tree.head {
            crate::validate_oid(head)?;
            run_ref_transaction(root,&format!("start\ncreate refs/gitnova/recovery/worktree-move-{token} {head}\nprepare\ncommit\n"))?;
        }
        run_offline_write(
            root,
            &[
                "worktree".into(),
                "move".into(),
                "--".into(),
                from.as_os_str().into(),
                to.as_os_str().into(),
            ],
        )
        .map_err(|e| {
            AppError::new(
                "operation_unknown",
                format!(
                    "{} Inspect worktree locations and retained move metadata before retrying.",
                    e.message
                ),
            )
        })?;
        let moved = self.discover(to)?;
        if from.exists()
            || moved.common_dir != d.common_dir
            || moved.git_dir != d.git_dir
            || self.head_index_snapshot(to)? != selected_state
            || self.head_index_snapshot(root)? != state
            || serde_json::to_vec(&self.refs(root)?).map_err(|e| err(&e.to_string()))? != refs
        {
            return Err(AppError::new(
                "operation_unknown",
                "Worktree move needs inspection. Retained metadata and branches remain available.",
            ));
        }
        Ok(())
    }
}
