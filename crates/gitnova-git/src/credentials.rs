//! Process-local Git credential bridge. No secret is placed in argv, environment or config files.
use gitnova_domain::*;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

static HELPER: OnceLock<PathBuf> = OnceLock::new();

pub fn install_transport_credential_helper(executable: &Path) -> Result<()> {
    let executable = executable.canonicalize()?;
    if !executable.is_file() || executable.to_str().is_none() {
        return Err(AppError::new(
            "credential_helper",
            "Invalid native credential helper executable.",
        ));
    }
    HELPER.set(executable).map_err(|_| {
        AppError::new(
            "credential_helper",
            "Credential helper is already configured.",
        )
    })
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn transport_target(args: &[std::ffi::OsString]) -> Option<&std::ffi::OsStr> {
    let mut position = 0;
    while args.get(position).is_some_and(|v| v == "-c") {
        position += 2;
    }
    if !args
        .get(position)
        .and_then(|v| v.to_str())
        .is_some_and(|v| matches!(v, "ls-remote" | "fetch" | "push" | "clone"))
    {
        return None;
    }
    let separator = args
        .iter()
        .enumerate()
        .skip(position + 1)
        .find(|(_, v)| *v == "--")?
        .0;
    args.get(separator + 1).map(|v| v.as_os_str())
}

pub(crate) fn configure(command: &mut Command, root: &Path, args: &[std::ffi::OsString]) {
    let Some(executable) = HELPER.get() else {
        return;
    };
    let Some(target) = transport_target(args).and_then(|v| v.to_str()) else {
        return;
    };
    let url = if target.starts_with("https://") {
        target.to_owned()
    } else if !target.is_empty() && !target.starts_with('-') && !target.contains(['/', ':', '@']) {
        let Ok(url) = crate::GitCli.text(root, &["remote", "get-url", "--", target]) else {
            return;
        };
        url.trim_end_matches('\n').to_owned()
    } else {
        return;
    };
    if !url.starts_with("https://")
        || url.len() > 4096
        || url.chars().any(char::is_control)
        || url.contains(['?', '#'])
        || url
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or("")
            .contains('@')
    {
        return;
    }
    configure_helper(command, executable);
    // URL-scoped settings can override generic http.* keys. Pin this selected
    // URL too, so same-host redirects cannot reuse its token on another path.
    command.args(["-c", &format!("http.{url}.followRedirects=false")]);
    // Reset URL-specific helpers as well: otherwise a more-specific plaintext
    // helper could store the app's token when Git approves authentication.
    let scoped_helper = format!("credential.{url}.helper");
    command.args([
        "-c",
        &format!("{scoped_helper}="),
        "-c",
        &format!("{scoped_helper}={}", helper_value(executable)),
    ]);
    command.env(
        "GITNOVA_CREDENTIAL_TARGETS",
        serde_json::to_string(&[url]).unwrap(),
    );
}

fn configure_helper(command: &mut Command, executable: &Path) {
    let helper = format!("credential.helper={}", helper_value(executable));
    // The helper prefers the app account, then consults the existing Git helpers
    // in a clean child process. Never approve/store the app token in an unrelated
    // helper (which could be plaintext). SSH agents remain independent.
    command.args([
        "-c",
        "credential.helper=",
        "-c",
        &helper,
        "-c",
        "credential.useHttpPath=true",
        "-c",
        "http.followRedirects=false",
    ]);
}

fn helper_value(executable: &Path) -> String {
    format!(
        "!{} --gitnova-credential-helper",
        shell_quote(&helper_path(executable))
    )
}

pub(crate) fn transport_error(code: &str, message: String) -> AppError {
    if message.contains("terminal prompts disabled")
        || message.contains("Authentication failed")
        || message.contains("HTTP Basic: Access denied")
        || message.contains("could not read Username")
        || message.contains("could not read Password")
        || message.contains("returned error: 401")
        || message.contains("returned error: 403")
        || message.contains("Invalid username or token")
    {
        AppError::new("git_auth", "Git authentication failed. Reconnect the matching host account in Settings, or check Git credentials/SSH. The account needs repository access and permission for this operation.")
    } else {
        AppError::new(code, message)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs, io::Write, os::unix::fs::PermissionsExt, process::Stdio};

    #[test]
    fn only_the_selected_target_not_command_option_or_ref_names_is_authorized() {
        let args = [
            "-c",
            "protocol.allow=never",
            "fetch",
            "--no-tags",
            "--",
            "origin",
            "fetch",
        ]
        .map(std::ffi::OsString::from);
        assert_eq!(
            transport_target(&args),
            Some(std::ffi::OsStr::new("origin"))
        );
        let args = [
            "clone",
            "--origin",
            "another",
            "--",
            "https://one.example/repo.git",
            "/tmp/destination",
        ]
        .map(std::ffi::OsString::from);
        assert_eq!(
            transport_target(&args),
            Some(std::ffi::OsStr::new("https://one.example/repo.git"))
        );
        let args = ["submodule", "update", "--", "origin"].map(std::ffi::OsString::from);
        assert!(transport_target(&args).is_none());
    }

    #[test]
    fn real_git_uses_process_helper_with_quoted_path_and_does_not_store() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("native helper's path");
        fs::write(&executable, "#!/bin/sh\n[ \"$1\" = --gitnova-credential-helper ] || exit 1\n[ \"$2\" = get ] || exit 0\ncat >/dev/null\nprintf 'username=fixture\\npassword=fixture-test-only\\n\\n'\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let mut command = Command::new(crate::executable::git_executable());
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        configure_helper(&mut command, &executable);
        command
            .args(["credential", "fill"])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .current_dir(dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let args = format!("{command:?}");
        assert!(!args.contains("fixture-test-only"));
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"protocol=https\nhost=github.example\npath=team/repo.git\n\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let response = String::from_utf8(output.stdout).unwrap();
        assert!(response.contains("username=fixture\n"));
        assert!(response.contains("password=fixture-test-only\n"));
        assert!(!dir.path().join(".gitconfig").exists());
    }

    #[test]
    fn authentication_failure_is_distinct_from_unknown_remote_effects() {
        assert_eq!(
            transport_error(
                "git_exit_128",
                "fatal: could not read Username: terminal prompts disabled".into()
            )
            .code,
            "git_auth"
        );
        assert_eq!(
            transport_error("git_exit_1", "connection lost".into()).code,
            "git_exit_1"
        );
    }
}

pub(crate) fn helper_path(executable: &Path) -> String {
    let value = executable.to_str().unwrap();
    #[cfg(windows)]
    {
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            return format!("//{}", unc.replace('\\', "/"));
        }
        value
            .strip_prefix(r"\\?\")
            .unwrap_or(value)
            .replace('\\', "/")
    }
    #[cfg(not(windows))]
    value.to_owned()
}

#[cfg(all(test, windows))]
mod windows_path_tests {
    use super::*;
    #[test]
    fn git_shell_helper_keeps_spaces_and_normalizes_extended_paths() {
        assert_eq!(
            helper_path(Path::new(
                r"\\?\C:\Program Files\GitNova\gitnova-desktop.exe"
            )),
            "C:/Program Files/GitNova/gitnova-desktop.exe"
        );
        assert_eq!(
            helper_path(Path::new(r"\\?\UNC\host\share\GitNova.exe")),
            "//host/share/GitNova.exe"
        );
        assert!(helper_value(Path::new(r"C:\user's folder\GitNova.exe")).contains(r"'\''"));
    }
}
