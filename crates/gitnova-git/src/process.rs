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
pub(crate) fn configure(command: &mut Command) {
    #[cfg(windows)]
    command.args(["-c", "core.longpaths=true"]);
    #[cfg(unix)]
    let _ = command;
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
