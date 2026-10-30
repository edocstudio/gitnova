//! Reattach a checkout moved outside GitNova, without moving files or branches.
use crate::{working::run_offline_write, GitCli, RepositoryReader};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
fn err(message: &str) -> AppError {
    AppError::new("worktree_request", message)
}
fn bytes(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(err("Worktree metadata must be a regular file."));
    }
    let mut value = Vec::new();
    fs::File::open(path)?
        .take((limit + 1) as u64)
        .read_to_end(&mut value)?;
    if value.len() > limit {
        return Err(err("Worktree metadata exceeds the complete review budget."));
    }
    Ok(value)
}
fn destination(
    git: &GitCli,
    root: &Path,
    r: &WorktreeMaintenanceRequest,
) -> Result<(PathBuf, PathBuf, Vec<u8>)> {
    let wanted = PathBuf::from(&r.destination);
    if r.destination.len() > 4096
        || !wanted.is_absolute()
        || r.destination.chars().any(char::is_control)
        || wanted.components().any(|p| {
            matches!(
                p,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(err(
            "Enter the absolute existing folder of the moved checkout.",
        ));
    }
    let canonical = wanted.canonicalize()?;
    if !canonical.is_dir()
        || wanted
            .ancestors()
            .any(|p| fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
    {
        return Err(err(
            "Use the canonical moved checkout folder without symbolic links.",
        ));
    }
    let gitfile = bytes(&canonical.join(".git"), 65536)?;
    let locator = std::str::from_utf8(&gitfile)
        .map_err(|_| err("The moved checkout locator is not UTF-8."))?
        .strip_prefix("gitdir: ")
        .ok_or_else(|| err("Choose a linked checkout with its original .git locator."))?
        .trim_end_matches(['\n', '\r']);
    let locator = Path::new(locator);
    let admin = if locator.is_absolute() {
        locator.to_path_buf()
    } else {
        canonical.join(locator)
    }
    .canonicalize()?;
    let common = git.discover(root)?.common_dir;
    let relative = admin
        .strip_prefix(common.join("worktrees"))
        .map_err(|_| err("The moved checkout belongs to another repository."))?;
    if relative.components().count() != 1 || fs::symlink_metadata(&admin)?.file_type().is_symlink()
    {
        return Err(err(
            "The moved checkout locator has an unexpected administrative path.",
        ));
    }
    let old = bytes(&admin.join("gitdir"), 65536)?;
    let old_path = crate::path_from_bytes(old.strip_suffix(b"\n").unwrap_or(&old).to_vec())?;
    if old_path != r.path.to_path()?.join(".git") {
        return Err(err(
            "The moved folder does not identify the selected missing checkout.",
        ));
    }
    Ok((canonical, admin, gitfile))
}
impl GitCli {
    pub(crate) fn preview_reattach_worktree(
        &self,
        root: &Path,
        r: &WorktreeMaintenanceRequest,
    ) -> Result<WorktreeMaintenancePreview> {
        let (_, state) = self.write_state(root)?;
        let d = self.discover(root)?;
        let old = r.path.to_path()?;
        if fs::symlink_metadata(&old).is_ok() {
            return Err(err(
                "The original worktree folder still exists. It will not be replaced or detached.",
            ));
        }
        let trees = self.linked_worktrees(root)?;
        let tree = trees
            .iter()
            .find(|t| t.path == r.path)
            .ok_or_else(|| err("Choose the registered missing worktree."))?;
        let (new, admin, gitfile) = destination(self, root, r)?;
        if tree.bare || admin == d.git_dir || admin == d.common_dir {
            return Err(err(
                "Choose a moved linked checkout, not the main repository.",
            ));
        }
        for name in ["HEAD.lock", "index.lock", "locked.lock"] {
            if admin.join(name).exists() {
                return Err(err(
                    "Another Git operation holds a worktree lock. Wait for its owner.",
                ));
            }
        }
        if trees
            .iter()
            .any(|t| t.path.to_path().is_ok_and(|p| p == new))
        {
            return Err(err("The destination is already a registered checkout."));
        }
        let mut snapshots = Vec::new();
        for t in &trees {
            if t.path == r.path {
                continue;
            }
            let p = t.path.to_path()?;
            // Native repair also checks forward locators; expose and bind every location.
            if !p.is_dir() || fs::symlink_metadata(&p)?.file_type().is_symlink() {
                return Err(err("Inspect other missing or symbolic-link worktrees before reattaching this checkout."));
            }
            if p.join(".git").is_file() {
                snapshots.push((t.path.clone(), bytes(&p.join(".git"), 65536)?));
            }
        }
        if trees.len() > 256 {
            return Err(err(
                "Worktree repair exceeds the 256-checkout review budget.",
            ));
        }
        let head = bytes(&admin.join("HEAD"), 65536)?;
        let index = bytes(&admin.join("index"), 64 * 1024 * 1024)?;
        let forward = bytes(&admin.join("gitdir"), 65536)?;
        let snapshot = serde_json::to_vec(&(
            r,
            &state,
            &trees,
            &snapshots,
            &gitfile,
            &head,
            format!("{:x}", Sha256::digest(&index)),
            &forward,
            self.config_snapshot(root)?,
        ))
        .map_err(|e| err(&e.to_string()))?;
        let token = format!("{:x}", Sha256::digest(snapshot));
        let recovery = d.git_dir.join(format!("gitnova-worktree-reattach-{token}"));
        let blocked_reason = recovery.exists().then(|| {
            "This repair has recovery metadata. Inspect its previous result before retrying.".into()
        });
        Ok(WorktreeMaintenancePreview {
            request: r.clone(),
            tree: tree.clone(),
            affected: trees,
            recovery_dir: Some(NativePath::new(&recovery)),
            token,
            blocked_reason,
        })
    }
    pub(crate) fn reattach_worktree(
        &self,
        root: &Path,
        r: &WorktreeMaintenanceRequest,
        token: &str,
    ) -> Result<()> {
        let p = self.preview_reattach_worktree(root, r)?;
        if p.token != token {
            return Err(err("Worktree metadata changed. Review maintenance again."));
        }
        if let Some(reason) = p.blocked_reason {
            return Err(err(&reason));
        }
        let (new, admin, gitfile) = destination(self, root, r)?;
        let _guard = crate::working::OperationLock::acquire(admin.join("gitnova-operation.lock"))?;
        if self.preview_reattach_worktree(root, r)?.token != token {
            return Err(err("Worktree metadata changed while acquiring its guard."));
        }
        let before = self.head_index_snapshot(root)?;
        let head = bytes(&admin.join("HEAD"), 65536)?;
        let index = bytes(&admin.join("index"), 64 * 1024 * 1024)?;
        let recovery = p
            .recovery_dir
            .as_ref()
            .ok_or_else(|| err("Missing repair recovery location."))?
            .to_path()?;
        fs::create_dir(&recovery)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&recovery, fs::Permissions::from_mode(0o700))?;
        }
        for (name, value) in [
            ("gitfile", gitfile),
            ("gitdir", bytes(&admin.join("gitdir"), 65536)?),
            ("HEAD", head.clone()),
            ("index", index.clone()),
        ] {
            let mut options = fs::OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(recovery.join(name))?;
            file.write_all(&value)?;
            file.sync_all()?;
        }
        let result = (|| {
            if self.preview_reattach_worktree(root, r)?.token != token {
                return Err(err("Worktree metadata changed while saving recovery."));
            }
            run_offline_write(
                root,
                &[
                    "worktree".into(),
                    "repair".into(),
                    "--".into(),
                    new.as_os_str().into(),
                ],
            )?;
            let now = self.linked_worktrees(root)?;
            let selected = now
                .iter()
                .find(|t| t.path.to_path().is_ok_and(|v| v == new))
                .ok_or_else(|| err("The moved checkout was not reattached."))?;
            if selected.head != p.tree.head
                || selected.branch != p.tree.branch
                || selected.locked != p.tree.locked
                || bytes(&admin.join("HEAD"), 65536)? != head
                || bytes(&admin.join("index"), 64 * 1024 * 1024)? != index
                || self.head_index_snapshot(root)? != before
            {
                return Err(err("Repair changed unexpected checkout state."));
            }
            if self.discover(&new)?.common_dir != self.discover(root)?.common_dir {
                return Err(err(
                    "Repair did not restore the expected repository identity.",
                ));
            }
            Ok(())
        })();
        result.map_err(|e:AppError|AppError::new("operation_unknown",format!("{} Inspect worktree locators and recovery folder {} before retrying; no rollback was attempted.",e.message,recovery.display())))
    }
}
