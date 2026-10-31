//! Common subprocess lifetime for Git reads/writes. Windows owns a Job Object so
//! a timeout terminates helper/hook descendants as well as the Git process.
use std::{
    io,
    process::{ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus},
};
#[cfg(unix)]
type NativeChild = std::process::Child;
#[cfg(windows)]
type NativeChild = command_group::GroupChild;
pub(crate) struct Child(NativeChild);
/// Paths passed to Git's own parser must not use Windows verbatim prefixes.
/// Preserve arbitrary native bytes on Unix; reject unrepresentable Windows paths.
pub(crate) fn git_path(path: &std::path::Path) -> gitnova_domain::Result<std::ffi::OsString> {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        if path.to_str().is_none()
            || matches!(path.components().next(), Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Verbatim(_) | Prefix::DeviceNS(_)))
        {
            return Err(gitnova_domain::AppError::new(
                "encoding",
                "Cannot represent Git path.",
            ));
        }
        Ok(crate::credentials::helper_path(path).into())
    }
    #[cfg(not(windows))]
    Ok(path.as_os_str().to_owned())
}
pub(crate) fn configure(command: &mut Command) {
    #[cfg(windows)]
    command.args(["-c", "core.longpaths=true"]);
    #[cfg(unix)]
    let _ = command;
}
/// Pin an already-resolved push URL to a process-local remote with no fetch
/// refspecs. Git 2.56 otherwise matches anonymous URL pushes to configured
/// push URLs and can mutate a different server's fetch-tracking refs.
/// Credential selection still receives the original reviewed arguments.
/// The pre-push hook receives this temporary remote name and the exact URL.
pub(crate) fn pinned_push_args(args: &[std::ffi::OsString]) -> Vec<std::ffi::OsString> {
    let mut position = 0;
    while args.get(position).is_some_and(|v| v == "-c") {
        position += 2;
    }
    let mut result = args.to_vec();
    if args.get(position).is_none_or(|v| v != "push") {
        return result;
    }
    let Some(separator) = args
        .iter()
        .enumerate()
        .skip(position + 1)
        .find(|(_, v)| *v == "--")
        .map(|(i, _)| i)
    else {
        return result;
    };
    let Some(url) = args.get(separator + 1) else {
        return result;
    };
    let name = format!("gitnova-reviewed-{}", uuid::Uuid::new_v4().simple());
    let mut assignment = std::ffi::OsString::from(format!("remote.{name}.url="));
    assignment.push(url);
    result[separator + 1] = name.into();
    result.splice(0..0, [std::ffi::OsString::from("-c"), assignment]);
    result
}
pub(crate) fn spawn(command: &mut Command) -> io::Result<Child> {
    #[cfg(unix)]
    let process = command.spawn()?;
    #[cfg(windows)]
    let process = {
        use command_group::CommandGroup;
        command
            .group()
            .kill_on_drop(true)
            .creation_flags(0x08000000)
            .spawn()?
    };
    Ok(Child(process))
}
impl Child {
    fn inner(&mut self) -> &mut std::process::Child {
        #[cfg(unix)]
        {
            &mut self.0
        }
        #[cfg(windows)]
        {
            self.0.inner()
        }
    }
    pub(crate) fn stdin(&mut self) -> Option<ChildStdin> {
        self.inner().stdin.take()
    }
    pub(crate) fn stdout(&mut self) -> Option<ChildStdout> {
        self.inner().stdout.take()
    }
    pub(crate) fn stderr(&mut self) -> Option<ChildStderr> {
        self.inner().stderr.take()
    }
    #[cfg(unix)]
    pub(crate) fn id(&self) -> u32 {
        self.0.id()
    }
    pub(crate) fn kill(&mut self) -> io::Result<()> {
        self.0.kill()
    }
    pub(crate) fn wait(&mut self) -> io::Result<ExitStatus> {
        self.0.wait()
    }
}
impl crate::wait::Waitable for Child {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.0.try_wait()
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn windows_git_paths_preserve_scope_and_reject_device_namespaces() {
        use std::{os::windows::ffi::OsStringExt, path::Path};
        assert_eq!(
            super::git_path(Path::new(r"\\?\C:\space ' ไทย\message")).unwrap(),
            std::ffi::OsString::from("C:/space ' ไทย/message")
        );
        assert_eq!(
            super::git_path(Path::new(r"\\?\UNC\host\share\message")).unwrap(),
            std::ffi::OsString::from("//host/share/message")
        );
        assert!(super::git_path(Path::new(r"\\.\PIPE\message")).is_err());
        assert!(super::git_path(Path::new(r"\\?\Volume{fake}\message")).is_err());
        let invalid = std::ffi::OsString::from_wide(&[0xD800]);
        assert!(super::git_path(Path::new(&invalid)).is_err());
    }
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        process::Stdio,
        sync::mpsc,
        time::Duration,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        Storage::FileSystem::SYNCHRONIZE,
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    #[test]
    fn timeout_cleanup_terminates_descendants_and_reaps_the_group() {
        let mut command = Command::new("powershell.exe");
        command.args(["-NoLogo","-NoProfile","-NonInteractive","-Command", "$p=Start-Process cmd.exe -ArgumentList '/D /C ping -n 60 127.0.0.1 >nul' -NoNewWindow -PassThru; [Console]::WriteLine($p.Id); Start-Sleep 60"])
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = spawn(&mut command).unwrap();
        let out = child.stdout().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = BufReader::new(out).read_line(&mut text);
            let _ = tx.send(text);
        });
        let pid = rx
            .recv_timeout(Duration::from_secs(20))
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        let handle =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid) };
        assert!(!handle.is_null());
        child.kill().unwrap();
        child.wait().unwrap();
        let status = unsafe { WaitForSingleObject(handle, 3000) };
        unsafe {
            CloseHandle(handle);
        }
        assert_eq!(status, WAIT_OBJECT_0);
    }
}
