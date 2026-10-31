//! Explicit everyday Git writes. Read previews never invoke hooks, filters or textconv.
mod amend;
mod batch;
mod discard;
mod gitlink;
mod hunk;
mod registration;
use crate::{path_from_bytes, GitCli, RepositoryReader, StatusReader};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
const FILE_LIMIT: u64 = 16 * 1024 * 1024;
const INDEX_LIMIT: u64 = 64 * 1024 * 1024;
const PATCH_LIMIT: usize = 512 * 1024;
pub trait WorkingTreeReader {
    fn working_changes(&self, path: &Path) -> Result<WorkingChanges>;
    fn file_preview(&self, path: &Path, file: &NativePath, staged: bool) -> Result<FilePreview>;
    fn commit_preview(&self, path: &Path) -> Result<CommitPreview>;
}
pub trait IndexWriter {
    fn change_index(&self, path: &Path, file: &NativePath, stage: bool, token: &str) -> Result<()>;
}
pub trait CommitWriter {
    fn create_commit(&self, path: &Path, message: &str, token: &str) -> Result<()>;
}
fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = vec![];
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(AppError::new(
            "safety_budget",
            "File or index exceeds this milestone's safe write budget",
        ));
    }
    Ok(bytes)
}
pub(crate) fn relative(file: &NativePath) -> Result<PathBuf> {
    let path = file.to_path()?;
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || path
            .components()
            .any(|c| c.as_os_str().eq_ignore_ascii_case(OsStr::new(".git")))
    {
        return Err(AppError::new(
            "path",
            "Expected an exact repository-relative file path",
        ));
    }
    Ok(path)
}
pub(crate) fn file_digest(root: &Path, path: &Path) -> Result<String> {
    let mut current = root.to_path_buf();
    for part in path.parent().unwrap_or(Path::new("")).components() {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(AppError::new(
                    "path",
                    "Cannot write through a symbolic-link directory",
                ))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
    }
    let path = root.join(path);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Ok(digest(
            NativePath::new(&fs::read_link(path)?).hex.as_bytes(),
        )),
        Ok(metadata) if metadata.is_file() => {
            let mut bytes = read_bounded(&path, FILE_LIMIT)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                bytes.extend(metadata.permissions().mode().to_le_bytes());
            }
            #[cfg(windows)]
            {
                bytes.push(u8::from(metadata.permissions().readonly()));
            }
            Ok(digest(&bytes))
        }
        Ok(_) => Err(AppError::new(
            "unsupported_write",
            "Directory and submodule writes are not enabled",
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("missing".into()),
        Err(e) => Err(e.into()),
    }
}
pub(crate) fn preserve_index_timestamp(source: &Path, destination: &Path) -> Result<()> {
    let modified = fs::metadata(source)?.modified()?;
    File::options()
        .write(true)
        .open(destination)?
        .set_times(fs::FileTimes::new().set_modified(modified))?;
    Ok(())
}
fn index_bytes(git_dir: &Path) -> Result<Vec<u8>> {
    match fs::symlink_metadata(git_dir.join("index")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
        Ok(_) => read_bounded(&git_dir.join("index"), INDEX_LIMIT),
    }
}
impl GitCli {
    pub(crate) fn write_state(&self, root: &Path) -> Result<(crate::Discovery, String)> {
        let d = self.discover(root)?;
        if d.root.is_none() {
            return Err(AppError::new(
                "bare",
                "Working changes are unavailable in a bare repository",
            ));
        }
        let mut bytes = index_bytes(&d.git_dir)?;
        bytes.extend(
            self.optional_text(root, &["rev-parse", "--verify", "--quiet", "HEAD"])?
                .unwrap_or_default()
                .as_bytes(),
        );
        bytes.push(0);
        bytes.extend(
            self.optional_text(root, &["symbolic-ref", "--quiet", "HEAD"])?
                .unwrap_or_default()
                .as_bytes(),
        );
        for marker in [
            "gitnova-merge.json",
            "MERGE_HEAD",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "rebase-merge",
            "rebase-apply",
            "sequencer",
        ] {
            if d.git_dir.join(marker).exists() {
                return Err(AppError::new("operation_in_progress", "Finish or recover the existing merge, rebase, cherry-pick or revert in Git before writing here"));
            }
        }
        Ok((d, digest(&bytes)))
    }
    fn preview_token(&self, root: &Path, file: &NativePath, staged: bool) -> Result<String> {
        let (_, state) = self.write_state(root)?;
        let path = relative(file)?;
        let content = file_digest(root, &path)?;
        Ok(digest(
            format!("{state}\0{}\0{staged}\0{content}", file.hex).as_bytes(),
        ))
    }
    fn registered_change(
        &self,
        root: &Path,
        file: &NativePath,
        staged: bool,
    ) -> Result<ChangedFile> {
        relative(file)?;
        let changes = self.working_changes(root)?;
        if changes.files.iter().any(|f| {
            f.index_status == "U"
                || f.worktree_status == "U"
                || matches!(
                    (f.index_status.as_str(), f.worktree_status.as_str()),
                    ("A", "A") | ("D", "D")
                )
        }) {
            return Err(AppError::new(
                "conflicts",
                "Resolve the existing index conflicts in Git before writing here",
            ));
        }
        let entry = changes
            .files
            .into_iter()
            .find(|f| {
                f.path.encoding == file.encoding
                    && f.path.hex == file.hex
                    && if staged { f.staged } else { f.unstaged }
            })
            .ok_or_else(|| {
                AppError::new(
                    "stale_preview",
                    "The selected file no longer has that change. Refresh and review it again.",
                )
            })?;
        if let Some(reason) = &entry.blocked_reason {
            return Err(AppError::new("unsupported_write", reason));
        }
        Ok(entry)
    }
    pub fn head_index_snapshot(&self, path: &Path) -> Result<String> {
        Ok(self.write_state(path)?.1)
    }
    /// Read-only display consistency check, including during pending Git operations.
    /// Unlike head_index_snapshot, this does not validate or authorize a write.
    pub fn index_display_fingerprint(&self, path: &Path) -> Result<String> {
        Ok(digest(&index_bytes(&self.discover(path)?.git_dir)?))
    }
    pub fn acquire_index_guard(&self, path: &Path) -> Result<OperationLock> {
        OperationLock::acquire(self.discover(path)?.git_dir.join("index.lock"))
    }
    pub fn acquire_operation(&self, path: &Path) -> Result<OperationLock> {
        let d = self.discover(path)?;
        OperationLock::acquire(d.common_dir.join("gitnova-operation.lock"))
    }
}
impl WorkingTreeReader for GitCli {
    fn working_changes(&self, root: &Path) -> Result<WorkingChanges> {
        let (status, raw) = self.status_observation(root, 4 * 1024 * 1024)?;
        if status.bare {
            return Ok(WorkingChanges {
                files: vec![],
                status,
                token: "bare".into(),
            });
        }
        let d = self.discover(root)?;
        // Incremental hashing avoids duplicating the entire index alongside the
        // status buffer merely to build a display fingerprint.
        let mut fingerprint = Sha256::new();
        fingerprint.update(&raw);
        fingerprint.update(index_bytes(&d.git_dir)?);
        fingerprint.update(status.head.as_deref().unwrap_or("").as_bytes());
        let mut files = vec![];
        for record in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            if record.len() < 4 {
                return Err(AppError::new(
                    "git_parse",
                    "Invalid working-tree status record",
                ));
            }
            if files.len() >= 2000 {
                return Err(AppError::new("safety_budget","More than 2,000 changed paths. Narrow the working tree before using this initial changes viewer."));
            }
            let path = path_from_bytes(record[3..].to_vec())?;
            let native = NativePath::new(&path);
            relative(&native)?;
            let x = record[0] as char;
            let y = record[1] as char;
            let mut blocked_reason =
                if x == 'U' || y == 'U' || (x == 'A' && y == 'A') || (x == 'D' && y == 'D') {
                    Some("Conflicted files require conflict-resolution support".into())
                } else {
                    None
                };
            if let Ok(meta) = fs::symlink_metadata(root.join(&path)) {
                fingerprint.update(meta.len().to_le_bytes());
                fingerprint.update(format!("{:?}", meta.modified()).as_bytes());
                if meta.is_dir() {
                    blocked_reason=Some("Submodule and nested-repository pointer writes need a separate reviewed workflow".into());
                }
            }
            files.push(ChangedFile {
                path: native,
                index_status: x.to_string(),
                worktree_status: y.to_string(),
                staged: x != ' ' && x != '?',
                unstaged: y != ' ',
                blocked_reason,
            });
        }
        if !files.is_empty() {
            let paths: Vec<PathBuf> = files
                .iter()
                .map(|f| f.path.to_path())
                .collect::<Result<_>>()?;
            let mut args = vec![
                OsStr::new("ls-files"),
                OsStr::new("--stage"),
                OsStr::new("-z"),
                OsStr::new("--"),
            ];
            args.extend(paths.iter().map(|p| p.as_os_str()));
            let index = self.read(root, &args, 4 * 1024 * 1024)?.bytes;
            let mut gitlinks: std::collections::HashSet<PathBuf> = index
                .split(|b| *b == 0)
                .filter(|e| e.starts_with(b"160000 "))
                .filter_map(|entry| {
                    entry
                        .iter()
                        .position(|b| *b == b'\t')
                        .map(|tab| path_from_bytes(entry[tab + 1..].to_vec()))
                })
                .collect::<Result<_>>()?;
            if status.head.is_some() {
                let mut tree_args = vec![
                    OsStr::new("ls-tree"),
                    OsStr::new("-r"),
                    OsStr::new("-z"),
                    OsStr::new("HEAD"),
                    OsStr::new("--"),
                ];
                tree_args.extend(paths.iter().map(|p| p.as_os_str()));
                let tree = self.read(root, &tree_args, 4 * 1024 * 1024)?.bytes;
                for record in tree
                    .split(|b| *b == 0)
                    .filter(|e| e.starts_with(b"160000 "))
                {
                    if let Some(tab) = record.iter().position(|b| *b == b'\t') {
                        gitlinks.insert(path_from_bytes(record[tab + 1..].to_vec())?);
                    }
                }
            }
            for file in &mut files {
                if gitlinks.contains(&file.path.to_path()?) {
                    file.blocked_reason =
                        Some("Submodule pointer writes need a separate reviewed workflow".into());
                }
            }
        }
        Ok(WorkingChanges {
            files,
            status,
            token: format!("{:x}", fingerprint.finalize()),
        })
    }
    fn file_preview(&self, root: &Path, file: &NativePath, staged: bool) -> Result<FilePreview> {
        let path = relative(file)?;
        let changes = self.working_changes(root)?;
        let entry = changes
            .files
            .iter()
            .find(|f| f.path.hex == file.hex && f.path.encoding == file.encoding)
            .ok_or_else(|| AppError::new("stale_preview", "File no longer has changes"))?;
        let pointer = entry.blocked_reason.as_deref()
            == Some("Submodule pointer writes need a separate reviewed workflow");
        let before = if pointer {
            changes.token.clone()
        } else {
            self.preview_token(root, file, staged)?
        };
        let mut args: Vec<&OsStr> = vec![
            OsStr::new("diff"),
            OsStr::new("--no-ext-diff"),
            OsStr::new("--no-textconv"),
            OsStr::new("--no-renames"),
            OsStr::new("--no-color"),
            OsStr::new("--submodule=short"),
            OsStr::new("--src-prefix=a/"),
            OsStr::new("--dst-prefix=b/"),
        ];
        if staged {
            args.push(OsStr::new("--cached"));
        }
        args.extend([OsStr::new("--"), path.as_os_str()]);
        let (diff, truncated) = if entry.index_status == "?" && !staged {
            let absolute = root.join(&path);
            let bytes = if fs::symlink_metadata(&absolute)?.file_type().is_symlink() {
                NativePath::new(&fs::read_link(&absolute)?)
                    .display
                    .into_bytes()
            } else {
                read_bounded(&absolute, FILE_LIMIT)?
            };
            let old_name = quoted_diff_path(&path, b"a/")?;
            let new_name = quoted_diff_path(&path, b"b/")?;
            if bytes.contains(&0) {
                (format!("diff --git {old_name} {new_name}\nBinary file. Text preview is unavailable.\n"), false)
            } else {
                let truncated = bytes.len() > PATCH_LIMIT;
                let content = String::from_utf8_lossy(&bytes[..bytes.len().min(PATCH_LIMIT)]);
                (
                    format!(
                        "diff --git {old_name} {new_name}\n--- /dev/null\n+++ {new_name}\n@@ -0,0 +1,{} @@\n{}",
                        content.lines().count(),
                        content
                            .lines()
                            .map(|line| format!("+{line}\n"))
                            .collect::<String>()
                    ),
                    truncated,
                )
            }
        } else {
            let out = self.execute(root, &args, PATCH_LIMIT, true)?;
            (String::from_utf8_lossy(&out.bytes).into(), out.truncated)
        };
        if (if pointer {
            self.working_changes(root)?.token
        } else {
            self.preview_token(root, file, staged)?
        }) != before
        {
            return Err(AppError::new(
                "stale_preview",
                "File or index changed while reading. Review it again.",
            ));
        }
        Ok(FilePreview {
            path: file.clone(),
            staged,
            token: before,
            diff,
            truncated,
        })
    }
    fn commit_preview(&self, root: &Path) -> Result<CommitPreview> {
        let (_, before) = self.commit_state(root)?;
        let changes = self.working_changes(root)?;
        self.validate_registration_commit(root, &changes.files)?;
        if changes.files.iter().any(|f| {
            f.staged
                && f.blocked_reason.is_some()
                && !(matches!(f.index_status.as_str(), "A" | "M" | "D")
                    && f.blocked_reason.as_deref()
                        == Some("Submodule pointer writes need a separate reviewed workflow"))
        }) {
            return Err(AppError::new(
                "unsupported_write",
                "A staged conflict or submodule pointer needs a separate reviewed workflow",
            ));
        }
        let files: Vec<_> = changes.files.into_iter().filter(|f| f.staged).collect();
        if files.is_empty() {
            return Err(AppError::new(
                "nothing_staged",
                "Stage at least one file before reviewing a commit",
            ));
        }
        let out = self.execute(
            root,
            &[
                OsStr::new("diff"),
                OsStr::new("--cached"),
                OsStr::new("--no-ext-diff"),
                OsStr::new("--no-textconv"),
                OsStr::new("--no-renames"),
                OsStr::new("--no-color"),
                OsStr::new("--submodule=short"),
                OsStr::new("--src-prefix=a/"),
                OsStr::new("--dst-prefix=b/"),
                OsStr::new("--"),
            ],
            PATCH_LIMIT,
            true,
        )?;
        if self.commit_state(root)?.1 != before {
            return Err(AppError::new(
                "stale_preview",
                "Staged changes changed while reading. Review again.",
            ));
        }
        Ok(CommitPreview {
            complete_manifest: true,
            token: before,
            files,
            status: changes.status,
            diff: String::from_utf8_lossy(&out.bytes).into(),
            truncated: out.truncated,
        })
    }
}
/// Creates only its own lock; an existing lock is never deleted or adopted.
pub struct OperationLock {
    path: PathBuf,
    file: Option<File>,
}
impl OperationLock {
    pub(crate) fn acquire(path: PathBuf) -> Result<Self> {
        let file=OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e|if e.kind()==std::io::ErrorKind::AlreadyExists {AppError::new("locked",format!("{} already exists. Verify the owning Git operation before attempting recovery; GitNova will not remove it.",path.display()))}else{e.into()})?;
        let mut guard = Self {
            path,
            file: Some(file),
        };
        writeln!(
            guard
                .file
                .as_mut()
                .ok_or_else(|| AppError::new("lock", "Lock file unavailable"))?,
            "GitNova pid={}",
            std::process::id()
        )?;
        Ok(guard)
    }
}
impl OperationLock {
    pub(crate) fn publish(
        &mut self,
        destination: &Path,
        bytes: &[u8],
        modified: Option<std::time::SystemTime>,
    ) -> Result<()> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| AppError::new("lock", "Lock handle unavailable"))?;
        if let Ok(metadata) = fs::metadata(destination) {
            file.set_permissions(metadata.permissions())?;
        }
        file.set_len(0)?;
        file.rewind()?;
        file.write_all(bytes)?;
        if let Some(time) = modified {
            file.set_times(fs::FileTimes::new().set_modified(time))?;
        }
        file.sync_all()?;
        self.file.take();
        fs::rename(&self.path, destination)?;
        self.path = PathBuf::new();
        Ok(())
    }
}
impl Drop for OperationLock {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}
impl IndexWriter for GitCli {
    fn change_index(&self, root: &Path, file: &NativePath, stage: bool, token: &str) -> Result<()> {
        self.registered_change(root, file, !stage)?;
        if self.preview_token(root, file, !stage)? != token {
            return Err(AppError::new(
                "stale_preview",
                "File, HEAD or index changed. Review the file again before staging or unstaging.",
            ));
        }
        let (d, state) = self.write_state(root)?;
        let mut lock = OperationLock::acquire(d.git_dir.join("index.lock"))?;
        if let Ok(metadata) = fs::metadata(d.git_dir.join("index")) {
            lock.file
                .as_ref()
                .ok_or_else(|| AppError::new("lock", "Index lock unavailable"))?
                .set_permissions(metadata.permissions())?;
        }
        let original = index_bytes(&d.git_dir)?;
        let temporary = tempfile::NamedTempFile::new_in(&d.git_dir)?.into_temp_path();
        if original.is_empty() {
            fs::remove_file(&temporary)?;
        } else {
            fs::write(&temporary, &original)?;
            preserve_index_timestamp(&d.git_dir.join("index"), &temporary)?;
        }
        if self.preview_token(root, file, !stage)? != token {
            return Err(AppError::new(
                "stale_preview",
                "Repository changed while acquiring the index lock",
            ));
        }
        let path = relative(file)?;
        let mut args: Vec<OsString> = if stage {
            vec!["add".into()]
        } else if self.status(root)?.head.is_some() {
            vec!["reset".into(), "--quiet".into(), "HEAD".into()]
        } else {
            vec![
                "rm".into(),
                "--cached".into(),
                "--force".into(),
                "--quiet".into(),
            ]
        };
        args.push("--".into());
        args.push(path.into_os_string());
        run_write(root, &args, Some(temporary.as_ref()))?;
        if self.write_state(root)?.1 != state || self.preview_token(root, file, !stage)? != token {
            return Err(AppError::new("stale_preview","File or HEAD changed during preparation. Original index was preserved; review again."));
        }
        let new_index = read_bounded(&temporary, INDEX_LIMIT)?;
        let handle = lock
            .file
            .as_mut()
            .ok_or_else(|| AppError::new("lock", "Index lock handle unavailable"))?;
        handle.set_len(0)?;
        handle.rewind()?;
        handle.write_all(&new_index)?;
        // A later copy timestamp can turn a racy stat entry into a falsely clean one.
        handle
            .set_times(fs::FileTimes::new().set_modified(fs::metadata(&temporary)?.modified()?))?;
        handle.sync_all()?;
        lock.file.take();
        fs::rename(&lock.path, d.git_dir.join("index"))?;
        // The path was moved; Drop's remove of the former lock is harmless while the
        // common-directory operation guard still excludes other GitNova writers.
        lock.path = PathBuf::new();
        Ok(())
    }
}
impl CommitWriter for GitCli {
    fn create_commit(&self, root: &Path, message: &str, token: &str) -> Result<()> {
        self.commit_with_index(root, message, token, false)
    }
}
impl GitCli {
    pub(crate) fn commit_state(&self, root: &Path) -> Result<(crate::Discovery, String)> {
        let (discovery, state) = self.write_state(root)?;
        let mut bytes = state.into_bytes();
        bytes.extend(self.config_snapshot(root)?);
        Ok((discovery, digest(&bytes)))
    }
    pub(crate) fn commit_with_index(
        &self,
        root: &Path,
        message: &str,
        token: &str,
        amend: bool,
    ) -> Result<()> {
        if message.trim().is_empty() || message.len() > 65536 || message.contains('\0') {
            return Err(AppError::new(
                "message",
                "Commit message must be nonempty, contain no NUL and be at most 64 KiB",
            ));
        }
        if !amend {
            let preview = self.commit_preview(root)?;
            if !preview.complete_manifest {
                return Err(AppError::new(
                    "safety_budget",
                    "The staged file manifest is incomplete. Refresh and review before committing.",
                ));
            }
            if preview.token != token {
                return Err(AppError::new(
                    "stale_preview",
                    "HEAD or staged content changed. Review the commit again.",
                ));
            }
        } else if self.commit_state(root)?.1 != token {
            return Err(AppError::new(
                "stale_preview",
                "HEAD or index changed. Review the amend again.",
            ));
        }
        let d = self.discover(root)?;
        if d.git_dir.join("index.lock").exists() {
            return Err(AppError::new(
                "locked",
                "Git index.lock exists. Another Git operation may be active; it was not removed.",
            ));
        }
        let mut lock = OperationLock::acquire(d.git_dir.join("index.lock"))?;
        if self.commit_state(root)?.1 != token {
            return Err(AppError::new(
                "stale_preview",
                "Index or HEAD changed while acquiring the commit lock. Review again.",
            ));
        }
        let temporary = tempfile::NamedTempFile::new_in(&d.git_dir)?.into_temp_path();
        if let Ok(metadata) = fs::metadata(d.git_dir.join("index")) {
            lock.file
                .as_ref()
                .ok_or_else(|| AppError::new("lock", "Index lock unavailable"))?
                .set_permissions(metadata.permissions())?;
        }
        fs::write(&temporary, index_bytes(&d.git_dir)?)?;
        preserve_index_timestamp(&d.git_dir.join("index"), &temporary)?;
        let mut body = tempfile::NamedTempFile::new_in(&d.git_dir)?;
        body.write_all(message.as_bytes())?;
        body.flush()?;
        let mut args = vec![
            "commit".into(),
            "--cleanup=verbatim".into(),
            "--file".into(),
            body.path().as_os_str().to_owned(),
        ];
        if amend {
            args.extend(["--amend".into(), "--allow-empty".into()]);
        }
        run_write(root, &args, Some(temporary.as_ref()))?;
        let new_index = read_bounded(&temporary, INDEX_LIMIT)?;
        let handle = lock
            .file
            .as_mut()
            .ok_or_else(|| AppError::new("lock", "Index lock unavailable"))?;
        handle.set_len(0)?;
        handle.rewind()?;
        handle.write_all(&new_index)?;
        // A later copy timestamp can turn a racy stat entry into a falsely clean one.
        handle
            .set_times(fs::FileTimes::new().set_modified(fs::metadata(&temporary)?.modified()?))?;
        handle.sync_all()?;
        lock.file.take();
        fs::rename(&lock.path, d.git_dir.join("index"))?;
        lock.path = PathBuf::new();
        Ok(())
    }
}
pub(crate) fn run_write(
    root: &Path,
    args: &[OsString],
    alternate_index: Option<&Path>,
) -> Result<()> {
    run_git_write(root, args, alternate_index, None, None)
}
pub(crate) fn run_offline_write(root: &Path, args: &[OsString]) -> Result<()> {
    run_git_write(root, args, None, Some(""), None)
}
pub(crate) fn run_fetch(root: &Path, args: &[OsString]) -> Result<()> {
    // GIT_ALLOW_PROTOCOL overrides per-protocol config too, including user-enabled helpers.
    run_git_write(root, args, None, Some("file:http:https:ssh:git"), None)
}
pub(crate) fn run_ref_transaction(root: &Path, transaction: &str) -> Result<()> {
    run_git_write(
        root,
        &["update-ref".into(), "--no-deref".into(), "--stdin".into()],
        None,
        Some(""),
        Some(transaction.as_bytes()),
    )
}
pub(crate) fn run_stash_write(root: &Path, args: &[OsString]) -> Result<()> {
    // Stash consumes no UI pathspecs. Older Git's internal clean uses :/ and breaks under literal-pathspecs.
    run_git_write_with_paths(
        root,
        args,
        None,
        Some(""),
        None,
        false,
        Duration::from_secs(60),
    )
}
fn run_git_write(
    root: &Path,
    args: &[OsString],
    alternate_index: Option<&Path>,
    protocols: Option<&str>,
    input: Option<&[u8]>,
) -> Result<()> {
    run_git_write_with_paths(
        root,
        args,
        alternate_index,
        protocols,
        input,
        true,
        Duration::from_secs(60),
    )
}
pub(crate) fn run_clone(root: &Path, args: &[OsString]) -> Result<()> {
    run_git_write_with_paths(
        root,
        args,
        None,
        Some("file:http:https:ssh:git"),
        None,
        true,
        Duration::from_secs(600),
    )
}
fn run_git_write_with_paths(
    root: &Path,
    args: &[OsString],
    alternate_index: Option<&Path>,
    protocols: Option<&str>,
    input: Option<&[u8]>,
    literal_paths: bool,
    budget: Duration,
) -> Result<()> {
    let mut command = Command::new(crate::executable::git_executable());
    crate::process::configure(&mut command);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    crate::commit_identity::configure(&mut command, root);
    command.env_remove("GITNOVA_CREDENTIAL_TARGETS");
    command.env_remove("GITNOVA_BACKGROUND_FETCH");
    let background = args
        .windows(2)
        .any(|pair| pair[0] == "-c" && pair[1] == "gitnova.backgroundFetch=true");
    let budget = if background {
        Duration::from_secs(20)
    } else {
        budget
    };
    if background {
        command
            .args([
                "-c",
                "credential.helper=",
                "-c",
                "credential.interactive=false",
                "-c",
                "core.askPass=",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .env("GITNOVA_BACKGROUND_FETCH", "1")
            .env("GCM_INTERACTIVE", "Never")
            .env(
                "GIT_SSH_COMMAND",
                "ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=10",
            )
            .env("SSH_ASKPASS_REQUIRE", "never")
            .env_remove("SSH_ASKPASS");
    }
    if protocols.is_some_and(|value| !value.is_empty()) {
        crate::credentials::configure(&mut command, root, args);
    }
    command.env_remove("GITNOVA_CREDENTIAL_FALLBACK");
    if literal_paths {
        command.arg("--literal-pathspecs");
    }
    command
        .args([
            "--no-pager",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "color.ui=false",
            "-C",
        ])
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(index) = alternate_index {
        command.env("GIT_INDEX_FILE", index);
    }
    if let Some(protocols) = protocols {
        command.env("GIT_ALLOW_PROTOCOL", protocols);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let _timer =
        crate::metrics::CommandTimer::new(args.first().and_then(|v| v.to_str()).unwrap_or("write"));
    let mut child = crate::process::spawn(&mut command)
        .map_err(|e| AppError::new("git_unavailable", e.to_string()))?;
    if let Some(bytes) = input {
        let result = child
            .stdin()
            .ok_or_else(|| AppError::new("git", "Missing Git input"))?
            .write_all(bytes);
        if let Err(error) = result {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
    }
    let stdout = child
        .stdout()
        .ok_or_else(|| AppError::new("git", "Missing stdout"))?;
    let stderr = child
        .stderr()
        .ok_or_else(|| AppError::new("git", "Missing stderr"))?;
    let (wake_tx, wake_rx) = mpsc::channel();
    let drain = |mut source: Box<dyn Read + Send>| {
        let wake = wake_tx.clone();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut captured = Vec::new();
            let mut buffer = [0; 8192];
            loop {
                match source.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let remaining = 65536_usize.saturating_sub(captured.len());
                        captured.extend_from_slice(&buffer[..n.min(remaining)]);
                    }
                    Err(_) => break,
                }
            }
            let _ = tx.send(captured);
            let _ = wake.send(());
        });
        rx
    };
    let out = drain(Box::new(stdout));
    let err = drain(Box::new(stderr));
    drop(wake_tx);
    let status = match crate::wait::wait_for_exit(
        &mut child,
        budget,
        &wake_rx,
        Duration::from_millis(15),
    )? {
        Some(status) => status,
        None => {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::new("operation_unknown","Git exceeded its execution budget. Inspect repository state and hooks before retrying; no rollback was attempted."));
        }
    };
    let stdout = out.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    let stderr = err.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    if !status.success() {
        let message = format!(
            "{}{}",
            String::from_utf8_lossy(&stderr),
            String::from_utf8_lossy(&stdout)
        )
        .trim()
        .to_owned();
        return Err(if protocols.is_some_and(|value| !value.is_empty()) {
            crate::credentials::transport_error("git_write_failed", message)
        } else {
            AppError::new("git_write_failed", message)
        });
    }
    Ok(())
}

