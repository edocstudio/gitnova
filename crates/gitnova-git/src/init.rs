use crate::{working::run_offline_write, GitCli, RepositoryReader};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Component, Path},
};

impl GitCli {
    pub fn preview_init(&self, request: &InitRequest) -> Result<InitPreview> {
        let invalid = || {
            AppError::new(
                "init",
                "Choose an absolute repository folder with an existing parent.",
            )
        };
        let path = Path::new(&request.destination);
        if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(invalid());
        }
        let name = path
            .file_name()
            .filter(|n| !n.eq_ignore_ascii_case(OsStr::new(".git")))
            .ok_or_else(invalid)?;
        let parent = fs::canonicalize(path.parent().ok_or_else(invalid)?)?;
        let destination = parent.join(name);
        let existing_folder = match fs::symlink_metadata(&destination) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => true,
            Ok(_) => {
                return Err(AppError::new(
                    "init",
                    "Repository destination must be a directory, not a file or symlink.",
                ))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.into()),
        };
        if fs::symlink_metadata(destination.join(".git")).is_ok()
            || self
                .discover(if existing_folder {
                    &destination
                } else {
                    &parent
                })
                .is_ok()
        {
            return Err(AppError::new(
                "init",
                "This folder is already in a Git repository. Open the existing repository instead.",
            ));
        }
        if request.branch.is_empty()
            || request.branch.len() > 255
            || request.branch.starts_with('-')
            || request.branch.chars().any(char::is_control)
        {
            return Err(AppError::new("init", "Enter a valid initial branch name."));
        }

        self.read(
            &parent,
            &[
                OsStr::new("check-ref-format"),
                OsStr::new("--branch"),
                OsStr::new(&request.branch),
            ],
            4096,
        )
        .map_err(|_| AppError::new("init", "Enter a valid initial branch name."))?;
        let identity = |p: &Path| -> Result<String> {
            let m = fs::metadata(p)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                Ok(format!("{}:{}", m.dev(), m.ino()))
            }
            #[cfg(not(unix))]
            {
                Ok(format!("{:?}", m.created()))
            }
        };
        let destination = NativePath::new(&destination);
        let token = format!(
            "{:x}",
            Sha256::digest(format!(
                "init-v1\0{}\0{}\0{}\0{}\0{}",
                request.workspace_id,
                destination.hex,
                request.branch,
                identity(&parent)?,
                if existing_folder {
                    identity(&destination.to_path()?)?
                } else {
                    "new".into()
                }
            ))
        );
        Ok(InitPreview {
            request: request.clone(),
            destination,
            existing_folder,
            token,
        })
    }
    pub fn init_repository(&self, request: &InitRequest, token: &str) -> Result<()> {
        let preview = self.preview_init(request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Repository destination changed. Review initialization again.",
            ));
        }
        let path = preview.destination.to_path()?;
        if !preview.existing_folder {
            fs::create_dir(&path)?;
        }
        // Exclusive reservation prevents reinitializing existing Git metadata, including gitfiles.
        fs::create_dir(path.join(".git"))?;
        let args: Vec<OsString> = vec![
            "init".into(),
            "--template=".into(),
            "--object-format=sha1".into(),
            format!("--initial-branch={}", request.branch).into(),
            "--".into(),
            path.as_os_str().into(),
        ];
        run_offline_write(&path, &args).map_err(|e| AppError::new("init", format!("Initialization stopped: {}. Files and partial Git metadata are retained; inspect the destination before retrying.", e.message)))
    }
}
