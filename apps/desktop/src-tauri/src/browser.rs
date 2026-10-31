//! Open only URLs validated by the caller; never invoke a command shell.
use gitnova_ipc::AppError;
#[cfg(not(target_os = "windows"))]
use std::process::{Command, Stdio};
#[cfg(not(target_os = "windows"))]
pub(crate) fn open(url: &str) -> Result<(), AppError> {
    let fail = || {
        AppError::new(
            "hosting",
            "Could not open the browser. Copy this build link.",
        )
    };
    #[cfg(target_os = "macos")]
    let mut command = Command::new("/usr/bin/open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(not(target_os = "windows"))]
    command.arg(url);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| fail())?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(fail())
                }
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(fail());
            }
            Ok(None) => {}
        }
        if start.elapsed() > std::time::Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(fail());
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn open(url: &str) -> Result<(), AppError> {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let url: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    // Validated HTTPS, stable NUL-terminated buffers, no command interpreter.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize > 32 {
        Ok(())
    } else {
        Err(AppError::new(
            "hosting",
            "Could not open the browser. Copy this build link.",
        ))
    }
}
