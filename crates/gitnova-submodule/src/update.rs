use crate::checked_child;
use gitnova_domain::*;
use gitnova_git::{
    path_from_bytes, GitCli, RepositoryReader, StatusReader, SubmoduleWriter, WorkingTreeReader,
};
use sha2::{Digest, Sha256};
use std::{ffi::OsStr, path::Path};

// Canonicalize an admin locator even when its final directories have not been created yet.
fn canonical_locator(path: &Path) -> Result<std::path::PathBuf> {
    if !path.is_absolute() {
        return Err(AppError::new(
            "submodule_path",
            "Git returned a non-absolute metadata path.",
        ));
    }
    let mut parent = path.to_path_buf();
    let mut tail = vec![];
    loop {
        match std::fs::canonicalize(&parent) {
            Ok(mut resolved) => {
                for part in tail.iter().rev() {
                    resolved.push(part);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = parent
                    .file_name()
                    .ok_or_else(|| {
                        AppError::new("submodule_path", "Cannot resolve module metadata ancestor.")
                    })?
                    .to_os_string();
                tail.push(name);
                if !parent.pop() {
                    return Err(AppError::new(
                        "submodule_path",
                        "Cannot resolve module metadata ancestor.",
                    ));
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
}
fn config(git: &GitCli, parent: &Path) -> Result<Vec<u8>> {
    Ok(git
        .read(
            parent,
            &[
                OsStr::new("config"),
                OsStr::new("--null"),
                OsStr::new("--list"),
            ],
            1024 * 1024,
        )?
        .bytes)
}
fn configured_url(git: &GitCli, parent: &Path, name: &str) -> Result<Option<String>> {
    let key = format!("submodule.{name}.url");
    if let Some(url) = git.optional_text(parent, &["config", "--get", &key])? {
        return Ok(Some(url));
    }
    git.optional_text(
        parent,
        &[
            "config",
            "--no-includes",
            "--file",
            ".gitmodules",
            "--get",
            &key,
        ],
    )
}
fn display_url(url: &str) -> String {
    let url = url.split(['?', '#']).next().unwrap_or(url);
    if let Some((scheme, rest)) = url.split_once("://") {
        let (authority, path) = rest
            .split_once('/')
            .map_or((rest, ""), |(authority, path)| (authority, path));
        if let Some((_, host)) = authority.rsplit_once('@') {
            return format!("{scheme}://[credentials]@{host}/{path}");
        }
    }
    url.to_owned()
}
fn active_operation(git_dir: &Path) -> bool {
    [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ]
    .iter()
    .any(|marker| git_dir.join(marker).exists())
}
// Match native submodule remote/branch selection, but read only cached refs.
// A preview never fetches or executes a configured submodule update command.
fn remote_target(
    git: &GitCli,
    parent: &Path,
    child: &Path,
    sub: &Submodule,
    parent_branch: Option<&str>,
    child_branch: Option<&str>,
) -> Result<(String, String)> {
    let name = sub.config_name.as_ref().ok_or_else(|| {
        AppError::new(
            "submodule_remote",
            "No unambiguous .gitmodules entry identifies this gitlink.",
        )
    })?;
    let key = format!("submodule.{name}.branch");
    let branch = match git.optional_text(parent, &["config", "--get", &key])? {
        Some(value) => Some(value),
        None => git.optional_text(
            parent,
            &[
                "config",
                "--no-includes",
                "--file",
                ".gitmodules",
                "--get",
                &key,
            ],
        )?,
    };
    let remote = if let Some(branch) = child_branch {
        git.optional_text(
            child,
            &["config", "--get", &format!("branch.{branch}.remote")],
        )?
    } else {
        None
    }
    .unwrap_or_else(|| "origin".into());
    if remote == "."
        || remote.is_empty()
        || remote.starts_with('-')
        || remote.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "submodule_remote",
            "Choose an ordinary configured child remote before updating from its branch.",
        ));
    }
    git.text(child, &["remote", "get-url", "--", &remote])?;
    let prefix = format!("refs/remotes/{remote}/");
    let reference=match branch.as_deref(){
        Some(".")=>format!("{prefix}{}",parent_branch.ok_or_else(||AppError::new("submodule_remote","The submodule inherits its parent branch; check out a parent branch first."))?),
        Some(branch)=>format!("{prefix}{branch}"),
        None=>git.optional_text(child,&["symbolic-ref","--quiet",&format!("{prefix}HEAD")])?.ok_or_else(||AppError::new("submodule_remote","The child remote default branch is not cached. Open and fetch the submodule, then review again."))?,
    };
    if !reference.starts_with(&prefix) || reference == format!("{prefix}HEAD") {
        return Err(AppError::new(
            "submodule_remote",
            "The cached remote branch does not match the configured child remote.",
        ));
    }
    git.text(child, &["check-ref-format", &reference])?;
    // Ref expressions and wildcards are rejected by check-ref-format above.
    let oid = git
        .text(
            child,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{reference}^{{commit}}"),
            ],
        )?
        .trim()
        .to_owned();
    gitnova_git::validate_oid(&oid)?;
    Ok((reference, oid))
}
fn remote_changes(git: &GitCli, child: &Path, from: &str, to: &str) -> Result<Vec<NativePath>> {
    let output = git.read(
        child,
        &[
            "diff".as_ref(),
            "--raw".as_ref(),
            "-z".as_ref(),
            "--no-ext-diff".as_ref(),
            "--no-textconv".as_ref(),
            "--no-renames".as_ref(),
            "--no-color".as_ref(),
            "--no-relative".as_ref(),
            from.as_ref(),
            to.as_ref(),
            "--".as_ref(),
        ],
        256 * 1024,
    )?;
    let parts = output
        .bytes
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();
    if !parts.len().is_multiple_of(2) || parts.len() > 2000 {
        return Err(AppError::new("submodule_remote","Changed paths exceed the complete 1,000-path review budget. Inspect this update in the child repository."));
    }
    let mut paths = Vec::new();
    for pair in parts.as_chunks::<2>().0 {
        let fields = pair[0].split(|b| *b == b' ').collect::<Vec<_>>();
        if fields.len() != 5 || !fields[0].starts_with(b":") {
            return Err(AppError::new(
                "submodule_remote",
                "Invalid changed-path metadata.",
            ));
        }
        if fields[0] == b":160000"
            || fields[1] == b"160000"
            || pair[1].eq_ignore_ascii_case(b".gitmodules")
        {
            return Err(AppError::new("submodule_remote","This version changes nested submodule pointers or layout. Review those repositories in the child before updating here."));
        }
        paths.push(NativePath::new(&path_from_bytes(pair[1].to_vec())?));
    }
    Ok(paths)
}
pub fn preview(
    git: &GitCli,
    root: &Path,
    sub: &Submodule,
    mode: &str,
) -> Result<SubmoduleUpdatePreview> {
    preview_inner(git, root, sub, mode, false)
}
fn preview_inner(
    git: &GitCli,
    root: &Path,
    sub: &Submodule,
    mode: &str,
    held_index: bool,
) -> Result<SubmoduleUpdatePreview> {
    if !matches!(mode, "initialize" | "restore" | "remote") {
        return Err(AppError::new(
            "submodule_mode",
            "Choose initialize, restore recorded version or update from a cached remote branch.",
        ));
    }
    let parent = if sub.parent_path.hex.is_empty() {
        root.to_path_buf()
    } else {
        checked_child(root, &sub.parent_path.to_path()?)?
    };
    let child = checked_child(&parent, &sub.relative_path.to_path()?)?;
    let discovery = git.discover(&parent)?;
    let root_discovery = git.discover(root)?;
    let parent_status = git.status(&parent)?;
    let parent_state = git.working_changes(&parent)?;
    let root_state = git.working_changes(root)?;
    let settings = config(git, &parent)?;
    let mut target_oid = sub.index_oid.clone();
    let mut remote_ref = None;
    let mut changed_paths = None;
    let mut preserved_by = vec![];
    let mut blocked = sub.inspection_error.clone();
    let mut url = None;
    let mut child_state = None;
    let mut child_settings = None;
    let mut child_head_index = None;
    let mut source_base = None;
    if active_operation(&root_discovery.git_dir)
        || (!held_index && root_discovery.git_dir.join("index.lock").exists())
        || root_discovery.git_dir.join("HEAD.lock").exists()
        || active_operation(&discovery.git_dir)
        || (!held_index && discovery.git_dir.join("index.lock").exists())
        || discovery.git_dir.join("HEAD.lock").exists()
    {
        blocked = Some(
            "Finish the containing repository's active Git operation before changing a submodule."
                .into(),
        );
    }
    if parent_state
        .files
        .iter()
        .any(|file| file.path.display == ".gitmodules")
    {
        blocked = Some(
            "Commit or preserve .gitmodules edits before initializing or restoring a submodule."
                .into(),
        );
    }
    if let Some(name) = &sub.config_name {
        url = configured_url(git, &parent, name)?;
        if url
            .as_ref()
            .is_some_and(|url| url.starts_with("./") || url.starts_with("../"))
        {
            let remote = if let Some(branch) = &parent_status.branch {
                git.optional_text(
                    &parent,
                    &["config", "--get", &format!("branch.{branch}.remote")],
                )?
            } else {
                None
            }
            .unwrap_or_else(|| "origin".into());
            source_base = Some(if remote == "." {
                parent.to_string_lossy().into_owned()
            } else {
                match git.text(&parent, &["remote", "get-url", "--", &remote]) {
                    Ok(value) => value.trim_end_matches('\n').to_owned(),
                    Err(error) if error.code == "git_exit_2" || error.code == "git_exit_128" => {
                        parent.to_string_lossy().into_owned()
                    }
                    Err(error) => return Err(error),
                }
            });
        }
    } else {
        blocked = Some("No unambiguous .gitmodules entry identifies this gitlink.".into());
    }
    if mode == "initialize" {
        if sub.initialized {
            blocked = Some(
                "This submodule is already initialized. Review Restore recorded version instead."
                    .into(),
            );
        }
        if url.as_ref().is_none_or(|url| url.trim().is_empty()) {
            blocked = Some("This submodule has no configured source URL.".into());
        }
        if child.exists() && (!child.is_dir() || std::fs::read_dir(&child)?.next().is_some()) {
            blocked = Some("The submodule destination is not empty. Its existing contents will not be overwritten.".into());
        }
        if let Some(name) = &sub.config_name {
            let admin = format!("modules/{name}");
            let mut bytes = git
                .read(
                    &parent,
                    &[
                        OsStr::new("rev-parse"),
                        OsStr::new("--path-format=absolute"),
                        OsStr::new("--git-path"),
                        OsStr::new(&admin),
                    ],
                    65536,
                )?
                .bytes;
            if bytes.last() == Some(&b'\n') {
                bytes.pop();
            }
            let metadata_path = canonical_locator(&path_from_bytes(bytes)?)?;
            let anchor = if metadata_path.starts_with(&discovery.git_dir) {
                &discovery.git_dir
            } else {
                &discovery.common_dir
            };
            let relative = metadata_path.strip_prefix(anchor).map_err(|_| {
                AppError::new(
                    "submodule_path",
                    "Module metadata resolves outside the registered Git directories.",
                )
            })?;
            let metadata_path = checked_child(anchor, relative)?;
            if std::fs::symlink_metadata(metadata_path).is_ok() {
                blocked = Some("Cached module metadata already exists. Inspect or reattach it with Git before initializing here; hidden local commits must be preserved.".into());
            }
        }
    } else if !sub.initialized {
        blocked = Some("Initialize this submodule before restoring its recorded revision.".into());
    } else {
        let child_discovery = git.discover(&child)?;
        let status = git.status(&child)?;
        child_state = Some(git.working_changes(&child)?);
        child_settings = Some(config(git, &child)?);
        child_head_index = Some(git.head_index_snapshot(&child)?);
        if mode == "remote"
            && (child_discovery.common_dir.join("info/grafts").exists()
                || !git
                    .text(
                        &child,
                        &["for-each-ref", "--format=%(refname)", "refs/replace"],
                    )?
                    .is_empty())
        {
            blocked=Some("Replacement or grafted child histories require manual investigation before updating.".into());
        } else if mode == "remote" {
            match remote_target(
                git,
                &parent,
                &child,
                sub,
                parent_status.branch.as_deref(),
                status.branch.as_deref(),
            ) {
                Ok((reference, oid)) => {
                    remote_ref = Some(reference);
                    target_oid = oid;
                }
                Err(error) => {
                    blocked = Some(if error.code == "submodule_remote" {
                        error.message
                    } else {
                        "The configured cached remote target is unavailable. Check its configured branch and fetch in the child, then review again.".into()
                    })
                }
            }
        }
        if status.bare
            || child_discovery.root.as_deref() != Some(std::fs::canonicalize(&child)?.as_path())
        {
            blocked = Some("The submodule does not identify its own available checkout.".into());
        }
        if status.dirty {
            blocked = Some("The submodule contains uncommitted work. Commit or preserve it before restoring; nothing will be discarded or stashed.".into());
        }
        if active_operation(&child_discovery.git_dir)
            || child_discovery.git_dir.join("index.lock").exists()
            || child_discovery.git_dir.join("HEAD.lock").exists()
        {
            blocked = Some(
                "Another Git operation is active in the submodule. Its locks will not be removed."
                    .into(),
            );
        }
        let flags = git
            .read(
                &child,
                &[OsStr::new("ls-files"), OsStr::new("-v"), OsStr::new("-z")],
                4 * 1024 * 1024,
            )?
            .bytes;
        if flags
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .any(|entry| entry[0].is_ascii_lowercase() || entry[0] == b'S')
        {
            blocked = Some("Review hidden assume-unchanged or skip-worktree entries before restoring the submodule.".into());
        }
        if let Some(head) = &status.head {
            preserved_by = git
                .text(
                    &child,
                    &[
                        "for-each-ref",
                        &format!("--contains={head}"),
                        "--format=%(refname)",
                        "refs/heads/",
                    ],
                )?
                .lines()
                .map(str::to_owned)
                .collect();
            if head != &target_oid
                && mode != "remote"
                && status.branch.is_none()
                && preserved_by.is_empty()
            {
                blocked = Some("The detached checkout is not retained by a local branch. Create a branch for its commits before restoring; GitNova will not abandon them.".into());
            }
            if head == &target_oid && !status.dirty && (mode != "remote" || remote_ref.is_some()) {
                blocked = Some(
                    if mode == "remote" {
                        "The checkout already matches the cached remote revision."
                    } else {
                        "The checkout already matches the recorded index revision."
                    }
                    .into(),
                );
            }
        }
        if mode == "remote" && remote_ref.is_some() {
            if let Some(head) = &status.head {
                match remote_changes(git, &child, head, &target_oid) {
                    Ok(paths) => changed_paths = Some(paths),
                    Err(error) => blocked = Some(error.message),
                }
                match git.text(&child,&["merge-base","--is-ancestor",head,&target_oid]) {
                    Ok(_)=>{},
                    Err(error) if error.code=="git_exit_1"=>blocked=Some("The cached remote version is not a descendant of the current checkout. Open the child to compare or merge; local commits will not be abandoned.".into()),
                    Err(error)=>return Err(error),
                }
            } else {
                blocked = Some(
                    "The child has no available current commit. Inspect it before updating.".into(),
                );
            }
        }
        if let Err(error) = git.text(
            &child,
            &["cat-file", "-e", &format!("{}^{{commit}}", target_oid)],
        ) {
            if error.code.starts_with("git_exit_") {
                blocked = Some("The recorded commit is not available locally. Fetch this child repository first, then review Restore again.".into());
            } else {
                return Err(error);
            }
        }
    }
    let snapshot = serde_json::to_vec(&(
        sub,
        mode,
        &parent_status,
        parent_state.token,
        root_state.token,
        settings,
        child_settings,
        child_head_index,
        &url,
        &source_base,
        (
            &preserved_by,
            &remote_ref,
            &target_oid,
            &changed_paths,
            &blocked,
        ),
        child_state.map(|state| state.token),
    ))
    .map_err(|error| AppError::new("snapshot", error.to_string()))?;
    Ok(SubmoduleUpdatePreview {
        target_oid,
        remote_ref,
        changed_paths,
        submodule: sub.clone(),
        mode: mode.into(),
        url: url.as_deref().map(display_url),
        source_base: source_base.as_deref().map(display_url),
        parent_status,
        preserved_by,
        network_required: mode == "initialize",
        blocked_reason: blocked,
        token: format!("{:x}", Sha256::digest(snapshot)),
    })
}
/// The core holds the selected repository's operation guard. Acquire distinct parent/child guards too.
pub fn apply(git: &GitCli, root: &Path, sub: &Submodule, mode: &str, token: &str) -> Result<()> {
    let before = preview(git, root, sub, mode)?;
    if before.token != token {
        return Err(AppError::new(
            "stale_preview",
            "Submodule, containing index, configuration or working state changed. Review again.",
        ));
    }
    if let Some(reason) = before.blocked_reason {
        return Err(AppError::new("submodule_blocked", reason));
    }
    let parent = if sub.parent_path.hex.is_empty() {
        root.to_path_buf()
    } else {
        checked_child(root, &sub.parent_path.to_path()?)?
    };
    let child = checked_child(&parent, &sub.relative_path.to_path()?)?;
    let root_discovery = git.discover(root)?;
    let parent_discovery = git.discover(&parent)?;
    let mut guarded = std::collections::HashSet::from([root_discovery.common_dir]);
    let mut guards = vec![];
    if guarded.insert(parent_discovery.common_dir.clone()) {
        guards.push(git.acquire_operation(&parent)?);
    }
    if sub.initialized {
        let child_discovery = git.discover(&child)?;
        if guarded.insert(child_discovery.common_dir) {
            guards.push(git.acquire_operation(&child)?);
        }
    }
    let _root_index = if root_discovery.git_dir != parent_discovery.git_dir {
        Some(git.acquire_index_guard(root)?)
    } else {
        None
    };
    let _parent_index = git.acquire_index_guard(&parent)?;
    let confirmed = preview_inner(git, root, sub, mode, true)?;
    if confirmed.token != token {
        return Err(AppError::new(
            "stale_preview",
            "Submodule state changed while acquiring locks. Review again.",
        ));
    }
    if let Some(reason) = confirmed.blocked_reason {
        return Err(AppError::new("submodule_blocked", reason));
    }
    let root_snapshot = git.head_index_snapshot(root)?;
    let parent_snapshot = git.head_index_snapshot(&parent)?;
    if mode == "initialize" {
        git.initialize_recorded(&parent, &sub.relative_path.to_path()?)?;
    } else {
        git.restore_recorded(&child, &confirmed.target_oid)?;
    }
    let after = git.status(&child)?;
    let current = git
        .read(
            &parent,
            &[
                OsStr::new("ls-files"),
                OsStr::new("--stage"),
                OsStr::new("-z"),
                OsStr::new("--"),
                sub.relative_path.to_path()?.as_os_str(),
            ],
            65536,
        )?
        .bytes;
    if git.head_index_snapshot(root)? != root_snapshot
        || git.head_index_snapshot(&parent)? != parent_snapshot
        || after.head.as_deref() != Some(confirmed.target_oid.as_str())
        || !current
            .split(|byte| *byte == 0)
            .any(|entry| entry.starts_with(format!("160000 {} 0\t", sub.index_oid).as_bytes()))
    {
        return Err(AppError::new("operation_unknown", "Submodule checkout or containing gitlink differs from the reviewed revision. Inspect both repositories; no rollback was attempted."));
    }
    drop(guards);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_display_redacts_userinfo_and_query_without_confusing_path_at_signs() {
        assert_eq!(
            display_url("https://user:secret@example.test/repo.git?token=private#fragment"),
            "https://[credentials]@example.test/repo.git"
        );
        assert_eq!(
            display_url("https://example.test/a@b.git"),
            "https://example.test/a@b.git"
        );
        assert_eq!(
            display_url("git@example.test:repo.git"),
            "git@example.test:repo.git"
        );
    }
}
