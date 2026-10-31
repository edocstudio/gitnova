use crate::err;
use gitnova_domain::*;
use std::{
    io::Read,
    process::{Child, Command, Stdio},
};
pub(crate) fn secure_token(prompt: &CredentialPrompt, identity: &str) -> Result<String> {
    for value in [&prompt.message, &prompt.confirm, &prompt.cancel] {
        if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
            return Err(err("Invalid secure account prompt."));
        }
    }
    if identity.is_empty() || identity.len() > 4096 || identity.chars().any(char::is_control) {
        return Err(err("Invalid secure account prompt."));
    }
    let mut child = launch(prompt, identity)?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or_else(|| err("Could not read secure account entry."))?
        .take(16385)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > 16384 {
        let _ = child.kill();
        let _ = child.wait();
        return Err(err(
            "The provider token could not be read or exceeded 16 KiB.",
        ));
    }
    if !child
        .wait()
        .map_err(|_| err("Secure account entry could not finish."))?
        .success()
    {
        return Err(err("Account connection canceled."));
    }
    let token = String::from_utf8(bytes)
        .map_err(|_| err("Invalid provider token."))?
        .trim()
        .to_owned();
    if token.is_empty() {
        return Err(err("Enter a provider access token."));
    }
    Ok(token)
}

#[cfg(target_os = "macos")]
fn launch(prompt: &CredentialPrompt, identity: &str) -> Result<Child> {
    let escape = |value: &str| value.replace('\\', "\\\\").replace('"', "\\\"");
    let message = escape(&format!("GitNova: {}. {}", identity, prompt.message));
    let confirm = escape(&prompt.confirm);
    let cancel = escape(&prompt.cancel);
    let script=format!("text returned of (display dialog \"{message}\" default answer \"\" with hidden answer buttons {{\"{cancel}\", \"{confirm}\"}} default button \"{confirm}\" cancel button \"{cancel}\")");
    let child = Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| err("Could not open secure account entry."))?;
    Ok(child)
}

#[cfg(target_os = "linux")]
fn launch(prompt: &CredentialPrompt, identity: &str) -> Result<Child> {
    Command::new("zenity")
        .args([
            "--password",
            "--title",
            &format!("GitNova: {identity}. {}", prompt.message),
            "--ok-label",
            &prompt.confirm,
            "--cancel-label",
            &prompt.cancel,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| err("Could not open secure account entry. Install Zenity on Linux."))
}

#[cfg(target_os = "windows")]
fn launch(prompt: &CredentialPrompt, identity: &str) -> Result<Child> {
    // Only public prompt text enters the command; the secret leaves over stdout.
    let quote = |s: &str| format!("'{}'", s.replace('\'', "''"));
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
$f = New-Object Windows.Forms.Form
$f.Text = 'GitNova'; $f.Width = 560; $f.Height = 240; $f.StartPosition = 'CenterScreen'
$l = New-Object Windows.Forms.Label
$l.Text = {}; $l.SetBounds(16,16,510,70); $f.Controls.Add($l)
$t = New-Object Windows.Forms.TextBox
$t.UseSystemPasswordChar = $true; $t.SetBounds(16,94,510,26); $f.Controls.Add($t)
$ok = New-Object Windows.Forms.Button
$ok.Text = {}; $ok.SetBounds(300,140,110,32); $ok.DialogResult = 'OK'; $f.Controls.Add($ok)
$cancel = New-Object Windows.Forms.Button
$cancel.Text = {}; $cancel.SetBounds(416,140,110,32); $cancel.DialogResult = 'Cancel'; $f.Controls.Add($cancel)
$f.AcceptButton = $ok; $f.CancelButton = $cancel
if ($f.ShowDialog() -eq 'OK') {{ [Console]::OutputEncoding = New-Object Text.UTF8Encoding($false); [Console]::Write($t.Text); $f.Dispose(); exit 0 }}
$f.Dispose(); exit 1
"#,
        quote(&format!("{identity}. {}", prompt.message)),
        quote(&prompt.confirm),
        quote(&prompt.cancel)
    );
    let mut command = Command::new("powershell.exe");
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000); // Hide console; the masked native form remains visible.
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-STA",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| err("Could not open secure account entry."))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_prompt_data_is_refused_before_launching_a_native_dialog() {
        for text in ["", "bad\ntext"] {
            let p = CredentialPrompt {
                message: text.into(),
                confirm: "Connect".into(),
                cancel: "Cancel".into(),
            };
            assert!(secure_token(&p, "fixture").is_err());
        }
        let p = CredentialPrompt {
            message: "Enter token".into(),
            confirm: "Connect".into(),
            cancel: "Cancel".into(),
        };
        assert!(secure_token(&p, "bad\nidentity").is_err());
    }
}
