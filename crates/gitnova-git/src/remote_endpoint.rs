//! Snapshot-bound endpoint migration. Configuration only; never contacts a host.
use crate::{
    clone::validate_clone_source,
    remote_config::config_bytes,
    working::{run_write, OperationLock},
    GitCli, RepositoryReader,
};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{ffi::OsString, fs, io::Write, path::Path};

impl GitCli {
    fn endpoint_values(
        &self,
        root: &Path,
        config: Option<&Path>,
        key: &str,
    ) -> Result<Vec<String>> {
        let mut args = vec![OsString::from("config")];
        if let Some(config) = config {
            args.extend([
                "--file".into(),
                crate::process::git_path(config)?,
                "--no-includes".into(),
            ]);
        }
        args.extend(["--null".into(), "--get-all".into(), key.into()]);
        let refs = args.iter().map(OsString::as_os_str).collect::<Vec<_>>();
        let output = match self.read(root, &refs, 128 * 1024) {
            Ok(value) => value.bytes,
            Err(error) if error.code == "git_exit_1" => return Ok(vec![]),
            Err(error) => return Err(error),
        };
        let text = String::from_utf8(output)
            .map_err(|_| AppError::new("remote_config", "Remote URLs must be UTF-8."))?;
        let values = text
            .split_terminator('\0')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if values.len() > 8
            || values
                .iter()
                .any(|value| value.len() > 8192 || value.chars().any(char::is_control))
        {
            return Err(AppError::new(
                "remote_config",
                "Edit complex remote URL configuration with Git before using this dialog.",
            ));
        }
        // Do not return embedded credentials to UI state or receipts.
        for value in &values {
            if let Some((scheme, rest)) = value.split_once("://") {
                let authority = rest.split('/').next().unwrap_or("");
                let embedded = authority.rsplit_once('@').is_some_and(|(user, _)| {
                    scheme != "ssh" || user.is_empty() || user.contains(':')
                });
                if embedded || value.contains(['?', '#']) {
                    return Err(AppError::new("remote_config", "Remove embedded credentials from the remote URL with Git before using this dialog."));
                }
            }
        }
        Ok(values)
    }

    pub fn remote_endpoint(&self, root: &Path, name: &str) -> Result<RemoteEndpointSettings> {
        if name.is_empty()
            || name.len() > 128
            || name.starts_with('-')
            || name.chars().any(|c| c.is_whitespace() || c.is_control())
            || !self
                .text(root, &["remote"])?
                .lines()
                .any(|value| value == name)
        {
            return Err(AppError::new(
                "remote_config",
                "Choose an existing repository remote.",
            ));
        }
        if self.merge_state(root)?.phase != "none" {
            return Err(AppError::new(
                "remote_config",
                "Finish or recover the current operation before changing a remote.",
            ));
        }
        let discovered = self.discover(root)?;
        let config = discovered.common_dir.join("config");
        let original = config_bytes(&config)?;
        let mut fields = vec![];
        for suffix in ["url", "pushurl"] {
            let key = format!("remote.{name}.{suffix}");
            let effective = self.endpoint_values(root, None, &key)?;
            let local = self.endpoint_values(root, Some(&config), &key)?;
            if effective != local {
                return Err(AppError::new(
                    "remote_config",
                    "Remote URLs from included, global or worktree config must be edited with Git.",
                ));
            }
            fields.push(local);
        }
        let request = RemoteEndpointRequest {
            name: name.into(),
            urls: fields.remove(0),
            push_urls: fields.remove(0),
        };
        let data = serde_json::to_vec(&(
            &request,
            &original,
            self.config_snapshot(root)?,
            NativePath::new(&discovered.common_dir),
        ))
        .map_err(|error| AppError::new("snapshot", error.to_string()))?;
        if config_bytes(&config)? != original {
            return Err(AppError::new(
                "stale_preview",
                "Repository configuration changed. Review again.",
            ));
        }
        Ok(RemoteEndpointSettings {
            request,
            token: format!("{:x}", Sha256::digest(data)),
        })
    }

