//! Desktop-owned PTYs. No shell input, credentials or transcripts are persisted.
use gitnova_ipc::AppError;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock, Mutex,
    },
};
const OUTPUT_LIMIT: usize = 256 * 1024;
const MAX_SESSIONS: usize = 4;
static INPUT_SLOTS: LazyLock<Arc<tokio::sync::Semaphore>> =
    LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(MAX_SESSIONS)));
static NEXT: AtomicU64 = AtomicU64::new(1);
static EPOCH: AtomicU64 = AtomicU64::new(1);
static SESSIONS: LazyLock<Mutex<HashMap<String, Arc<Session>>>> = LazyLock::new(Default::default);
#[derive(Default)]
struct Output {
    bytes: VecDeque<u8>,
    dropped: bool,
    exited: bool,
}
impl Output {
    fn append(&mut self, bytes: &[u8]) {
        let excess = (self.bytes.len() + bytes.len()).saturating_sub(OUTPUT_LIMIT);
        if excess > 0 {
            self.dropped = true;
            self.bytes.drain(..excess.min(self.bytes.len()));
        }
        self.bytes
            .extend(bytes.iter().rev().take(OUTPUT_LIMIT).rev().copied());
    }
}
struct Session {
    membership: String,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    output: Arc<Mutex<Output>>,
}
impl Session {
    fn stop(&self) {
        if let Ok(mut child) = self.child.lock() {
            if matches!(child.try_wait(), Ok(None)) {
                let _ = child.kill();
            }
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    id: String,
    path: String,
    shell: String,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    data: Vec<u8>,
    dropped: bool,
    exited: bool,
}
fn err(message: impl ToString) -> AppError {
    AppError::new("terminal", message.to_string())
}
fn size(cols: u16, rows: u16) -> Result<PtySize, AppError> {
    if !(2..=500).contains(&cols) || !(2..=200).contains(&rows) {
        return Err(err("Invalid terminal size."));
    }
    Ok(PtySize {
        cols,
        rows,
        pixel_width: 0,
        pixel_height: 0,
    })
}
fn session(membership: &str, id: &str) -> Result<Arc<Session>, AppError> {
    SESSIONS
        .lock()
        .map_err(err)?
        .get(id)
        .filter(|s| s.membership == membership)
        .cloned()
        .ok_or_else(|| err("Terminal session is no longer available."))
}
fn open(membership: String, root: &Path, cols: u16, rows: u16) -> Result<Opened, AppError> {
    // A shell executable only; never interpolate a repository path into shell code.
    #[cfg(unix)]
    let shell = std::env::var_os("SHELL")
        .filter(|p| Path::new(p).is_absolute() && Path::new(p).is_file())
        .unwrap_or_else(|| "/bin/sh".into());
    #[cfg(windows)]
    let shell = std::ffi::OsString::from("powershell.exe");
    let mut command = CommandBuilder::new(&shell);
    #[cfg(unix)]
    command.arg("-l");
    open_command(membership, root, cols, rows, command)
}
fn open_command(
    membership: String,
    root: &Path,
    cols: u16,
    rows: u16,
    mut command: CommandBuilder,
) -> Result<Opened, AppError> {
    let dimensions = size(cols, rows)?;
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(err("Repository folder is no longer available."));
    }
    let mut sessions = SESSIONS.lock().map_err(err)?;
    if sessions.len() >= MAX_SESSIONS {
        return Err(err(
            "End an existing terminal session before opening another (maximum 4).",
        ));
    }
    let shell = command
        .get_argv()
        .first()
        .cloned()
        .ok_or_else(|| err("Terminal shell is unavailable."))?;
    command.cwd(&root);
    command.env("TERM", "xterm-256color");
    // Do not leak app-internal Git routing or any one-shot authentication to a shell.
    let environment: Vec<_> = std::env::vars_os().map(|(key, _)| key).collect();
    for key in environment {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let pair = native_pty_system().openpty(dimensions).map_err(err)?;
    let reader = pair.master.try_clone_reader().map_err(err)?;
    let writer = pair.master.take_writer().map_err(err)?;
    let child = pair.slave.spawn_command(command).map_err(err)?;
    let child = Arc::new(Mutex::new(child));
    drop(pair.slave);
    let output = Arc::new(Mutex::new(Output::default()));
    let session = Arc::new(Session {
        membership,
        master: Mutex::new(pair.master),
        writer: Mutex::new(writer),
        child: child.clone(),
        output: output.clone(),
    });
    let read_output = output.clone();
    if let Err(e) = std::thread::Builder::new()
        .name("terminal-output".into())
        .spawn(move || {
            let mut reader = reader;
            let mut bytes = [0u8; 8192];
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        if let Ok(mut out) = read_output.lock() {
                            out.append(&bytes[..count]);
                        } else {
                            break;
                        }
                    }
                }
            }
            if let Ok(mut out) = read_output.lock() {
                out.exited = true;
            }
        })
    {
        if let Ok(mut child) = child.lock() {
            let _ = child.kill();
        }
        return Err(err(e));
    }
    if let Err(e) = std::thread::Builder::new()
        .name("terminal-wait".into())
        .spawn(move || loop {
            let done = match child.lock() {
                Ok(mut child) => !matches!(child.try_wait(), Ok(None)),
                Err(_) => true,
            };
            if done {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        })
    {
        return Err(err(e));
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed).to_string();
    sessions.insert(id.clone(), session);
    Ok(Opened {
        id,
        path: root.to_string_lossy().into_owned(),
        shell: shell.to_string_lossy().into_owned(),
    })
}
#[tauri::command]
pub async fn terminal_open(
    state: super::State<'_>,
    membership_id: String,
    cols: u16,
    rows: u16,
) -> Result<Opened, AppError> {
    let epoch = EPOCH.load(Ordering::SeqCst);
    super::run(state, move |app| {
        if epoch != EPOCH.load(Ordering::SeqCst) {
            return Err(err("Terminal context changed; open the terminal again."));
        }
        let opened = open(
            membership_id.clone(),
            &app.path(&membership_id)?,
            cols,
            rows,
        )?;
        if epoch != EPOCH.load(Ordering::SeqCst) {
            terminal_close(membership_id, opened.id)?;
            return Err(err("Terminal context changed; open the terminal again."));
        }
        Ok(opened)
    })
    .await
}
#[tauri::command]
pub fn terminal_read(membership_id: String, id: String) -> Result<Batch, AppError> {
    let session = session(&membership_id, &id)?;
    let mut out = session.output.lock().map_err(err)?;
    let count = out.bytes.len().min(65536);
    let data = out.bytes.drain(..count).collect();
    let dropped = std::mem::take(&mut out.dropped);
    Ok(Batch {
        data,
        dropped,
        exited: out.exited && out.bytes.is_empty(),
    })
}
#[tauri::command]
pub async fn terminal_write(
    membership_id: String,
    id: String,
    data: String,
) -> Result<(), AppError> {
    if data.len() > 16384 {
        return Err(err("Terminal input exceeds its limit."));
    }
    let session = session(&membership_id, &id)?;
    let slot = INPUT_SLOTS
        .clone()
        .try_acquire_owned()
        .map_err(|_| err("Terminal input is busy; wait for the current paste to finish."))?;
    tauri::async_runtime::spawn_blocking(move || {
        let _slot = slot;
        let mut writer = session.writer.lock().map_err(err)?;
        writer.write_all(data.as_bytes())?;
        writer.flush()?;
        Ok(())
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub fn terminal_resize(
    membership_id: String,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), AppError> {
    session(&membership_id, &id)?
        .master
        .lock()
        .map_err(err)?
        .resize(size(cols, rows)?)
        .map_err(err)
}
#[tauri::command]
pub fn terminal_close(membership_id: String, id: String) -> Result<(), AppError> {
    let mut sessions = SESSIONS.lock().map_err(err)?;
    if sessions
        .get(&id)
        .is_some_and(|s| s.membership != membership_id)
    {
        return Err(err("Terminal session is no longer available."));
    }
    if let Some(session) = sessions.remove(&id) {
        session.stop();
    }
    Ok(())
}
pub fn close_all() {
    EPOCH.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut sessions) = SESSIONS.lock() {
        for session in sessions.values() {
            session.stop();
        }
        sessions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static TEST_LOCK: Mutex<()> = Mutex::new(());
    #[test]
    fn output_and_dimensions_are_bounded() {
        let mut out = Output::default();
        out.append(&vec![b'x'; OUTPUT_LIMIT + 99]);
        assert_eq!(out.bytes.len(), OUTPUT_LIMIT);
        assert!(out.dropped);
        out.append(b"end");
        assert_eq!(out.bytes.len(), OUTPUT_LIMIT);
        assert_eq!(out.bytes.back(), Some(&b'd'));
        assert!(size(0, 24).is_err());
        assert!(size(80, 65535).is_err());
        assert!(size(80, 24).is_ok());
    }
    #[test]
    #[cfg(unix)]
    fn real_pty_uses_exact_folder_and_rejects_other_membership() {
        let _guard = TEST_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("folder ' ; $ test");
        std::fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        let mut command = CommandBuilder::new("/bin/sh");
        #[cfg(windows)]
        let mut command = CommandBuilder::new("powershell.exe");
        #[cfg(unix)]
        command.arg("-i");
        #[cfg(windows)]
        command.arg("-NoProfile");
        command.env("HOME", temp.path());
        let opened = open_command("fixture".into(), &root, 80, 24, command).unwrap();
        assert_eq!(Path::new(&opened.path), root.canonicalize().unwrap());
        assert!(terminal_read("other".into(), opened.id.clone()).is_err());
        assert!(terminal_close("other".into(), opened.id.clone()).is_err());
        let handle = session("fixture", &opened.id).unwrap();
        handle
            .writer
            .lock()
            .unwrap()
            .write_all(b"printf '\\nPTY_OK:'; pwd; exit\r")
            .unwrap();
        drop(handle);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut collected = Vec::new();
        loop {
            let batch = terminal_read("fixture".into(), opened.id.clone()).unwrap();
            collected.extend(batch.data);
            if batch.exited {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "shell did not exit");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let output = String::from_utf8_lossy(&collected);
        assert!(output.contains("PTY_OK:"));
        assert!(output.contains(root.to_str().unwrap()), "{output}");
        terminal_close("fixture".into(), opened.id.clone()).unwrap();
        assert!(session("fixture", &opened.id).is_err());
    }
    #[test]
    #[cfg(windows)]
    fn windows_pty_uses_current_repository_and_rejects_other_membership() {
        let _guard = TEST_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("folder ' ; test");
        std::fs::create_dir(&root).unwrap();
        let mut command = CommandBuilder::new("powershell.exe");
        command.args(["-NoLogo", "-NoProfile"]);
        let opened = open_command("fixture".into(), &root, 300, 24, command).unwrap();
        assert_eq!(Path::new(&opened.path), root.canonicalize().unwrap());
        assert!(terminal_read("other".into(), opened.id.clone()).is_err());
        assert!(terminal_close("other".into(), opened.id.clone()).is_err());
        let handle = session("fixture", &opened.id).unwrap();
        handle
            .writer
            .lock()
            .unwrap()
            .write_all(
                b"[Console]::WriteLine(('PTY_' + 'OK:') + (Get-Location).ProviderPath); exit\r",
            )
            .unwrap();
        drop(handle);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut output = Vec::new();
        loop {
            let batch = terminal_read("fixture".into(), opened.id.clone()).unwrap();
            output.extend(batch.data);
            if batch.exited {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "PowerShell did not exit"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        terminal_close("fixture".into(), opened.id.clone()).unwrap();
        let output = String::from_utf8_lossy(&output);
        let normalized = output.replace(r"\\?\", "");
        let expected = format!("PTY_OK:{}", root.to_str().unwrap());
        assert!(normalized.contains(&expected), "{output}");
        assert!(session("fixture", &opened.id).is_err());
    }
    #[test]
    fn session_limit_and_explicit_close_reap_shells() {
        let _guard = TEST_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let make_command = || {
            #[cfg(unix)]
            let mut cmd = CommandBuilder::new("/bin/sh");
            #[cfg(unix)]
            cmd.arg("-i");
            #[cfg(windows)]
            let mut cmd = CommandBuilder::new("powershell.exe");
            #[cfg(windows)]
            cmd.args(["-NoLogo", "-NoProfile"]);
            cmd.env("HOME", temp.path());
            cmd
        };
        let mut opened = Vec::new();
        for i in 0..MAX_SESSIONS {
            opened.push(
                open_command(format!("limit-{i}"), temp.path(), 80, 24, make_command()).unwrap(),
            );
        }
        assert!(open_command("extra".into(), temp.path(), 80, 24, make_command()).is_err());
        let children: Vec<_> = opened
            .iter()
            .enumerate()
            .map(|(i, o)| session(&format!("limit-{i}"), &o.id).unwrap().child.clone())
            .collect();
        for (i, o) in opened.iter().enumerate() {
            terminal_close(format!("limit-{i}"), o.id.clone()).unwrap();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        for child in children {
            loop {
                if child.lock().unwrap().try_wait().unwrap().is_some() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "shell still running after close"
                );
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        let next = open_command("reused-slot".into(), temp.path(), 80, 24, make_command()).unwrap();
        terminal_close("reused-slot".into(), next.id).unwrap();
    }
}
