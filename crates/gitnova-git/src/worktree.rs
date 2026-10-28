//! Linked worktree creation at an explicitly reviewed local commit.
use crate::{working::run_offline_write, GitCli, RefReader, RepositoryReader};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{ffi::OsStr, fs, io::Read, path::Path};

fn request_error(message: &str) -> AppError {
    AppError::new("worktree_request", message)
}
impl GitCli {
    pub fn preview_create_worktree(
        &self,
        root: &Path,
        request: &WorktreeCreateRequest,
    ) -> Result<WorktreeCreatePreview> {
        crate::validate_oid(&request.oid)?;
        if self.text(root, &["cat-file", "-t", &request.oid])?.trim() != "commit" {
            return Err(request_error("Choose an exact commit object."));
        }
        let input = Path::new(&request.destination);
        if request.destination.len() > 4096
            || !input.is_absolute()
            || request
                .destination
                .split(std::path::is_separator)
                .any(|part| part == "." || part == "..")
        {
            return Err(request_error(
                "Enter an absolute destination without dot components.",
            ));
        }
        let parent = input
            .parent()
            .ok_or_else(|| request_error("Choose a new folder inside an existing directory."))?
            .canonicalize()?;
        if !parent.is_dir() {
            return Err(request_error(
                "The destination parent must be an existing directory.",
            ));
        }
        let leaf = input
            .file_name()
            .ok_or_else(|| request_error("Choose a new folder inside an existing directory."))?;
        if leaf == ".git" {
            return Err(request_error(
                "Choose a destination outside Git administrative directories.",
            ));
        }
        let destination = parent.join(leaf);
        let (discovery, state) = self.write_state(root)?;
        let refs = self.refs(root)?;
        let trees = self
            .read(
                root,
                &[
                    OsStr::new("worktree"),
                    OsStr::new("list"),
                    OsStr::new("--porcelain"),
                    OsStr::new("-z"),
                ],
                1024 * 1024,
            )?
            .bytes;
        let config = self
            .read(
                root,
                &[
                    OsStr::new("config"),
                    OsStr::new("--null"),
                    OsStr::new("--list"),
                    OsStr::new("--show-origin"),
                ],
                1024 * 1024,
            )?
            .bytes;
        let mut blocked = None;
        match fs::symlink_metadata(&destination) {
            Ok(_) => blocked = Some("The destination already exists. Choose a new folder; existing content is never replaced.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
        let inside_repository = if parent.starts_with(&discovery.common_dir)
            || parent.starts_with(&discovery.git_dir)
        {
            true
        } else {
            match self.discover(&parent) {
                Ok(_) => true,
                Err(error)
                    if error.code == "git_exit_128"
                        && error
                            .message
                            .starts_with("fatal: not a git repository (or any") =>
                {
                    false
                }
                Err(error) => return Err(error),
            }
        };
        if parent.starts_with(&discovery.common_dir)
            || parent.starts_with(&discovery.git_dir)
            || inside_repository
        {
            blocked = Some("Choose a destination outside existing Git repositories and administrative directories.".into());
        }
        // Git metadata can be outside the worktree; also protect every linked root, including missing entries.
        for field in trees.split(|b| *b == 0) {
            if let Some(value) = field.strip_prefix(b"worktree ") {
                let path = crate::path_from_bytes(value.to_vec())?;
                if destination == path || parent.starts_with(&path) {
                    blocked = Some("Choose a destination outside existing Git repositories and administrative directories.".into());
                }
            }
        }
        if discovery.git_dir.join("index.lock").exists()
            || discovery.git_dir.join("HEAD.lock").exists()
            || discovery.common_dir.join("packed-refs.lock").exists()
        {
            blocked = Some(
                "Another Git operation holds an index or HEAD lock. Its locks will not be removed."
                    .into(),
            );
        }
        if request.mode == WorktreeCreateMode::Detached {
            if !request.branch.is_empty() {
                return Err(request_error(
                    "Detached worktrees do not accept a branch name.",
                ));
            }
        } else {
            if request.branch.is_empty()
                || request.branch.len() > 256
                || request.branch.starts_with('-')
                || request.branch == "HEAD"
            {
                return Err(request_error(
                    "Enter a valid branch name (up to 256 bytes).",
                ));
            }
            let name = format!("refs/heads/{}", request.branch);
            self.text(root, &["check-ref-format", &name])
                .map_err(|_| request_error("This branch name is not valid in Git."))?;
            let existing = refs.iter().find(|r| r.name == name);
            match request.mode {
                WorktreeCreateMode::NewBranch if existing.is_some() => blocked = Some(
                    "That branch already exists. Choose an available branch or enter a new name."
                        .into(),
                ),
                WorktreeCreateMode::ExistingBranch => {
                    if existing.is_none_or(|r| r.oid != request.oid) {
                        blocked = Some("The selected local branch no longer points to this commit. Select it again in the graph.".into());
                    }
                    if trees
                        .split(|b| *b == 0)
                        .any(|field| field == format!("branch {name}").as_bytes())
                    {
                        blocked = Some("This branch is already checked out in another worktree. Choose another branch or detached HEAD.".into());
                    }
                }
                _ => (),
            }
            if discovery.common_dir.join(format!("{name}.lock")).exists() {
                blocked = Some(
                    "Another Git operation holds this branch lock. Its lock will not be removed."
                        .into(),
                );
            }
        }
        let tree = self
            .read(
                root,
                &[
                    OsStr::new("ls-tree"),
                    OsStr::new("-r"),
                    OsStr::new("-z"),
                    OsStr::new("--full-tree"),
                    OsStr::new(&request.oid),
                ],
                4 * 1024 * 1024,
            )?
            .bytes;
        let mut files = 0;
        let mut submodules = 0;
        for entry in tree.split(|b| *b == 0).filter(|v| !v.is_empty()) {
            files += 1;
            submodules += usize::from(entry.starts_with(b"160000 "));
        }
        if files > 10_000 {
            return Err(request_error(
                "This worktree exceeds the complete review limit of 10,000 paths.",
            ));
        }
        let metadata = fs::metadata(&parent)?;
        #[cfg(unix)]
        let parent_identity = {
            use std::os::unix::fs::MetadataExt;
            format!("{}:{}", metadata.dev(), metadata.ino())
        };
        #[cfg(not(unix))]
        let parent_identity = format!("{:?}", metadata.created().ok());
        let bytes = serde_json::to_vec(&(
            request,
            NativePath::new(&destination),
            NativePath::new(&discovery.common_dir),
            NativePath::new(&discovery.git_dir),
            state,
            refs,
            trees,
            config,
            parent_identity,
            &blocked,
        ))
        .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        Ok(WorktreeCreatePreview {
            version: 1,
            request: request.clone(),
            destination: NativePath::new(&destination),
            source: NativePath::new(discovery.root.as_ref().unwrap()),
            files,
            submodules,
            blocked_reason: blocked,
            token: format!("{:x}", Sha256::digest(bytes)),
        })
    }
    pub fn create_worktree(
        &self,
        root: &Path,
        request: &WorktreeCreateRequest,
        token: &str,
    ) -> Result<()> {
        let preview = self.preview_create_worktree(root, request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Worktree destination, branch or repository changed. Review creation again.",
            ));
        }
        if let Some(reason) = preview.blocked_reason {
            return Err(AppError::new("worktree_blocked", reason));
        }
        let source = self.discover(root)?;
        let mut args = vec![
            "-c".into(),
            "submodule.recurse=false".into(),
            "-c".into(),
            "fetch.recurseSubmodules=false".into(),
            "worktree".into(),
            "add".into(),
            "--no-guess-remote".into(),
        ];
        match request.mode {
            WorktreeCreateMode::NewBranch => args.extend([
                "--no-track".into(),
                "-b".into(),
                request.branch.clone().into(),
            ]),
            WorktreeCreateMode::Detached => args.push("--detach".into()),
            WorktreeCreateMode::ExistingBranch => (),
        }
        args.push("--".into());
        args.push(preview.destination.to_path()?.into_os_string());
        args.push(if request.mode == WorktreeCreateMode::ExistingBranch {
            request.branch.clone().into()
        } else {
            request.oid.clone().into()
        });
        // Claim only an absent destination. Git itself also accepts pre-existing empty folders;
        // create_dir prevents us from taking over a folder created after the last review.
        fs::create_dir(preview.destination.to_path()?).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                AppError::new(
                    "stale_preview",
                    "Worktree destination, branch or repository changed. Review creation again.",
                )
            } else {
                error.into()
            }
        })?;
        run_offline_write(root, &args)?;
        let created = self.discover(&preview.destination.to_path()?)?;
        let head = self.text(
            &preview.destination.to_path()?,
            &["rev-parse", "--verify", "HEAD"],
        )?;
        let branch = self.optional_text(
            &preview.destination.to_path()?,
            &["symbolic-ref", "--quiet", "HEAD"],
        )?;
        let expected = if request.mode == WorktreeCreateMode::Detached {
            None
        } else {
            Some(format!("refs/heads/{}", request.branch))
        };
        if created.common_dir != source.common_dir
            || created
                .root
                .as_ref()
                .map(|path| NativePath::new(path))
                .as_ref()
                != Some(&preview.destination)
            || head.trim() != request.oid
            || branch.as_deref().map(str::trim) != expected.as_deref()
        {
            return Err(AppError::new("operation_unknown", "Worktree creation finished with unexpected state. Inspect the destination, branch and worktree metadata before retrying."));
        }
        Ok(())
    }
}

