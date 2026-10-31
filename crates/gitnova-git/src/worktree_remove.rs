//! One clean linked checkout, with retained history and exclusive native guards.
use crate::{
    merge::scratch::Scratch, working::run_offline_write, GitCli, RefReader, RepositoryReader,
    StatusReader,
};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn directory_identity(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(AppError::new(
            "worktree_blocked",
            "The worktree Git identity changed. Rediscover it before removal.",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("{}:{}", metadata.dev(), metadata.ino()))
    }
    #[cfg(not(unix))]
    {
        crate::file_identity::directory(path)
    }
}
pub fn membership_token(
    native: &str,
    worktree_id: &Option<String>,
    memberships: &[WorktreeMembershipImpact],
) -> Result<String> {
    Ok(digest(
        &serde_json::to_vec(&(native, worktree_id, memberships))
            .map_err(|e| AppError::new("snapshot", e.to_string()))?,
    ))
}
struct RemovalLock {
    path: PathBuf,
    marker: Vec<u8>,
}
impl RemovalLock {
    fn acquire(path: PathBuf) -> Result<Self> {
        let marker = format!("GitNova worktree removal {}\n", uuid::Uuid::new_v4()).into_bytes();
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists { AppError::new("worktree_blocked", "A native Git lock exists. Wait for its owner; locks are never removed automatically.") } else { e.into() })?;
        let guard = Self { path, marker };
        file.write_all(&guard.marker)?;
        file.sync_all()?;
        Ok(guard)
    }
}
impl Drop for RemovalLock {
    fn drop(&mut self) {
        if fs::read(&self.path).is_ok_and(|bytes| bytes == self.marker) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
impl GitCli {
    pub fn linked_worktrees(&self, root: &Path) -> Result<Vec<LinkedWorktree>> {
        let raw = self
            .read(
                root,
                &[
                    "worktree".as_ref(),
                    "list".as_ref(),
                    "--porcelain".as_ref(),
                    "-z".as_ref(),
                ],
                1024 * 1024,
            )?
            .bytes;
        let mut trees = vec![];
        let mut current: Option<LinkedWorktree> = None;
        for field in raw.split(|b| *b == 0) {
            if let Some(value) = field.strip_prefix(b"worktree ") {
                if let Some(tree) = current.take() {
                    trees.push(tree);
                }
                current = Some(LinkedWorktree {
                    missing: false,
                    path: NativePath::new(&crate::path_from_bytes(value.to_vec())?),
                    head: None,
                    branch: None,
                    bare: false,
                    locked: false,
                    prunable: false,
                });
            } else if let Some(tree) = current.as_mut() {
                if let Some(value) = field.strip_prefix(b"HEAD ") {
                    tree.head = Some(String::from_utf8_lossy(value).into());
                }
                if let Some(value) = field.strip_prefix(b"branch ") {
                    tree.branch = Some(
                        String::from_utf8_lossy(value)
                            .trim_start_matches("refs/heads/")
                            .into(),
                    );
                }
                tree.bare |= field == b"bare";
                tree.locked |= field.starts_with(b"locked");
                tree.prunable |= field.starts_with(b"prunable");
            }
        }
        if let Some(tree) = current {
            trees.push(tree);
        }
        for tree in &mut trees {
            tree.missing = fs::symlink_metadata(tree.path.to_path()?)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound);
        }
        Ok(trees)
    }
    fn removal_cleanliness(&self, target: &Path) -> Result<Option<String>> {
        let (_, _) = self.write_state(target)?;
        if self.status(target)?.dirty {
            return Ok(Some(
                "This worktree has uncommitted work. Commit or preserve it before removal.".into(),
            ));
        }
        let flags = self
            .read(
                target,
                &["ls-files".as_ref(), "-v".as_ref(), "-z".as_ref()],
                4 * 1024 * 1024,
            )?
            .bytes;
        if flags
            .split(|b| *b == 0)
            .filter(|v| !v.is_empty())
            .any(|v| v[0].is_ascii_lowercase() || v[0] == b'S')
        {
            return Ok(Some(
                "Hidden or sparse index entries need Git inspection before removing this worktree."
                    .into(),
            ));
        }
        let entries = self
            .read(
                target,
                &["ls-files".as_ref(), "--stage".as_ref(), "-z".as_ref()],
                4 * 1024 * 1024,
            )?
            .bytes;
        if entries
            .split(|b| *b == 0)
            .any(|v| v.starts_with(b"160000 "))
        {
            return Ok(Some("Worktrees containing submodules need separate recovery and cannot be removed here.".into()));
        }
        let ignored = self
            .read(
                target,
                &[
                    "ls-files".as_ref(),
                    "--others".as_ref(),
                    "--ignored".as_ref(),
                    "--exclude-standard".as_ref(),
                    "-z".as_ref(),
                ],
                4 * 1024 * 1024,
            )?
            .bytes;
        if !ignored.is_empty() {
            return Ok(Some(
                "Ignored files are still local work. Preserve them before removing this worktree."
                    .into(),
            ));
        }
        let head = self.text(target, &["rev-parse", "--verify", "HEAD"])?;
        let scratch = Scratch::new(self, target)?;
        if !scratch.worktree_is_clean(target, head.trim())? {
            return Ok(Some("Tracked file contents differ from HEAD when checked without the index cache. Preserve this work before removal.".into()));
        }
        Ok(None)
    }
    pub fn preview_remove_worktree(
        &self,
        root: &Path,
        request: &WorktreeRemoveRequest,
    ) -> Result<WorktreeRemovePreview> {
        let uuid = uuid::Uuid::parse_str(&request.id).map_err(|_| {
            AppError::new(
                "worktree_request",
                "Review this worktree removal before confirming.",
            )
        })?;
        let source = self.discover(root)?;
        let (_, source_state) = self.write_state(root)?;
        let trees = self.linked_worktrees(root)?;
        let tree = trees
            .iter()
            .find(|tree| tree.path == request.path)
            .ok_or_else(|| {
                AppError::new(
                    "worktree_request",
                    "Choose a discovered linked worktree from this repository.",
                )
            })?;
        let target = request.path.to_path()?;
        let mut blocked = None;
        let mut git_dir = None;
        let mut target_state = None;
        if tree.locked || tree.prunable || tree.bare {
            blocked = Some(
                "Locked, prunable or bare worktrees require Git inspection before removal.".into(),
            );
        }
        if source.root.as_ref() == Some(&target) {
            blocked = Some("Open another worktree before removing this checkout. The active worktree is never removed here.".into());
        }
        let metadata = fs::symlink_metadata(&target);
        if !metadata
            .as_ref()
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        {
            blocked = Some("This worktree folder is missing or replaced. Repair its metadata with Git before removal.".into());
        }
        let ref_storage =
            self.optional_text(root, &["config", "--get", "extensions.refStorage"])?;
        if ref_storage
            .as_deref()
            .is_some_and(|value| value.trim() != "files")
        {
            blocked = Some("Worktree removal currently requires Git files ref storage.".into());
        }
        let mut target_config = None;
        if metadata
            .as_ref()
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        {
            let discovery = self.discover(&target)?;
            git_dir = Some(NativePath::new(&discovery.git_dir));
            if discovery.common_dir != source.common_dir
                || discovery.root.as_deref() != Some(target.as_path())
            {
                blocked =
                    Some("The worktree Git identity changed. Rediscover it before removal.".into());
            }
            if discovery.git_dir == source.common_dir {
                blocked = Some("The main worktree cannot be removed here.".into());
            }
            for lock in [
                discovery.git_dir.join("HEAD.lock"),
                discovery.git_dir.join("index.lock"),
            ] {
                if lock.exists() {
                    blocked = Some("A native Git lock exists. Wait for its owner; locks are never removed automatically.".into());
                }
            }
            if let Some(branch) = &tree.branch {
                if source
                    .common_dir
                    .join(format!("refs/heads/{branch}.lock"))
                    .exists()
                {
                    blocked = Some("A native Git lock exists. Wait for its owner; locks are never removed automatically.".into());
                }
            }
            if blocked.is_none() {
                target_state = Some(self.write_state(&target)?.1);
                if let Some(reason) = self.removal_cleanliness(&target)? {
                    blocked = Some(reason);
                }
            }
            target_config = Some(self.config_snapshot(&target)?);
        }
        let recovery_ref = format!("refs/gitnova/recovery/worktree-{}", uuid.simple());
        if self
            .optional_text(root, &["rev-parse", "--verify", "--quiet", &recovery_ref])?
            .is_some()
        {
            blocked = Some("This removal already has recovery metadata. Inspect its previous result before retrying.".into());
        }
        let head = tree.head.clone();
        if head.is_none() {
            blocked = Some(
                "This worktree has no inspectable commit. Recover it with Git before removal."
                    .into(),
            );
        }
        if let Some(head) = &head {
            crate::validate_oid(head)?;
        }
        let folder_identity = if metadata
            .as_ref()
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        {
            Some(directory_identity(&target)?)
        } else {
            None
        };
        let admin_identity = git_dir
            .as_ref()
            .map(|path| path.to_path().and_then(|path| directory_identity(&path)))
            .transpose()?;
        let token = digest(
            &serde_json::to_vec(&(
                request,
                folder_identity,
                admin_identity,
                &trees,
                &source_state,
                &target_state,
                &target_config,
                self.config_snapshot(root)?,
                self.refs(root)?,
                &blocked,
            ))
            .map_err(|e| AppError::new("snapshot", e.to_string()))?,
        );
        Ok(WorktreeRemovePreview {
            worktree_id: None,
            version: 1,
            request: request.clone(),
            source: NativePath::new(source.root.as_ref().unwrap()),
            git_dir,
            common_dir: NativePath::new(&source.common_dir),
            head,
            branch: tree.branch.clone(),
            recovery_ref,
            memberships: vec![],
            git_token: token.clone(),
            token,
            blocked_reason: blocked,
        })
    }
    pub fn remove_worktree(
        &self,
        root: &Path,
        request: &WorktreeRemoveRequest,
        token: &str,
    ) -> Result<()> {
        let p = self.preview_remove_worktree(root, request)?;
        if p.git_token != token {
            return Err(AppError::new(
                "stale_preview",
                "Worktree contents, identity or branch changed. Review removal again.",
            ));
        }
        if let Some(reason) = p.blocked_reason {
            return Err(AppError::new("worktree_blocked", reason));
        }
        let target = p.request.path.to_path()?;
        let admin = p
            .git_dir
            .as_ref()
            .ok_or_else(|| AppError::new("worktree_request", "Missing linked worktree identity."))?
            .to_path()?;
        let folder_identity = directory_identity(&target)?;
        let admin_identity = directory_identity(&admin)?;
        let _head_lock = RemovalLock::acquire(admin.join("HEAD.lock"))?;
        let _index_lock = RemovalLock::acquire(admin.join("index.lock"))?;
        let branch_lock = p
            .branch
            .as_ref()
            .map(|branch| {
                p.common_dir
                    .to_path()
                    .map(|common| common.join(format!("refs/heads/{branch}.lock")))
            })
            .transpose()?;
        let _branch_lock = if let Some(path) = branch_lock {
            fs::create_dir_all(path.parent().unwrap())?;
            Some(RemovalLock::acquire(path)?)
        } else {
            None
        };
        let head = p
            .head
            .as_deref()
            .ok_or_else(|| AppError::new("worktree_request", "Missing target commit."))?;
        run_offline_write(
            root,
            &[
                "update-ref".into(),
                "--no-deref".into(),
                p.recovery_ref.clone().into(),
                head.into(),
                "".into(),
            ],
        )?;
        if !directory_identity(&target).is_ok_and(|actual| actual == folder_identity)
            || !directory_identity(&admin).is_ok_and(|actual| actual == admin_identity)
            || !self
                .removal_cleanliness(&target)
                .is_ok_and(|reason| reason.is_none())
            || !self.status(&target).is_ok_and(|status| {
                status.head.as_deref() == Some(head) && status.branch == p.branch
            })
        {
            return Err(AppError::new("operation_unknown", "Commit retained, but worktree contents changed before removal. Inspect its files and recovery ref before retrying."));
        }
        run_offline_write(
            root,
            &[
                "-c".into(),
                "submodule.recurse=false".into(),
                "worktree".into(),
                "remove".into(),
                "--".into(),
                target.into_os_string(),
            ],
        )?;
        if self
            .linked_worktrees(root)?
            .iter()
            .any(|tree| tree.path == p.request.path)
            || !matches!(fs::symlink_metadata(p.request.path.to_path()?), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
        {
            return Err(AppError::new("operation_unknown", "Native worktree removal needs inspection. The folder or metadata may remain; recovery is retained."));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removal_guard_never_unlinks_a_replacement_owners_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("HEAD.lock");
        let guard = RemovalLock::acquire(path.clone()).unwrap();
        assert!(RemovalLock::acquire(path.clone()).is_err());
        fs::remove_file(&path).unwrap();
        fs::write(&path, "new native owner").unwrap();
        drop(guard);
        assert_eq!(fs::read_to_string(path).unwrap(), "new native owner");
    }
}
