use crate::{progress::emit, working::run_clone, GitCli, RepositoryReader};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{ffi::OsString, fs, path::Path};

pub(crate) fn validate_clone_source(value: &str) -> Result<String> {
    if value.is_empty()
        || value.len() > 8192
        || value.chars().any(char::is_control)
        || value.starts_with('-')
    {
        return Err(AppError::new(
            "clone_source",
            "Enter a standard Git URL or an absolute local repository path.",
        ));
    }
    if Path::new(value).is_absolute() {
        let path = fs::canonicalize(value)?;
        if !path.is_dir() {
            return Err(AppError::new(
                "clone_source",
                "Local clone source must be a repository directory.",
            ));
        }
        return path.to_str().map(str::to_owned).ok_or_else(|| {
            AppError::new(
                "clone_source",
                "Enter a source path representable as UTF-8.",
            )
        });
    }
    if value.chars().any(char::is_whitespace) || value.contains(['?', '#']) || value.contains("::")
    {
        return Err(AppError::new("clone_source", "Use a credential helper or SSH agent; embedded credentials, query strings and custom transports are unsupported."));
    }
    if let Some((scheme, rest)) = value.split_once("://") {
        if !["https", "http", "ssh", "git", "file"].contains(&scheme) || rest.is_empty() {
            return Err(AppError::new(
                "clone_source",
                "Enter a standard Git URL or an absolute local repository path.",
            ));
        }
        let authority = rest.split('/').next().unwrap_or("");
        if let Some((user, _)) = authority.rsplit_once('@') {
            if scheme != "ssh" || user.contains(':') || user.is_empty() {
                return Err(AppError::new("clone_source", "Use a credential helper or SSH agent; embedded credentials, query strings and custom transports are unsupported."));
            }
        }
        return Ok(value.into());
    }
    // scp-style SSH source, such as git@host:group/repository.git.
    if let Some((host, path)) = value.split_once(':') {
        if !host.is_empty() && !host.contains('/') && !host.starts_with('-') && !path.is_empty() {
            return Ok(value.into());
        }
    }
    Err(AppError::new(
        "clone_source",
        "Enter a standard Git URL or an absolute local repository path.",
    ))
}

impl GitCli {
    pub fn preview_clone(&self, request: &CloneRequest) -> Result<ClonePreview> {
        let source = validate_clone_source(request.source.trim())?;
        let sandbox = tempfile::tempdir()?;
        // --get-url is configuration-only; previews never contact the source.
        let effective = self.text(sandbox.path(), &["ls-remote", "--get-url", "--", &source])?;
        let effective = validate_clone_source(effective.trim_end_matches('\n'))?;
        let resolved = self.text(
            sandbox.path(),
            &["ls-remote", "--get-url", "--", &effective],
        )?;
        if resolved.trim_end_matches('\n') != effective {
            return Err(AppError::new(
                "clone_source",
                "Chained URL rewrites require manual inspection before cloning.",
            ));
        }
        let destination = Path::new(&request.destination);
        if request.destination.len() > 8192
            || !destination.is_absolute()
            || destination
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(AppError::new(
                "clone_destination",
                "Choose an absolute path for a new clone folder.",
            ));
        }
        let name = destination
            .file_name()
            .filter(|name| !name.eq_ignore_ascii_case(std::ffi::OsStr::new(".git")))
            .ok_or_else(|| {
                AppError::new(
                    "clone_destination",
                    "Choose a new repository folder, not a root or .git directory.",
                )
            })?;
        let parent = fs::canonicalize(destination.parent().ok_or_else(|| {
            AppError::new(
                "clone_destination",
                "The clone parent folder must already exist.",
            )
        })?)?;
        let metadata = fs::metadata(&parent)?;
        if !metadata.is_dir() {
            return Err(AppError::new(
                "clone_destination",
                "The clone parent folder must already exist.",
            ));
        }
        let destination = parent.join(name);
        match fs::symlink_metadata(&destination) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(error.into()),
            Ok(_) => return Err(AppError::new("clone_destination", "Clone destination already exists. Choose a new folder; existing content will not be overwritten.")),
        }
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            format!("{}:{}", metadata.dev(), metadata.ino())
        };
        #[cfg(not(unix))]
        let identity = crate::file_identity::directory(&parent)?;
        let destination = NativePath::new(&destination);
        let request = CloneRequest {
            workspace_id: request.workspace_id.clone(),
            source: effective,
            destination: request.destination.clone(),
        };
        let token = format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "clone-v1\0{}\0{}\0{}\0{}",
                    request.workspace_id, request.source, destination.hex, identity
                )
                .as_bytes()
            )
        );
        Ok(ClonePreview {
            version: 1,
            request,
            destination,
            token,
        })
    }
    pub fn clone_repository(&self, request: &CloneRequest, token: &str) -> Result<()> {
        let preview = self.preview_clone(request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Clone source, destination or parent changed. Review again.",
            ));
        }
        let destination = preview.destination.to_path()?;
        // Exclusive reservation: never adopt an existing directory, even if empty.
        fs::create_dir(&destination)?;
        let sandbox = tempfile::tempdir()?;
        let empty_template = sandbox.path().join("empty-template");
        fs::create_dir(&empty_template)?;
        emit("cloning", 0, 0, "steps");
        let mut args: Vec<OsString> = [
            "-c",
            "core.hooksPath=",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
            "-c",
            "pack.threads=1",
            "-c",
            "index.threads=1",
            "clone",
            "--no-local",
            "--no-recurse-submodules",
            "--origin",
            "origin",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        // Absolute empty hook/template directories override configured templates for this command only.
        let empty_template_arg = crate::process::git_path(&empty_template)?;
        let mut hooks = OsString::from("core.hooksPath=");
        hooks.push(&empty_template_arg);
        args[1] = hooks;
        let mut template = OsString::from("--template=");
        template.push(empty_template_arg);
        args.push(template);
        args.extend([
            "--".into(),
            preview.request.source.into(),
            crate::process::git_path(&destination)?,
        ]);
        run_clone(sandbox.path(), &args)?;
        emit("verifying-clone", 0, 0, "steps");
        let discovered = self.discover(&destination)?;
        if discovered.root.as_deref() != Some(destination.as_path()) {
            return Err(AppError::new(
                "clone_result",
                "Clone result needs inspection before opening it.",
            ));
        }
        Ok(())
    }
}