// Quote raw path bytes so filenames cannot inject patch records or lose their identity.
pub(crate) fn quoted_diff_path(path: &Path, prefix: &[u8]) -> Result<String> {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    };
    #[cfg(windows)]
    let bytes = path
        .to_str()
        .ok_or_else(|| AppError::new("encoding", "Git diff path is not UTF-8"))?
        .replace('\\', "/")
        .into_bytes();
    let mut result = String::from("\"");
    for byte in prefix.iter().chain(&bytes) {
        result.push_str(&format!("\\{byte:03o}"));
    }
    result.push('"');
    Ok(result)
}

impl GitCli {
    pub(crate) fn commit_owned_merge(
        &self,
        root: &Path,
        message: &str,
        expected_state: &str,
    ) -> Result<()> {
        let d = self.discover(root)?;
        let original = index_bytes(&d.git_dir)?;
        let mut lock = OperationLock::acquire(d.git_dir.join("index.lock"))?;
        let state = self.merge_state(root)?;
        if state.phase != "pending" || state.token != expected_state {
            return Err(AppError::new(
                "stale_preview",
                "Merge state changed while locking its index. Review again.",
            ));
        }
        let temporary = tempfile::NamedTempFile::new_in(&d.git_dir)?.into_temp_path();
        fs::write(&temporary, &original)?;
        preserve_index_timestamp(&d.git_dir.join("index"), &temporary)?;
        if let Ok(meta) = fs::metadata(d.git_dir.join("index")) {
            lock.file
                .as_ref()
                .ok_or_else(|| AppError::new("lock", "Index lock unavailable"))?
                .set_permissions(meta.permissions())?;
        }
        let mut body = tempfile::NamedTempFile::new_in(&d.git_dir)?;
        body.write_all(message.as_bytes())?;
        body.flush()?;
        let mut args = vec![
            "commit".into(),
            "--cleanup=verbatim".into(),
            "--file".into(),
            body.path().as_os_str().to_owned(),
        ];
        if state.operation == "cherryPick" {
            let source = state
                .source_oid
                .as_deref()
                .ok_or_else(|| AppError::new("merge_metadata", "Missing cherry-pick source"))?;
            let author = self.text(
                root,
                &[
                    "show",
                    "-s",
                    "--no-show-signature",
                    "--format=%an%x00%ae%x00%aI",
                    source,
                ],
            )?;
            let parts: Vec<_> = author.trim_end_matches('\n').split('\0').collect();
            if parts.len() != 3 || parts.iter().any(|v| v.contains(['\n', '\r'])) {
                return Err(AppError::new(
                    "author",
                    "Source author needs manual inspection.",
                ));
            }
            args.push(format!("--author={} <{}>", parts[0], parts[1]).into());
            args.push(format!("--date={}", parts[2]).into());
        }
        run_write(root, &args, Some(temporary.as_ref()))?;
        if index_bytes(&d.git_dir)? != original {
            return Err(AppError::new("operation_unknown","Merge commit finished, but the protected index changed externally. Inspect HEAD/index; no index was overwritten."));
        }
        let bytes = read_bounded(&temporary, INDEX_LIMIT)?;
        let handle = lock
            .file
            .as_mut()
            .ok_or_else(|| AppError::new("lock", "Index lock unavailable"))?;
        handle.set_len(0)?;
        handle.rewind()?;
        handle.write_all(&bytes)?;
        handle
            .set_times(fs::FileTimes::new().set_modified(fs::metadata(&temporary)?.modified()?))?;
        handle.sync_all()?;
        lock.file.take();
        fs::rename(&lock.path, d.git_dir.join("index"))?;
        lock.path = PathBuf::new();
        Ok(())
    }
}

// Publish a prepared operation index while excluding native index writers.
pub(crate) fn publish_reviewed_index(
    git: &GitCli,
    root: &Path,
    original: &[u8],
    prepared: &Path,
    validate: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let d = git.discover(root)?;
    let mut lock = OperationLock::acquire(d.git_dir.join("index.lock"))?;
    validate()?;
    if index_bytes(&d.git_dir)? != original {
        return Err(AppError::new(
            "stale_preview",
            "Index changed during preparation. Review again.",
        ));
    }
    let bytes = read_bounded(prepared, INDEX_LIMIT)?;
    let handle = lock
        .file
        .as_mut()
        .ok_or_else(|| AppError::new("lock", "Index lock unavailable"))?;
    handle.set_permissions(fs::metadata(d.git_dir.join("index"))?.permissions())?;
    handle.set_len(0)?;
    handle.rewind()?;
    handle.write_all(&bytes)?;
    handle.set_times(fs::FileTimes::new().set_modified(fs::metadata(prepared)?.modified()?))?;
    handle.sync_all()?;
    lock.file.take();
    fs::rename(&lock.path, d.git_dir.join("index"))?;
    lock.path = PathBuf::new();
    Ok(())
}
