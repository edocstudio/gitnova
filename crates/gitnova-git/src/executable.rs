//! Respect PATH overrides. On macOS only, bypass Apple's /usr/bin/git dispatcher
//! using the active developer tool selected by xcrun. No version/toolchain changes.
use std::ffi::OsString;
#[cfg(any(target_os = "macos", test))]
use std::path::PathBuf;
pub(crate) fn git_executable() -> OsString {
    #[cfg(target_os = "macos")]
    {
        use std::sync::OnceLock;
        struct Resolved {
            path: Option<OsString>,
            developer: Option<OsString>,
            toolchains: Option<OsString>,
            binary: OsString,
        }
        static VALUE: OnceLock<Resolved> = OnceLock::new();
        let path = std::env::var_os("PATH");
        let developer = std::env::var_os("DEVELOPER_DIR");
        let toolchains = std::env::var_os("TOOLCHAINS");
        let value = VALUE.get_or_init(|| {
            let binary = if first_on_path(path.as_deref()).as_deref()
                == Some(std::path::Path::new("/usr/bin/git"))
            {
                developer_git().unwrap_or_else(|| "git".into())
            } else {
                "git".into()
            };
            Resolved {
                path: path.clone(),
                developer: developer.clone(),
                toolchains: toolchains.clone(),
                binary,
            }
        });
        if value.path == path && value.developer == developer && value.toolchains == toolchains {
            return value.binary.clone();
        }
    }
    "git".into()
}
#[cfg(any(target_os = "macos", test))]
fn first_on_path(path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let path = path?;
    std::env::split_paths(path)
        .map(|p| p.join("git"))
        .find(|p| {
            let Ok(meta) = std::fs::metadata(p) else {
                return false;
            };
            if !meta.is_file() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                meta.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                true
            }
        })
}
#[cfg(target_os = "macos")]
fn developer_git() -> Option<OsString> {
    use std::os::unix::{ffi::OsStringExt, process::CommandExt};
    use std::{
        io::Read,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let mut child = Command::new("/usr/bin/xcrun")
        .args(["--find", "git"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .ok()?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(v)) => break v,
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
        if start.elapsed() >= Duration::from_secs(2) {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    if !status.success() {
        return None;
    }
    let mut bytes = Vec::new();
    child
        .stdout
        .take()?
        .take(8193)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 8192 {
        return None;
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    let path = PathBuf::from(OsString::from_vec(bytes));
    if !path.is_absolute() || path == std::path::Path::new("/usr/bin/git") {
        return None;
    }
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(&path).ok()?;
    (meta.is_file() && meta.permissions().mode() & 0o111 != 0).then(|| path.into_os_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn respects_first_executable_in_path() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("custom");
        let second = dir.path().join("system");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        for p in [&first, &second] {
            let file = p.join("git");
            std::fs::write(&file, b"#!/bin/sh\nexit 0\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        let path = std::env::join_paths([first.clone(), second.clone()]).unwrap();
        assert_eq!(first_on_path(Some(&path)), Some(first.join("git")));
    }
}