impl GitCli {
    pub fn preview_worktree_maintenance(
        &self,
        root: &Path,
        request: &WorktreeMaintenanceRequest,
    ) -> Result<WorktreeMaintenancePreview> {
        if !["lock", "unlock", "repair"].contains(&request.kind.as_str())
            || request.reason.len() > 512
            || request.reason.contains(['\0', '\n', '\r'])
        {
            return Err(request_error(
                "Choose lock, unlock or repair with a short single-line reason.",
            ));
        }
        let d = self.discover(root)?;
        let (_, state) = self.write_state(root)?;
        let trees = self.linked_worktrees(root)?;
        let wanted = request.path.to_path()?;
        if !wanted.is_absolute() {
            return Err(request_error(
                "Choose an absolute registered worktree path.",
            ));
        }
        let canonical = wanted.canonicalize()?;
        let tree = trees
            .iter()
            .find(|t| t.path.to_path().ok().as_deref() == Some(canonical.as_path()))
            .ok_or_else(|| request_error("Choose a registered linked worktree."))?;
        let selected = request.path.to_path()?;
        let mut blocked = None;
        if tree.bare || d.root.as_ref() == Some(&selected) || selected.join(".git").is_dir() {
            blocked = Some("Choose a linked worktree, not the main checkout.".into());
        }
        if (tree.prunable && request.kind != "repair") || !selected.is_dir() {
            blocked=Some("This checkout is missing. Inspect its retained worktree metadata before maintenance.".into());
        }
        if request.kind == "lock" && tree.locked {
            blocked = Some("This worktree is already locked.".into());
        }
        if request.kind == "unlock" && !tree.locked {
            blocked = Some("This worktree is not locked.".into());
        }
        if fs::symlink_metadata(&selected).is_ok_and(|m| m.file_type().is_symlink())
            || fs::symlink_metadata(selected.join(".git")).is_ok_and(|m| m.file_type().is_symlink())
        {
            blocked =
                Some("Worktree metadata through symbolic links needs manual inspection.".into());
        }
        if let Ok(other) = self.discover(&selected) {
            if other.common_dir != d.common_dir {
                blocked = Some("The selected checkout now belongs to another repository.".into());
            }
        }
        let raw = self
            .read(
                root,
                &[
                    "worktree".as_ref(),
                    "list".as_ref(),
                    "--porcelain".as_ref(),
                    "-z".as_ref(),
                ],
                1024 * 1024,
            )?
            .bytes;
        let gitfile = if fs::symlink_metadata(selected.join(".git")).is_ok_and(|m| m.is_file()) {
            let mut bytes = Vec::new();
            fs::File::open(selected.join(".git"))?
                .take(65537)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 65536 {
                return Err(request_error("Worktree locator exceeds its review budget."));
            }
            bytes
        } else {
            vec![]
        };
        let mut repair_locators = Vec::new();
        if request.kind == "repair" {
            if trees.len() > 256 {
                return Err(request_error(
                    "Worktree repair exceeds the 256-checkout review budget.",
                ));
            }
            // Native repair without path arguments fixes known forward locators.
            // It can inspect every registered checkout, so bind each one and
            // refuse missing, redirected or symbolic-link destinations up front.
            for other in &trees {
                let path = other.path.to_path()?;
                if !path.is_dir() || fs::symlink_metadata(&path)?.file_type().is_symlink() {
                    blocked = Some("Inspect missing or symbolic-link worktree locations before repairing metadata.".into());
                    continue;
                }
                let locator = path.join(".git");
                let contents = match fs::symlink_metadata(&locator) {
                    Ok(m) if m.file_type().is_symlink() => {
                        blocked = Some(
                            "Worktree metadata through symbolic links needs manual inspection."
                                .into(),
                        );
                        vec![]
                    }
                    Ok(m) if m.is_file() => {
                        let mut value = Vec::new();
                        fs::File::open(&locator)?
                            .take(65537)
                            .read_to_end(&mut value)?;
                        if value.len() > 65536 {
                            return Err(request_error(
                                "Worktree locator exceeds its review budget.",
                            ));
                        }
                        value
                    }
                    _ => vec![],
                };
                if let Ok(discovered) = self.discover(&path) {
                    if discovered.common_dir != d.common_dir {
                        blocked = Some("A registered checkout now belongs to another repository. Inspect before repair.".into());
                    }
                }
                repair_locators.push((other.path.clone(), contents));
            }
        }
        let bytes = serde_json::to_vec(&(
            request,
            &state,
            &raw,
            &gitfile,
            &repair_locators,
            NativePath::new(&d.common_dir),
            self.config_snapshot(root)?,
        ))
        .map_err(|e| request_error(&e.to_string()))?;
        Ok(WorktreeMaintenancePreview {
            request: request.clone(),
            tree: tree.clone(),
            affected: if request.kind == "repair" {
                trees
            } else {
                vec![tree.clone()]
            },
            token: format!("{:x}", Sha256::digest(bytes)),
            blocked_reason: blocked,
        })
    }
    pub fn maintain_worktree(
        &self,
        root: &Path,
        request: &WorktreeMaintenanceRequest,
        token: &str,
    ) -> Result<()> {
        let p = self.preview_worktree_maintenance(root, request)?;
        if p.token != token {
            return Err(request_error(
                "Worktree metadata changed. Review maintenance again.",
            ));
        }
        if let Some(reason) = p.blocked_reason {
            return Err(request_error(&reason));
        }
        let mut args = vec!["worktree".into(), request.kind.clone().into()];
        if request.kind == "lock" && !request.reason.is_empty() {
            args.extend(["--reason".into(), request.reason.clone().into()]);
        }
        // Passing a broken checkout locator as an explicit path makes native
        // Git try reverse discovery and fail before repairing that locator.
        // All known checkout locations were reviewed above for repository repair.
        if request.kind != "repair" {
            args.push(request.path.to_path()?.into_os_string());
        }
        run_offline_write(root, &args)?;
        let now = self.linked_worktrees(root)?;
        let canonical = request.path.to_path()?.canonicalize()?;
        let tree = now
            .iter()
            .find(|t| t.path.to_path().ok().as_deref() == Some(canonical.as_path()))
            .ok_or_else(|| {
                request_error("Worktree metadata changed unexpectedly. Inspect with Git.")
            })?;
        if tree.head != p.tree.head
            || tree.branch != p.tree.branch
            || request.kind == "lock" && !tree.locked
            || request.kind == "unlock" && tree.locked
        {
            return Err(request_error("Worktree maintenance result needs inspection. No checkout was selected for deletion."));
        }
        if request.kind == "repair" {
            let d = self.discover(root)?;
            let selected = self.discover(&request.path.to_path()?)?;
            if d.common_dir != selected.common_dir {
                return Err(request_error(
                    "Worktree repair did not restore the expected repository identity.",
                ));
            }
        }
        Ok(())
    }
}
