//! Remove only the selected missing registration by archiving its metadata.
use crate::{working::run_ref_transaction, GitCli, RefReader, RepositoryReader};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
type MetadataFiles = Vec<(PathBuf, Vec<u8>)>;
fn err(s: &str) -> AppError {
    AppError::new("worktree_request", s)
}
pub(crate) fn collect(
    dir: &Path,
    base: &Path,
    items: &mut Vec<(PathBuf, Vec<u8>)>,
    total: &mut usize,
) -> Result<()> {
    if items.len() > 1024 {
        return Err(err("Worktree metadata exceeds the prune review budget."));
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(err(
                "Symbolic-link worktree metadata needs manual inspection.",
            ));
        }
        if meta.is_dir() {
            collect(&path, base, items, total)?;
        } else if meta.is_file() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.ends_with(".lock") || name == "locked" {
                return Err(err("Locked or active worktree metadata cannot be pruned."));
            }
            let mut bytes = vec![];
            fs::File::open(&path)?
                .take(64 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            *total += bytes.len();
            if *total > 64 * 1024 * 1024 {
                return Err(err("Worktree metadata exceeds the prune review budget."));
            }
            items.push((
                path.strip_prefix(base)
                    .map_err(|_| err("Invalid worktree metadata path."))?
                    .into(),
                bytes,
            ));
        } else {
            return Err(err("Unexpected worktree metadata needs manual inspection."));
        }
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(())
}
fn metadata(
    git: &GitCli,
    root: &Path,
    r: &WorktreeMaintenanceRequest,
) -> Result<(PathBuf, MetadataFiles)> {
    let d = git.discover(root)?;
    let wanted = r.path.to_path()?;
    if !wanted.is_absolute() || fs::symlink_metadata(&wanted).is_ok() {
        return Err(err(
            "Prune requires a missing checkout; existing folders are never removed.",
        ));
    }
    let trees = git.linked_worktrees(root)?;
    if trees.len() > 256 {
        return Err(err("Prune exceeds the 256-checkout review budget."));
    }
    let mut selected = None;
    let base = d.common_dir.join("worktrees");
    if fs::symlink_metadata(&base)?.file_type().is_symlink() {
        return Err(err(
            "Symbolic-link worktree metadata needs manual inspection.",
        ));
    }
    for entry in fs::read_dir(&base)? {
        let admin = entry?.path();
        let m = fs::symlink_metadata(&admin)?;
        if !m.is_dir() || m.file_type().is_symlink() {
            return Err(err("Unexpected worktree metadata needs manual inspection."));
        }
        let locator = admin.join("gitdir");
        let m = fs::symlink_metadata(&locator)?;
        if !m.is_file() || m.file_type().is_symlink() || m.len() > 65536 {
            return Err(err("Invalid worktree locator; repair before pruning."));
        }
        let raw = fs::read(&locator)?;
        let path = crate::path_from_bytes(raw.strip_suffix(b"\n").unwrap_or(&raw).to_vec())?;
        let checkout = path
            .parent()
            .ok_or_else(|| err("Invalid worktree locator; repair before pruning."))?;
        if checkout == wanted {
            if selected.is_some() {
                return Err(err("Ambiguous worktree metadata; inspect with Git."));
            }
            selected = Some(admin);
        } else {
            // Native prune considers all metadata. No other missing/broken entry may be touched.
            let other = git.discover(checkout).map_err(|_| {
                err("Repair or inspect the other missing worktrees before selected pruning.")
            })?;
            if other.git_dir != admin.canonicalize()?
                || other.common_dir != d.common_dir
                || !trees
                    .iter()
                    .any(|t| t.path.to_path().ok().as_deref() == Some(checkout))
            {
                return Err(err(
                    "Repair or inspect the other missing worktrees before selected pruning.",
                ));
            }
        }
    }
    let admin = selected.ok_or_else(|| err("Choose the registered missing checkout to prune."))?;
    let mut items = vec![];
    collect(&admin, &admin, &mut items, &mut 0)?;
    Ok((admin, items))
}
fn canonical_request(r: &WorktreeMaintenanceRequest) -> Result<WorktreeMaintenanceRequest> {
    let path = r.path.to_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| err("Choose an absolute missing checkout path."))?
        .canonicalize()?;
    let leaf = path
        .file_name()
        .ok_or_else(|| err("Choose an absolute missing checkout path."))?;
    let mut r = r.clone();
    r.path = NativePath::new(&parent.join(leaf));
    Ok(r)
}
impl GitCli {
    pub(crate) fn preview_prune_worktree(
        &self,
        root: &Path,
        r: &WorktreeMaintenanceRequest,
    ) -> Result<WorktreeMaintenancePreview> {
        let selected = canonical_request(r)?;
        let r = &selected;
        if !r.destination.is_empty() || !r.reason.is_empty() {
            return Err(err("Prune does not accept a destination or lock reason."));
        }
        let (d, state) = self.write_state(root)?;
        let trees = self.linked_worktrees(root)?;
        let tree = trees
            .iter()
            .find(|t| t.path == r.path)
            .ok_or_else(|| err("Choose a registered missing checkout."))?;
        let (_admin, items) = metadata(self, root, r)?;
        if tree.locked
            || !tree.missing
            || !tree.prunable
            || tree.bare
            || d.root.as_ref() == Some(&r.path.to_path()?)
        {
            return Err(err(
                "Only an unlocked prunable missing linked checkout can be pruned.",
            ));
        }
        let hashes = items
            .iter()
            .map(|(p, b)| (NativePath::new(p), format!("{:x}", Sha256::digest(b))))
            .collect::<Vec<_>>();
        let token = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    r,
                    state,
                    &trees,
                    hashes,
                    self.refs(root)?,
                    self.config_snapshot(root)?
                ))
                .map_err(|e| err(&e.to_string()))?
            )
        );
        let recovery =
            NativePath::new(&d.common_dir.join(format!("gitnova-worktree-prune-{token}")));
        Ok(WorktreeMaintenancePreview{request:r.clone(),tree:tree.clone(),affected:vec![tree.clone()],token,recovery_dir:Some(recovery.clone()),blocked_reason:recovery.to_path()?.exists().then(||"This prune already has recovery metadata. Inspect its prior result before retrying.".into())})
    }
    pub(crate) fn prune_worktree(
        &self,
        root: &Path,
        r: &WorktreeMaintenanceRequest,
        token: &str,
    ) -> Result<()> {
        let selected = canonical_request(r)?;
        let r = &selected;
        let p = self.preview_prune_worktree(root, r)?;
        if p.token != token {
            return Err(err("Worktree metadata changed. Review maintenance again."));
        }
        if let Some(reason) = p.blocked_reason {
            return Err(err(&reason));
        }
        let (admin, items) = metadata(self, root, r)?;
        let other_trees = self
            .linked_worktrees(root)?
            .into_iter()
            .filter(|t| t.path != r.path)
            .collect::<Vec<_>>();
        let refs = serde_json::to_vec(&self.refs(root)?).map_err(|e| err(&e.to_string()))?;
        let (_, state) = self.write_state(root)?;
        let recovery = p
            .recovery_dir
            .ok_or_else(|| err("Missing prune recovery directory."))?
            .to_path()?;
        fs::create_dir(&recovery)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&recovery, fs::Permissions::from_mode(0o700))?;
        }
        for (path, bytes) in &items {
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
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        if self.preview_prune_worktree(root, r)?.token != token {
            return Err(err(
                "Worktree metadata changed while saving recovery. Nothing was pruned.",
            ));
        }
        if let Some(head) = &p.tree.head {
            crate::validate_oid(head)?;
            run_ref_transaction(root,&format!("start\ncreate refs/gitnova/recovery/worktree-prune-{token} {head}\nprepare\ncommit\n"))?;
        }
        // A global `git worktree prune` can remove another registration if an
        // unrelated disk disappears after preview. Move only the reviewed admin
        // directory, on the same filesystem, retaining the original metadata.
        fs::rename(&admin, recovery.join("registration")).map_err(|e| {
            AppError::new(
                "operation_unknown",
                format!(
                    "{e}. Inspect saved prune metadata at {} before retrying.",
                    recovery.display()
                ),
            )
        })?;
        let now = self.linked_worktrees(root)?;
        if admin.exists()
            || now.iter().any(|t| t.path == r.path)
            || serde_json::to_vec(&self.refs(root)?).map_err(|e| err(&e.to_string()))? != refs
            || self.write_state(root)?.1 != state
            || serde_json::to_vec(&now.iter().filter(|t| t.path != r.path).collect::<Vec<_>>())
                .map_err(|e| err(&e.to_string()))?
                != serde_json::to_vec(&other_trees).map_err(|e| err(&e.to_string()))?
        {
            return Err(AppError::new(
                "operation_unknown",
                "Prune result needs inspection; retained metadata and recovery ref remain.",
            ));
        }
        Ok(())
    }
}
