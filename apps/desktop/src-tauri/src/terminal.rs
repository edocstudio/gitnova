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
    child_exited: bool,
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
    _cli_directory: tempfile::TempDir,
    membership: String,
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
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
fn open(
    membership: String,
    root: &Path,
    cols: u16,
    rows: u16,
    identities: &Path,
) -> Result<Opened, AppError> {
    // A shell executable only; never interpolate a repository path into shell code.
    #[cfg(unix)]
    let shell = std::env::var_os("SHELL")
        .filter(|p| Path::new(p).is_absolute() && Path::new(p).is_file())
        .unwrap_or_else(|| "/bin/sh".into());
    #[cfg(windows)]
    let shell = std::ffi::OsString::from("powershell.exe");
    let mut command = CommandBuilder::new(&shell);
    command.env("GITNOVA_CLI_IDENTITIES_FILE", identities);
    #[cfg(unix)]
    let command = {
        let mut command = command;
        command.arg("-l");
        command
    };
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
    // Session-owned launchers avoid changing shell rc files or global commands.
    // The native executable path is data, never interpolated into shell code.
    let cli_directory = tempfile::Builder::new()
        .prefix("gitnova-terminal-bin-")
        .tempdir()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let launcher = cli_directory.path().join("nova");
        std::fs::write(
            &launcher,
            "#!/bin/sh\nexec \"$GITNOVA_CLI_EXECUTABLE\" --nova \"$@\"\n",
        )?;
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(windows)]
    std::fs::write(
        cli_directory.path().join("nova.cmd"),
        "@echo off\r\n\"%GITNOVA_CLI_EXECUTABLE%\" --nova %*\r\n",
    )?;
    let mut paths = vec![cli_directory.path().to_owned()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    command.env("PATH", std::env::join_paths(paths).map_err(err)?);
    command.env("GITNOVA_CLI_EXECUTABLE", std::env::current_exe()?);
    #[cfg(unix)]
    if Path::new(&shell)
        .file_name()
        .is_some_and(|name| name == "zsh")
    {
        // User startup files may replace PATH. Session-owned startup wrappers
        // restore the launcher after those files without editing user settings.
        let original = command
            .get_env("ZDOTDIR")
            .or_else(|| command.get_env("HOME"))
            .map(std::ffi::OsStr::to_owned)
            .ok_or_else(|| err("Shell home directory is unavailable."))?;
        command.env("GITNOVA_ZSH_STARTUP_DIRECTORY", original);
        command.env("GITNOVA_TERMINAL_BIN", cli_directory.path());
        command.env("GITNOVA_TERMINAL_ROOT", &root);
        for file in [".zshenv", ".zprofile", ".zshrc", ".zlogin"] {
            let mut startup = format!(
                "if [[ -r \"$GITNOVA_ZSH_STARTUP_DIRECTORY/{file}\" ]]; then source \"$GITNOVA_ZSH_STARTUP_DIRECTORY/{file}\"; fi\n"
            );
            startup.push_str("export ZDOTDIR=\"$GITNOVA_TERMINAL_BIN\"\n");
            if matches!(file, ".zshrc" | ".zlogin") {
                startup.push_str("export PATH=\"$GITNOVA_TERMINAL_BIN:$PATH\"\n");
                startup.push_str("builtin cd -- \"$GITNOVA_TERMINAL_ROOT\"\n");
            }
            if file == ".zlogin" {
                startup.push_str("export ZDOTDIR=\"$GITNOVA_ZSH_STARTUP_DIRECTORY\"\n");
            } else if file == ".zshrc" {
                startup.push_str("if [[ ! -o login ]]; then export ZDOTDIR=\"$GITNOVA_ZSH_STARTUP_DIRECTORY\"; fi\n");
            }
            std::fs::write(cli_directory.path().join(file), startup)?;
        }
        command.env("ZDOTDIR", cli_directory.path());
    }

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
    let master = Arc::new(Mutex::new(Some(pair.master)));
    let session = Arc::new(Session {
        _cli_directory: cli_directory,
        membership,
        master: master.clone(),
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
                if let Ok(mut out) = output.lock() {
                    out.child_exited = true;
                }
                // ConPTY keeps output alive until its pseudo-console closes.
                // Release it while the reader drains; reader EOF marks exit.
                let finished = master.lock().ok().and_then(|mut value| value.take());
                drop(finished);
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
            &app.cli_identity_settings_path(),
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
        exited: out.exited && out.child_exited && out.bytes.is_empty(),
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
        .as_ref()
        .ok_or_else(|| err("Terminal session is no longer available."))?
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
    fn output_eof_does_not_report_a_live_child_as_exited() {
        let _guard = TEST_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "exec </dev/null >/dev/null 2>&1; exec sleep 30"]);
        let opened = open_command("fixture".into(), temp.path(), 80, 24, command).unwrap();
        let handle = session("fixture", &opened.id).unwrap();
        // Inject the reader's EOF event independently of process exit. A real
        // controlling terminal can retain extra slave descriptors on Unix.
        handle.output.lock().unwrap().exited = true;
        assert!(
            !terminal_read("fixture".into(), opened.id.clone())
                .unwrap()
                .exited
        );
        terminal_close("fixture".into(), opened.id).unwrap();
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
            .write_all(b"printf '\\nPTY_OK:'; pwd; command -v nova; exit\r")
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
        assert!(
            output.contains("gitnova-terminal-bin-"),
            "nova launcher was not in PATH: {output}"
        );
        terminal_close("fixture".into(), opened.id.clone()).unwrap();
        assert!(session("fixture", &opened.id).is_err());
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn zsh_login_keeps_nova_after_startup_replaces_path() {
        let _guard = TEST_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let startup = temp.path().join("startup ' $ directory");
        std::fs::create_dir(&startup).unwrap();
        let rc = "export PATH=/usr/bin:/bin\nexport NOVA_FIXTURE_RC=loaded\nbuiltin cd /\n";
        std::fs::write(startup.join(".zshrc"), rc).unwrap();
        std::fs::write(startup.join(".zlogin"), "export PATH=/bin:/usr/bin\n").unwrap();
        let mut command = CommandBuilder::new("/bin/zsh");
        command.arg("-il");
        command.env("ZDOTDIR", &startup);
        let opened = open_command("fixture".into(), temp.path(), 100, 24, command).unwrap();
        let handle = session("fixture", &opened.id).unwrap();
        handle.writer.lock().unwrap().write_all(
            b"pwd; printf '\nSTARTUP:%s\n' \"$NOVA_FIXTURE_RC\"; command -v nova; printf 'RESTORED:%s\n' \"$ZDOTDIR\"; exit\r"
        ).unwrap();
        drop(handle);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut collected = Vec::new();
        loop {
            let batch = terminal_read("fixture".into(), opened.id.clone()).unwrap();
            collected.extend(batch.data);
            if batch.exited {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "zsh did not exit");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        terminal_close("fixture".into(), opened.id).unwrap();
        let output = String::from_utf8_lossy(&collected);
        assert!(output.contains("STARTUP:loaded"), "{output}");
        assert!(
            output.contains(
                &temp
                    .path()
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            ),
            "{output}"
        );
        assert!(output.contains("gitnova-terminal-bin-"), "{output}");
        assert!(
            output.contains(&format!("RESTORED:{}", startup.display())),
            "{output}"
        );
        assert_eq!(std::fs::read_to_string(startup.join(".zshrc")).unwrap(), rc);
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
        let cwd_report = temp.path().join("actual-cwd.txt");
        command.env("GITNOVA_TEST_CWD_OUTPUT", &cwd_report);
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
                b"[IO.File]::WriteAllText($env:GITNOVA_TEST_CWD_OUTPUT, (Get-Location).ProviderPath); [Console]::WriteLine('PTY_OK'); exit\r",
            )
            .unwrap();
        drop(handle);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut output = Vec::new();
        let mut cursor_reports = 0;
        loop {
            let batch = terminal_read("fixture".into(), opened.id.clone()).unwrap();
            output.extend(batch.data);
            // xterm answers this PowerShell startup query in the real UI.
            let requested = output.windows(4).filter(|v| *v == b"\x1b[6n").count();
            if requested > cursor_reports {
                let handle = session("fixture", &opened.id).unwrap();
                let mut writer = handle.writer.lock().unwrap();
                for _ in cursor_reports..requested {
                    writer.write_all(b"\x1b[1;1R").unwrap();
                }
                writer.flush().unwrap();
                cursor_reports = requested;
            }
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
        // ConPTY output includes cursor positioning/line wrapping, so it is not
        // a plain line protocol. Check the shell's actual cwd independently of
        // its terminal rendering; still require PTY output and process exit.
        assert!(
            !output.is_empty(),
            "PowerShell must produce terminal output"
        );
        let reported = std::fs::read_to_string(cwd_report)
            .expect("PowerShell must report the actual working directory");
        assert_eq!(
            Path::new(&reported).canonicalize().unwrap(),
            root.canonicalize().unwrap(),
            "{}",
            String::from_utf8_lossy(&output)
        );
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
