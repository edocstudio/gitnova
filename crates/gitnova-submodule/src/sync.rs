//! Selected-only URL synchronization. No fetch, update command, hook or checkout.
use crate::checked_child;
use gitnova_domain::*;
use gitnova_git::{GitCli, RepositoryReader, StatusReader};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
fn err(message: &str) -> AppError {
    AppError::new("submodule_sync", message)
}
fn bytes(path: &Path) -> Result<Vec<u8>> {
    let mut v = Vec::new();
    match fs::File::open(path) {
        Ok(f) => {
            f.take(1024 * 1024 + 1).read_to_end(&mut v)?;
            if v.len() > 1024 * 1024 {
                return Err(err("Submodule configuration exceeds its review budget."));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    };
    Ok(v)
}
fn config(git: &GitCli, root: &Path) -> Result<Vec<u8>> {
    Ok(git
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
        .bytes)
}
fn parent(root: &Path, sub: &Submodule) -> Result<PathBuf> {
    if sub.parent_path.hex.is_empty() {
        Ok(root.to_path_buf())
    } else {
        checked_child(root, &sub.parent_path.to_path()?)
    }
}
fn display(value: &str) -> String {
    if let Some((scheme, tail)) = value.split_once("://") {
        if let Some((_, rest)) = tail.split_once('@') {
            return format!("{scheme}://[credentials]@{rest}");
        }
    }
    value.to_string()
}
fn child_remote(git: &GitCli, child: &Path) -> Result<String> {
    if let Some(branch) = git.status(child)?.branch {
        if let Some(remote) = git.optional_text(
            child,
            &["config", "--get", &format!("branch.{branch}.remote")],
        )? {
            if remote == "." {
                return Err(err("This submodule tracks a local remote. Inspect its remote configuration before synchronizing URLs."));
            }
            return Ok(remote);
        }
    }
    Ok("origin".into())
}
// Resolve only ordinary absolute sources. Native sync remains the writer; its
// postconditions must compare raw values, never redacted presentation strings.
fn resolved_url(git: &GitCli, parent: &Path, configured: &str) -> Result<String> {
    if !configured.starts_with("../") && !configured.starts_with("./") {
        git.validate_submodule_source(configured)?;
        return Ok(configured.into());
    }
    if configured.contains(['\\', '?', '#']) || configured.chars().any(char::is_control) {
        return Err(err(
            "Unsupported relative submodule URL. Inspect the source with Git.",
        ));
    }
    let remote = child_remote(git, parent)?;
    let base = git
        .optional_text(
            parent,
            &["config", "--get", &format!("remote.{remote}.url")],
        )?
        .unwrap_or_else(|| parent.to_string_lossy().into_owned());
    git.validate_submodule_source(&base)?;
    let (prefix, path) = if let Some((scheme, rest)) = base.split_once("://") {
        let (authority, path) = rest
            .split_once('/')
            .ok_or_else(|| err("The parent remote needs an absolute repository path."))?;
        (format!("{scheme}://{authority}/"), path.to_string())
    } else if Path::new(&base).is_absolute() {
        // Git resolves URL components lexically; canonicalizing here would
        // change a symlink/alias spelling and falsely report a partial sync.
        let mut destination = PathBuf::from(&base);
        for part in configured.split('/') {
            match part {
                "." => (),
                ".." => {
                    if !destination.pop() {
                        return Err(err("Relative submodule URL escapes its local root."));
                    }
                }
                "" => return Err(err("Empty relative URL components need manual inspection.")),
                value => destination.push(value),
            }
        }
        let value = destination
            .to_str()
            .ok_or_else(|| err("The source path is not UTF-8."))?;
        git.validate_submodule_source(value)?;
        return Ok(value.into());
    } else if let Some((host, path)) = base.split_once(':') {
        if host.contains('/') || path.starts_with('/') {
            return Err(err("This relative SSH URL needs manual inspection."));
        }
        (format!("{host}:"), path.to_string())
    } else {
        return Err(err(
            "Relative submodule URLs require an absolute parent remote.",
        ));
    };
    let mut parts = path
        .trim_end_matches('/')
        .split('/')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if parts
        .iter()
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(err("The parent remote path needs manual normalization."));
    }
    for part in configured.split('/') {
        match part {
            "." => (),
            ".." => {
                if parts.pop().is_none() {
                    return Err(err("Relative submodule URL escapes its remote root."));
                }
            }
            "" => return Err(err("Empty relative URL components need manual inspection.")),
            value => parts.push(value.into()),
        }
    }
    let value = format!("{prefix}{}", parts.join("/"));
    git.validate_submodule_source(&value)?;
    Ok(value)
}
pub fn preview(git: &GitCli, root: &Path, sub: &Submodule) -> Result<SubmoduleSyncPreview> {
    let parent = parent(root, sub)?;
    let child = checked_child(&parent, &sub.relative_path.to_path()?)?;
    let discovery = git.discover(&parent)?;
    let modules = parent.join(".gitmodules");
    if !fs::symlink_metadata(&modules)?.is_file() {
        return Err(err(".gitmodules must be a regular file."));
    }
    let name = sub
        .config_name
        .as_ref()
        .ok_or_else(|| err("No unambiguous .gitmodules entry identifies this submodule."))?;
    if crate::module_names(git, &parent)?.get(&sub.relative_path.to_path()?) != Some(name) {
        return Err(err("Submodule layout changed. Review again."));
    }
    let key = format!("submodule.{name}.url");
    let configured = git
        .optional_text(
            &parent,
            &[
                "config",
                "--no-includes",
                "--file",
                ".gitmodules",
                "--get",
                &key,
            ],
        )?
        .ok_or_else(|| err("This submodule has no URL in .gitmodules."))?;
    let mut blocked = sub.inspection_error.clone();
    if !sub.initialized {
        blocked = Some("Initialize this submodule before synchronizing its URLs.".into());
    }
    let url = resolved_url(git, &parent, &configured)?;
    let previous = git.optional_text(&parent, &["config", "--get", &key])?;
    let mut child_previous = None;
    let mut remote = None;
    let mut child_state = None;
    let mut child_config = vec![];
    if sub.initialized {
        let d = git.discover(&child)?;
        if d.root.as_ref() != Some(&fs::canonicalize(&child)?) {
            return Err(err("The submodule checkout identity changed."));
        }
        let name = child_remote(git, &child)?;
        child_previous =
            git.optional_text(&child, &["config", "--get", &format!("remote.{name}.url")])?;
        if child_previous.is_none() {
            blocked =
                Some("The initialized submodule has no unambiguous default remote URL.".into());
        }
        child_state = Some(git.head_index_snapshot(&child)?);
        child_config = config(git, &child)?;
        remote = Some(name);
    }
    let root_state = git.head_index_snapshot(root)?;
    let parent_state = git.head_index_snapshot(&parent)?;
    if previous.as_deref() == Some(url.as_str())
        && (!sub.initialized || child_previous.as_deref() == Some(url.as_str()))
    {
        blocked = Some("Submodule URLs already match .gitmodules.".into());
    }
    let snapshot = serde_json::to_vec(&(
        sub,
        &url,
        &root_state,
        &parent_state,
        &child_state,
        bytes(&modules)?,
        config(git, &parent)?,
        &child_config,
        NativePath::new(&discovery.common_dir),
    ))
    .map_err(|e| err(&e.to_string()))?;
    let token = format!("{:x}", Sha256::digest(snapshot));
    let recovery_dir = NativePath::new(
        &discovery
            .git_dir
            .join(format!("gitnova-submodule-sync-{token}")),
    );
    Ok(SubmoduleSyncPreview {
        submodule: sub.clone(),
        previous_url: previous.as_deref().map(display),
        child_previous_url: child_previous.as_deref().map(display),
        child_remote: remote,
        url: display(&url),
        recovery_dir,
        token,
        blocked_reason: blocked,
    })
}
fn private_file(path: &Path, value: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(path)?;
    f.write_all(value)?;
    f.sync_all()?;
    Ok(())
}
pub fn apply(git: &GitCli, root: &Path, sub: &Submodule, token: &str) -> Result<()> {
    let p = preview(git, root, sub)?;
    if p.token != token {
        return Err(err(
            "Submodule URLs or checkout changed. Review synchronization again.",
        ));
    }
    if let Some(reason) = &p.blocked_reason {
        return Err(err(reason));
    }
    let parent = parent(root, sub)?;
    let child = checked_child(&parent, &sub.relative_path.to_path()?)?;
    let root_d = git.discover(root)?;
    let parent_d = git.discover(&parent)?;
    let mut guarded = std::collections::HashSet::from([root_d.common_dir]);
    let mut guards = vec![];
    if guarded.insert(parent_d.common_dir.clone()) {
        guards.push(git.acquire_operation(&parent)?);
    }
    if sub.initialized {
        let d = git.discover(&child)?;
        if guarded.insert(d.common_dir) {
            guards.push(git.acquire_operation(&child)?);
        }
    }
    if preview(git, root, sub)?.token != token {
        return Err(err(
            "Submodule configuration changed while acquiring locks. Review again.",
        ));
    }
    let parent_state = git.head_index_snapshot(&parent)?;
    let root_state = git.head_index_snapshot(root)?;
    let child_state = if sub.initialized {
        Some(git.head_index_snapshot(&child)?)
    } else {
        None
    };
    let modules = bytes(&parent.join(".gitmodules"))?;
    let location = p.recovery_dir.to_path()?;
    fs::create_dir(&location)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&location, fs::Permissions::from_mode(0o700))?;
    }
    private_file(
        &location.join("parent-config"),
        &bytes(&parent_d.common_dir.join("config"))?,
    )?;
    private_file(
        &location.join("parent-worktree-config"),
        &bytes(&parent_d.git_dir.join("config.worktree"))?,
    )?;
    private_file(&location.join("gitmodules"), &modules)?;
    if sub.initialized {
        let d = git.discover(&child)?;
        private_file(
            &location.join("child-config"),
            &bytes(&d.common_dir.join("config"))?,
        )?;
        private_file(
            &location.join("child-worktree-config"),
            &bytes(&d.git_dir.join("config.worktree"))?,
        )?;
    }
    if preview(git, root, sub)?.token != token {
        return Err(err(
            "Configuration changed while saving recovery. No URL was synchronized.",
        ));
    }
    let name = sub
        .config_name
        .as_deref()
        .ok_or_else(|| err("Missing submodule name."))?;
    let configured = git
        .optional_text(
            &parent,
            &[
                "config",
                "--no-includes",
                "--file",
                ".gitmodules",
                "--get",
                &format!("submodule.{name}.url"),
            ],
        )?
        .ok_or_else(|| err("The recorded submodule URL disappeared."))?;
    let expected_url = resolved_url(git, &parent, &configured)?;
    git.synchronize_submodule_urls(&parent, &sub.relative_path.to_path()?)?;
    let now = git.optional_text(
        &parent,
        &["config", "--get", &format!("submodule.{name}.url")],
    )?;
    if now.as_deref() != Some(expected_url.as_str())
        || git.head_index_snapshot(&parent)? != parent_state
        || git.head_index_snapshot(root)? != root_state
        || bytes(&parent.join(".gitmodules"))? != modules
    {
        return Err(err("URL synchronization may be partial. Inspect current configuration and the saved recovery folder before retrying."));
    }
    if let Some(remote) = p.child_remote {
        if git.head_index_snapshot(&child)? != child_state.unwrap_or_default()
            || git
                .optional_text(
                    &child,
                    &["config", "--get", &format!("remote.{remote}.url")],
                )?
                .as_deref()
                != Some(expected_url.as_str())
        {
            return Err(err("The child URL update needs inspection. Commits and configuration backups were retained."));
        }
    }
    Ok(())
}