    pub fn preview_remote_endpoint(
        &self,
        root: &Path,
        request: &RemoteEndpointRequest,
        settings_token: &str,
    ) -> Result<RemoteEndpointPreview> {
        let previous = self.remote_endpoint(root, &request.name)?;
        if previous.token != settings_token {
            return Err(AppError::new(
                "stale_preview",
                "Remote configuration changed. Reload the current URLs and review again.",
            ));
        }
        if request.urls.len() != 1 || request.push_urls.len() > 1 {
            return Err(AppError::new(
                "remote_config",
                "Enter one fetch URL and at most one separate push URL.",
            ));
        }
        let validate = |values: &[String]| -> Result<Vec<String>> {
            values
                .iter()
                .map(|value| {
                    let url = validate_clone_source(value.trim())?;
                    let effective = self.text(root, &["ls-remote", "--get-url", &url])?;
                    validate_clone_source(effective.trim_end_matches('\n'))?;
                    Ok(url)
                })
                .collect()
        };
        let normalized = RemoteEndpointRequest {
            name: request.name.clone(),
            urls: validate(&request.urls)?,
            push_urls: validate(&request.push_urls)?,
        };
        if normalized == previous.request {
            return Err(AppError::new(
                "remote_config",
                "Change a remote URL before reviewing.",
            ));
        }
        let discovered = self.discover(root)?;
        let original = config_bytes(&discovered.common_dir.join("config"))?;
        let token = serde_json::to_vec(&(&normalized, &previous.token))
            .map_err(|error| AppError::new("snapshot", error.to_string()))?;
        let recovery_path = NativePath::new(&discovered.common_dir.join(format!(
            "gitnova-config-backup-{:x}.config",
            Sha256::digest(&original)
        )));
        Ok(RemoteEndpointPreview {
            request: normalized,
            previous,
            token: format!("{:x}", Sha256::digest(token)),
            recovery_path,
        })
    }

    pub fn change_remote_endpoint(
        &self,
        root: &Path,
        request: &RemoteEndpointRequest,
        settings_token: &str,
        token: &str,
    ) -> Result<()> {
        let preview = self.preview_remote_endpoint(root, request, settings_token)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Remote configuration review changed. Review again.",
            ));
        }
        let discovered = self.discover(root)?;
        let config = discovered.common_dir.join("config");
        let mut lock = OperationLock::acquire(discovered.common_dir.join("config.lock"))?;
        let original = config_bytes(&config)?;
        if self.remote_endpoint(root, &request.name)?.token != settings_token {
            return Err(AppError::new(
                "stale_preview",
                "Repository configuration changed. Review again.",
            ));
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&discovered.common_dir)?;
        temporary.write_all(&original)?;
        temporary.flush()?;
        let file = crate::process::git_path(temporary.path())?;
        for (suffix, values, old) in [
            ("url", &preview.request.urls, &preview.previous.request.urls),
            (
                "pushurl",
                &preview.request.push_urls,
                &preview.previous.request.push_urls,
            ),
        ] {
            let key = format!("remote.{}.{suffix}", request.name);
            if !old.is_empty() {
                run_write(
                    root,
                    &[
                        "config".into(),
                        "--file".into(),
                        file.clone(),
                        "--unset-all".into(),
                        key.clone().into(),
                    ],
                    None,
                )?;
            }
            for value in values {
                run_write(
                    root,
                    &[
                        "config".into(),
                        "--file".into(),
                        file.clone(),
                        "--add".into(),
                        key.clone().into(),
                        value.into(),
                    ],
                    None,
                )?;
            }
        }
        let updated = config_bytes(temporary.path())?;
        if self.remote_endpoint(root, &request.name)?.token != settings_token {
            return Err(AppError::new(
                "stale_preview",
                "Repository configuration changed. Review again.",
            ));
        }
        let backup = preview.recovery_path.to_path()?;
        if backup.exists() {
            if config_bytes(&backup)? != original {
                return Err(AppError::new(
                    "remote_config",
                    "The config recovery location is occupied. Inspect it before retrying.",
                ));
            }
        } else {
            let mut saved = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&backup)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                saved.set_permissions(fs::Permissions::from_mode(0o600))?;
            }
            saved.write_all(&original)?;
            saved.sync_all()?;
        }
        lock.publish(&config, &updated, None)?;
        if self.remote_endpoint(root, &request.name)?.request != preview.request {
            return Err(AppError::new("operation_unknown", "Remote configuration differs from the review. Inspect the saved config before retrying."));
        }
        Ok(())
    }
}
