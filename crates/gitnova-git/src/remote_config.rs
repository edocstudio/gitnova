//! Reviewed configuration only: no remote contact or hosted repository creation.
use crate::{
    clone::validate_clone_source,
    working::{run_write, OperationLock},
    GitCli, RepositoryReader, StatusReader,
};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
fn config_bytes(path: &Path) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(AppError::new(
            "remote_config",
            "Inspect a symbolic or non-regular repository config with Git before changing it.",
        ));
    }
    let mut data = vec![];
    fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut data)?;
    if data.len() > 1024 * 1024 {
        return Err(AppError::new(
            "safety_budget",
            "Repository config exceeds its 1 MiB review budget.",
        ));
    }
    Ok(data)
}
impl GitCli {
    pub fn preview_remote_add(
        &self,
        root: &Path,
        request: &RemoteAddRequest,
    ) -> Result<RemoteAddPreview> {
        if request.name.is_empty()
            || request.name.len() > 128
            || request.name.starts_with('-')
            || request
                .name
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(AppError::new(
                "remote_config",
                "Enter a valid remote name, such as origin.",
            ));
        }
        self.text(
            root,
            &[
                "check-ref-format",
                &format!("refs/remotes/{}/branch", request.name),
            ],
        )?;
        let url = validate_clone_source(request.url.trim())?;
        let effective = self.text(root, &["ls-remote", "--get-url", &url])?;
        validate_clone_source(effective.trim_end_matches('\n'))?;
        if self
            .text(root, &["remote"])?
            .lines()
            .any(|name| name == request.name)
        {
            return Err(AppError::new(
                "remote_config",
                "This remote name already exists. Choose a different name.",
            ));
        }
        if self.merge_state(root)?.phase != "none" {
            return Err(AppError::new(
                "remote_config",
                "Finish or recover the current operation before adding a remote.",
            ));
        }
        let d = self.discover(root)?;
        let original = config_bytes(&d.common_dir.join("config"))?;
        let normalized = RemoteAddRequest {
            name: request.name.clone(),
            url,
        };
        let state = self.status(root)?;
        let data = serde_json::to_vec(&(
            &normalized,
            &original,
            self.config_snapshot(root)?,
            state,
            NativePath::new(&d.common_dir),
        ))
        .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        let saved = format!(
            "gitnova-config-backup-{:x}.config",
            Sha256::digest(&original)
        );
        Ok(RemoteAddPreview {
            request: normalized,
            token: format!("{:x}", Sha256::digest(data)),
            recovery_path: NativePath::new(&d.common_dir.join(saved)),
        })
    }
    pub fn add_remote(&self, root: &Path, request: &RemoteAddRequest, token: &str) -> Result<()> {
        let p = self.preview_remote_add(root, request)?;
        if p.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Remote URL or repository configuration changed. Review again.",
            ));
        }
        let d = self.discover(root)?;
        let config = d.common_dir.join("config");
        let mut guard = OperationLock::acquire(d.common_dir.join("config.lock"))?;
        if self.preview_remote_add(root, request)?.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Repository configuration changed while acquiring the config lock. Review again.",
            ));
        }
        let original = config_bytes(&config)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&d.common_dir)?;
        temporary.write_all(&original)?;
        temporary.flush()?;
        let file = crate::process::git_path(temporary.path())?;
        for (key, value) in [
            (
                format!("remote.{}.url", p.request.name),
                p.request.url.clone(),
            ),
            (
                format!("remote.{}.fetch", p.request.name),
                format!("+refs/heads/*:refs/remotes/{}/*", p.request.name),
            ),
        ] {
            run_write(
                root,
                &[
                    "config".into(),
                    "--file".into(),
                    file.clone(),
                    "--add".into(),
                    key.into(),
                    value.into(),
                ],
                None,
            )?;
        }
        let new_config = config_bytes(temporary.path())?;
        if self.preview_remote_add(root, request)?.token != token {
            return Err(AppError::new("stale_preview","Repository configuration changed during preparation. The original config was preserved."));
        }
        let backup = p.recovery_path.to_path()?;
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
        guard.publish(&config, &new_config, None)?;
        let actual = self.text(
            root,
            &[
                "config",
                "--local",
                "--get-all",
                &format!("remote.{}.url", p.request.name),
            ],
        )?;
        let fetch = self.text(
            root,
            &[
                "config",
                "--local",
                "--get-all",
                &format!("remote.{}.fetch", p.request.name),
            ],
        )?;
        if actual.trim_end_matches('\n') != p.request.url
            || fetch.trim_end_matches('\n')
                != format!("+refs/heads/*:refs/remotes/{}/*", p.request.name)
        {
            return Err(AppError::new("operation_unknown","Remote configuration differs from the reviewed URL. Inspect the saved config before retrying; no automatic rollback was attempted."));
        }
        Ok(())
    }
}
