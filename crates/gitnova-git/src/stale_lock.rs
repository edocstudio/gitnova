//! Explicit archival of a proven-dead GitNova operation lock; never native Git locks.
use crate::{GitCli, RepositoryReader};
use gitnova_domain::*;
impl GitCli {
    pub fn preview_stale_lock(&self, root: &std::path::Path) -> Result<StaleLockPreview> {
        let d = self.discover(root)?;
        let path = d.common_dir.join("gitnova-operation.lock");
        let mut result = StaleLockPreview {
            path: NativePath::new(&path),
            owner_pid: None,
            exists: path.try_exists()?,
            token: String::new(),
            blocked_reason: None,
        };
        if !result.exists {
            return Ok(result);
        }
        match platform::inspect(self, root, &path) {
            Ok((pid, token)) => {
                result.owner_pid = Some(pid);
                result.token = token;
            }
            Err(e) => result.blocked_reason = Some(e.message),
        }
        Ok(result)
    }
    pub fn recover_stale_lock(&self, root: &std::path::Path, token: &str) -> Result<NativePath> {
        platform::recover(self, root, token)
    }
}
#[cfg(unix)]
mod platform {
    use super::*;
    use std::{
        fs::{self, File, OpenOptions},
        io::Read,
        os::{
            fd::AsRawFd,
            unix::fs::{MetadataExt, OpenOptionsExt},
        },
        path::Path,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    fn fail(s: &str) -> AppError {
        AppError::new("locked", s)
    }
    fn dead(pid: u32) -> Result<()> {
        // EPERM and PID reuse are live/unknown, never permission to recover.
        if unsafe { libc::kill(pid as i32, 0) } == -1
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        {
            Ok(())
        } else {
            Err(fail("The lock owner is still running or cannot be verified. Close or finish that operation first."))
        }
    }
    fn open(path: &Path) -> Result<File> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(fail(
                "The operation lock is not a regular file. Inspect it manually.",
            ));
        }
        Ok(file)
    }
    fn identity(path: &Path) -> Result<(u32, String)> {
        let mut file = open(path)?;
        let meta = file.metadata()?;
        let mut bytes = vec![];
        file.by_ref().take(129).read_to_end(&mut bytes)?;
        if bytes.len() > 128 {
            return Err(fail(
                "The operation lock has an unknown format. Inspect it manually.",
            ));
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| fail("The operation lock has an unknown format. Inspect it manually."))?;
        let pid = text
            .strip_prefix("GitNova pid=")
            .and_then(|s| s.strip_suffix('\n'))
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|p| *p > 0 && *p <= i32::MAX as u32)
            .ok_or_else(|| {
                fail("The operation lock has an unknown format. Inspect it manually.")
            })?;
        dead(pid)?;
        let stamp = format!(
            "{}:{}:{}:{}:{}:{text}",
            meta.dev(),
            meta.ino(),
            meta.mtime(),
            meta.mtime_nsec(),
            meta.len()
        );
        Ok((pid, stamp))
    }
    fn no_git_process() -> Result<()> {
        // No shell, credentials, arguments or process data crosses the IPC boundary.
        let output = tempfile::tempfile()?;
        let mut child = Command::new("/bin/ps")
            .args(["-A", "-o", "comm="])
            .stdin(Stdio::null())
            .stdout(output.try_clone()?)
            .stderr(Stdio::null())
            .spawn()?;
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    return Err(fail(
                        "Process ownership could not be verified. Try inspection again.",
                    ));
                }
                break;
            }
            if start.elapsed() > Duration::from_secs(3) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(fail(
                    "Process ownership could not be verified. Try inspection again.",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        use std::io::{Seek, SeekFrom};
        let mut output = output;
        output.seek(SeekFrom::Start(0))?;
        let mut bytes = vec![];
        output.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(fail(
                "Process ownership could not be verified. Try inspection again.",
            ));
        }
        if String::from_utf8_lossy(&bytes).lines().any(|line| {
            let name = Path::new(line.trim())
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            name == "git" || name.starts_with("git-")
        }) {
            return Err(fail(
                "A Git process is running. Wait for Git to finish, then inspect this lock again.",
            ));
        }
        Ok(())
    }
    pub(super) fn inspect(git: &GitCli, root: &Path, path: &Path) -> Result<(u32, String)> {
        super::inspect(git, root, path, identity, dead, no_git_process)
    }
    pub(super) fn recover(git: &GitCli, root: &Path, token: &str) -> Result<NativePath> {
        let d = git.discover(root)?;
        let path = d.common_dir.join("gitnova-operation.lock");
        let file = open(&path)?;
        // Serialize two recovery requests using the existing inode, without creating
        // another recovery lock that can itself become stale.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(fail(
                "Another recovery is inspecting this lock. Wait and inspect again.",
            ));
        }
        let meta = file.metadata()?;
        let current = fs::symlink_metadata(&path)?;
        if meta.dev() != current.dev() || meta.ino() != current.ino() {
            return Err(fail(
                "The lock changed. Inspect it again before recovering.",
            ));
        }
        let (pid, actual) = inspect(git, root, &path)?;
        if actual != token || token.len() != 64 {
            return Err(fail(
                "The lock changed. Inspect it again before recovering.",
            ));
        }
        let archive = d.common_dir.join(format!(
            "gitnova-operation.lock.stale-{pid}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::rename(&path, &archive)?;
        let moved = fs::symlink_metadata(&archive)?;
        if moved.dev() != meta.dev() || moved.ino() != meta.ino() {
            return Err(AppError::new(
                "operation_unknown",
                "Lock archival needs manual inspection. No Git operation was retried.",
            ));
        }
        Ok(NativePath::new(&archive))
    }
}

