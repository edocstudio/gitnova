//! Selected registration changes. Child files and object stores are never deleted.
use crate::{checked_child, module_names};
use gitnova_domain::*;
use gitnova_git::{GitCli, RepositoryReader, StatusReader, WorkingTreeReader};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

fn err(message: &str) -> AppError {
    AppError::new("submodule_lifecycle", message)
}
fn bytes(path: &Path, limit: usize) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
        Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => Err(err(
            "Registration metadata must be a regular file, not a symbolic link.",
        )),
        Ok(_) => {
            let mut value = Vec::new();
            fs::File::open(path)?
                .take((limit + 1) as u64)
                .read_to_end(&mut value)?;
            if value.len() > limit {
                return Err(err(
                    "Registration metadata exceeds its complete review budget.",
                ));
            }
            Ok(Some(value))
        }
    }
}
fn unrelated_index(git: &GitCli, root: &Path, selected: &NativePath) -> Result<Vec<Vec<u8>>> {
    let selected = selected.to_path()?;
    let raw = git
        .read(
            root,
            &["ls-files".as_ref(), "--stage".as_ref(), "-z".as_ref()],
            8 * 1024 * 1024,
        )?
        .bytes;
    let mut kept = Vec::new();
    for record in raw.split(|b| *b == 0).filter(|v| !v.is_empty()) {
        let tab = record
            .iter()
            .position(|b| *b == b'\t')
            .ok_or_else(|| err("Invalid index metadata."))?;
        let path = gitnova_git::path_from_bytes(record[tab + 1..].to_vec())?;
        if path != selected && path != Path::new(".gitmodules") {
            kept.push(record.to_vec());
        }
    }
    Ok(kept)
}
fn private_file(path: &Path, value: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(value)?;
    file.sync_all()?;
    Ok(())
}
pub fn preview(
    git: &GitCli,
    root: &Path,
    request: &SubmoduleLifecycleRequest,
) -> Result<SubmoduleLifecyclePreview> {
    if ["stage", "unstage"].contains(&request.kind.as_str()) {
        return git.preview_registration_index(root, request);
    }
    if !["add", "remove"].contains(&request.kind.as_str()) {
        return Err(err("Choose add or remove registration."));
    }
    // UUID identifies a one-shot recovery directory; core supplies it on the first review.
    if request.id.len() != 32 || !request.id.bytes().all(|v| v.is_ascii_hexdigit()) {
        return Err(err("Review the registration change before confirming."));
    }
    let path = std::path::PathBuf::from(&request.path);
    let value = path
        .to_str()
        .ok_or_else(|| err("Use a UTF-8 submodule registration path."))?;
    if value.len() > 512
        || value.chars().any(char::is_control)
        || path.components().any(|p| {
            p.as_os_str()
                .eq_ignore_ascii_case(std::ffi::OsStr::new(".gitmodules"))
        })
    {
        return Err(err(
            "Choose a short repository-relative submodule folder, not .gitmodules.",
        ));
    }
    let child = checked_child(root, &path)?;
    let d = git.discover(root)?;
    let status = git.status(root)?;
    let parent_state = git.head_index_snapshot(root)?;
    let changes = git.working_changes(root)?;
    let names = module_names(git, root)?;
    let modules = bytes(&root.join(".gitmodules"), 1024 * 1024)?;
    // head_index_snapshot already binds index bytes; do not expand a large index
    // into a JSON byte array just to hash it again.
    unrelated_index(git, root, &NativePath::new(&path))?;
    let config = git
        .read(
            root,
            &[
                "config".as_ref(),
                "--null".as_ref(),
                "--list".as_ref(),
                "--show-origin".as_ref(),
            ],
            1024 * 1024,
        )?
        .bytes;
    let mut blocked = None;
    if changes.files.iter().any(|f| {
        f.path
            .to_path()
            .is_ok_and(|p| p == Path::new(".gitmodules"))
    }) {
        blocked =
            Some("Commit or preserve .gitmodules edits before changing registrations.".into());
    }
    for file in [
        &d.git_dir.join("index.lock"),
        &d.common_dir.join("config.lock"),
    ] {
        if file.exists() {
            blocked =
                Some("Another Git operation holds a registration lock. Wait for its owner.".into());
        }
    }
    let mut child_head = None;
    let mut child_snapshot = None;
    let mut download = false;
    let mut source = request.source.clone();
    if request.kind == "remove" {
        if !request.source.is_empty() {
            return Err(err("Removing a registration does not accept a source URL."));
        }
        if !names.contains_key(&path) {
            return Err(err("Choose a submodule recorded directly in this repository. Open its parent to manage a nested registration."));
        }
        let raw = git
            .read(
                root,
                &[
                    "ls-files".as_ref(),
                    "--stage".as_ref(),
                    "-z".as_ref(),
                    "--".as_ref(),
                    path.as_os_str(),
                ],
                65536,
            )?
            .bytes;
        if !raw.starts_with(b"160000 ") || raw.iter().filter(|v| **v == 0).count() != 1 {
            blocked =
                Some("The selected gitlink has conflicts or changed registration metadata.".into());
        }
        if changes
            .files
            .iter()
            .any(|f| f.path == NativePath::new(Path::new(&request.path)) && f.staged)
        {
            blocked = Some(
                "Commit or unstage this submodule pointer before removing its registration.".into(),
            );
        }
    } else {
        source = git.validate_submodule_source(request.source.trim())?;
        if names
            .keys()
            .any(|other| path.starts_with(other) || other.starts_with(&path))
        {
            blocked = Some("This path overlaps an existing submodule registration.".into());
        }
        let tracked = git
            .read(
                root,
                &[
                    "ls-files".as_ref(),
                    "-z".as_ref(),
                    "--".as_ref(),
                    path.as_os_str(),
                ],
                1024 * 1024,
            )?
            .bytes;
        if !tracked.is_empty() {
            blocked = Some("The destination already contains tracked parent files.".into());
        }
        let parent = child
            .parent()
            .ok_or_else(|| err("Choose a submodule folder inside this repository."))?;
        if !parent.is_dir() {
            blocked = Some("The destination parent folder must already exist.".into());
        }
        download = match fs::symlink_metadata(&child) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
            Err(e) => return Err(e.into()),
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => false,
            _ => {
                blocked = Some(
                    "The destination must be a new folder or an existing independent Git checkout."
                        .into(),
                );
                false
            }
        };
        let admin = d.common_dir.join("modules").join(&path);
        if fs::symlink_metadata(admin).is_ok() {
            blocked=Some("Retained submodule metadata exists at this destination. Inspect or choose another folder; history will not be overwritten.".into());
        }
    }
    if !download && child.is_dir() {
        let child_d = git.discover(&child)?;
        if child_d.root.as_ref() != Some(&fs::canonicalize(&child)?)
            || child_d.common_dir == d.common_dir
        {
            return Err(err("The destination is not an independent child checkout."));
        }
        let child_status = git.status(&child)?;
        child_head = child_status.head;
        child_snapshot = Some(git.head_index_snapshot(&child)?);
        if child_head.is_none() {
            blocked = Some("The child checkout needs an existing commit.".into());
        }
        if request.kind == "add" && child_status.dirty {
            blocked =
                Some("Commit or preserve child changes before adding its registration.".into());
        }
    } else if !download && request.kind == "add" {
        blocked = Some("The destination is not an independent child checkout.".into());
    }
    if !download && request.kind == "add" && child.join(".git").is_file() {
        blocked = Some(
            "Adding a linked or already-absorbed checkout needs manual Git inspection.".into(),
        );
    }
    // Raw parent/child snapshots and layout protect drafts and other registrations.
    let names_sorted = names.iter().collect::<std::collections::BTreeMap<_, _>>();
    let mut request = request.clone();
    request.source = source;
    let destination_parent = child.parent().map(|p| p.canonicalize()).transpose()?;
    let snapshot = serde_json::to_vec(&(
        &request,
        &destination_parent,
        &parent_state,
        &config,
        &modules,
        &names_sorted,
        &child_head,
        &child_snapshot,
        download,
        &blocked,
        NativePath::new(&d.common_dir),
    ))
    .map_err(|e| err(&e.to_string()))?;
    let token = format!("{:x}", Sha256::digest(snapshot));
    let recovery_dir = NativePath::new(
        &d.git_dir
            .join(format!("gitnova-submodule-registration-{}", request.id)),
    );
    if recovery_dir.to_path()?.exists() {
        blocked=Some("This registration change has a recovery record. Inspect its previous result before retrying.".into());
    }
    Ok(SubmoduleLifecyclePreview {
        manifest_before: None,
        manifest_after: None,
        request,
        branch: status.branch,
        child_head,
        download,
        recovery_dir,
        token,
        blocked_reason: blocked,
    })
}
pub fn apply(
    git: &GitCli,
    root: &Path,
    request: &SubmoduleLifecycleRequest,
    token: &str,
) -> Result<()> {
    if ["stage", "unstage"].contains(&request.kind.as_str()) {
        return git.change_registration_index(root, request, token);
    }
    let p = preview(git, root, request)?;
    if p.token != token {
        return Err(err(
            "Registration, files or configuration changed. Review again.",
        ));
    }
    if let Some(reason) = p.blocked_reason {
        return Err(err(&reason));
    }
    let d = git.discover(root)?;
    let child = checked_child(root, &std::path::PathBuf::from(&request.path))?;
    let _child_guard = if child.is_dir() {
        Some(git.acquire_operation(&child)?)
    } else {
        None
    };
    if preview(git, root, request)?.token != token {
        return Err(err(
            "Registration changed while acquiring guards. Review again.",
        ));
    }
    let head = git.status(root)?.head;
    let unrelated = unrelated_index(git, root, &NativePath::new(Path::new(&request.path)))?;
    let index = bytes(&d.git_dir.join("index"), 64 * 1024 * 1024)?;
    let modules = bytes(&root.join(".gitmodules"), 1024 * 1024)?;
    let recovery = p.recovery_dir.to_path()?;
    fs::create_dir(&recovery)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&recovery, fs::Permissions::from_mode(0o700))?;
    }
    let config = bytes(&d.common_dir.join("config"), 1024 * 1024)?;
    let worktree_config = bytes(&d.git_dir.join("config.worktree"), 1024 * 1024)?;
    private_file(
        &recovery.join("original-state.json"),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "index_present": index.is_some(),
            "gitmodules_present": modules.is_some(),
            "config_present": config.is_some(),
            "worktree_config_present": worktree_config.is_some()
        }))
        .map_err(|e| err(&e.to_string()))?,
    )?;
    if let Some(value) = worktree_config {
        private_file(&recovery.join("config.worktree"), &value)?;
    }
    if let Some(value) = index {
        private_file(&recovery.join("index"), &value)?;
    }
    if let Some(value) = modules {
        private_file(&recovery.join("gitmodules"), &value)?;
    }
    if let Some(value) = config {
        private_file(&recovery.join("config"), &value)?;
    }
    private_file(
        &recovery.join("review.json"),
        &serde_json::to_vec_pretty(&p).map_err(|e| err(&e.to_string()))?,
    )?;
    // Every subsequent failure is partial/inspection-required, with the original index/config retained.
    let result = (|| {
        // The one-shot recovery directory is excluded from repository snapshots.
        // Its expected presence does not authorize changed files/configuration.
        if preview(git, root, request)?.token != token {
            return Err(err(
                "Registration changed while saving recovery. No registration command was run.",
            ));
        }
        if request.kind == "add" {
            git.add_submodule_registration(
                root,
                &request.source,
                &std::path::PathBuf::from(&request.path),
            )?;
        } else {
            let names = module_names(git, root)?;
            let name = names
                .get(&std::path::PathBuf::from(&request.path))
                .ok_or_else(|| err("The selected registration disappeared."))?;
            git.remove_submodule_registration(
                root,
                name,
                &std::path::PathBuf::from(&request.path),
            )?;
        }
        if unrelated_index(git, root, &NativePath::new(Path::new(&request.path)))? != unrelated {
            return Err(err("Unrelated staged entries changed during registration."));
        }
        if git.status(root)?.head != head {
            return Err(err("The parent HEAD changed during registration."));
        }
        let names = module_names(git, root)?;
        if (request.kind == "add") != names.contains_key(&std::path::PathBuf::from(&request.path)) {
            return Err(err("Registration postcondition failed."));
        }
        if let Some(expected) = p.child_head {
            if git.status(&child)?.head.as_ref() != Some(&expected) {
                return Err(err("The child commit changed during registration."));
            }
        }
        Ok(())
    })();
    result.map_err(|e:AppError|AppError::new("operation_unknown",format!("{} Registration may be partial; inspect the selected child and saved recovery directory {} before retrying. No rollback was attempted.",e.message,recovery.display())))
}