#[cfg(windows)]
#[path = "stale_lock_windows.rs"]
mod platform;
fn fail(s: &str) -> AppError {
    AppError::new("locked", s)
}
use std::{fs, path::Path};
fn native(dir: &Path) -> Result<()> {
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
        "BISECT_START",
        "index.lock",
        "HEAD.lock",
        "config.lock",
        "packed-refs.lock",
        "shallow.lock",
    ] {
        if match fs::symlink_metadata(dir.join(name)) {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.into()),
        } {
            return Err(fail("A native Git operation or lock remains. Inspect and finish it before recovering the GitNova lock."));
        }
    }
    Ok(())
}
fn ref_locks(dir: &Path, count: &mut usize) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for e in fs::read_dir(dir)? {
        *count += 1;
        if *count > 4096 {
            return Err(fail(
                "Lock inspection exceeded its budget. Inspect Git manually.",
            ));
        }
        let e = e?;
        let t = e.file_type()?;
        if is_link(&fs::symlink_metadata(e.path())?)
            || e.file_name().to_string_lossy().ends_with(".lock")
        {
            return Err(fail("A native Git operation or lock remains. Inspect and finish it before recovering the GitNova lock."));
        }
        if t.is_dir() {
            ref_locks(&e.path(), count)?;
        }
    }
    Ok(())
}

fn inspect(
    git: &GitCli,
    root: &Path,
    path: &Path,
    identity: fn(&Path) -> Result<(u32, String)>,
    dead: fn(u32) -> Result<()>,
    no_git_process: fn() -> Result<()>,
) -> Result<(u32, String)> {
    use sha2::{Digest, Sha256};
    let (pid, stamp) = identity(path)?;
    let d = git.discover(root)?;
    native(&d.common_dir)?;
    native(&d.git_dir)?;
    let admins = d.common_dir.join("worktrees");
    if admins.exists() {
        for (i, e) in fs::read_dir(admins)?.enumerate() {
            if i >= 128 {
                return Err(fail(
                    "Lock inspection exceeded its budget. Inspect Git manually.",
                ));
            }
            let e = e?;
            if !e.file_type()?.is_dir() || is_link(&fs::symlink_metadata(e.path())?) {
                return Err(fail(
                    "Worktree metadata needs manual inspection before lock recovery.",
                ));
            }
            native(&e.path())?;
            ref_locks(&e.path().join("refs"), &mut 0)?;
        }
    }
    ref_locks(&d.common_dir.join("refs"), &mut 0)?;
    let snapshot = git.head_index_snapshot(root)?;
    no_git_process()?;
    dead(pid)?;
    if identity(path)?.1 != stamp {
        return Err(fail(
            "The lock changed. Inspect it again before recovering.",
        ));
    }
    Ok((
        pid,
        format!(
            "{:x}",
            Sha256::digest(format!("{}:{stamp}:{snapshot}", d.common_dir.display()))
        ),
    ))
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(unix)]
    metadata.file_type().is_symlink()
}
