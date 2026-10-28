//! Bounded recursive discovery. Never runs submodule update, hooks or custom commands.
pub mod sync;
pub mod update;
use gitnova_domain::*;
use gitnova_git::{path_from_bytes, GitCli, RepositoryReader, StatusReader};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

pub struct DiscoveredSubmodule {
    pub parent_root: PathBuf,
    pub state: Submodule,
}
pub fn relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.components().any(|part| {
            !matches!(part, Component::Normal(_))
                || part.as_os_str().eq_ignore_ascii_case(OsStr::new(".git"))
        })
    {
        return Err(AppError::new(
            "submodule_path",
            "Submodule path must stay inside its containing repository.",
        ));
    }
    Ok(())
}
pub fn checked_child(root: &Path, path: &Path) -> Result<PathBuf> {
    relative(path)?;
    let mut child = root.to_path_buf();
    for part in path.components() {
        child.push(part);
        if std::fs::symlink_metadata(&child).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(AppError::new(
                "submodule_path",
                "Submodule paths through symlinks require manual Git inspection.",
            ));
        }
    }
    Ok(child)
}
pub fn module_names(git: &GitCli, root: &Path) -> Result<HashMap<PathBuf, String>> {
    let module_file = root.join(".gitmodules");
    match std::fs::symlink_metadata(&module_file) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Ok(meta) if !meta.is_file() => {
            return Err(AppError::new(
                "submodule_config",
                ".gitmodules must be a regular file.",
            ))
        }
        Err(error) => return Err(error.into()),
        _ => {}
    }
    let raw = match git.read(
        root,
        &[
            OsStr::new("config"),
            OsStr::new("--no-includes"),
            OsStr::new("--file"),
            OsStr::new(".gitmodules"),
            OsStr::new("--null"),
            OsStr::new("--get-regexp"),
            OsStr::new(r"^submodule\..*\.path$"),
        ],
        1024 * 1024,
    ) {
        Ok(out) => out.bytes,
        Err(error) if error.code == "git_exit_1" => vec![],
        Err(error) => return Err(error),
    };
    let mut names = HashMap::new();
    for record in raw
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let newline = record
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or_else(|| AppError::new("git_parse", "Missing submodule config path"))?;
        let key = std::str::from_utf8(&record[..newline])
            .map_err(|_| AppError::new("encoding", "Submodule name is not UTF-8"))?;
        let name = key
            .strip_prefix("submodule.")
            .and_then(|name| name.strip_suffix(".path"))
            .ok_or_else(|| AppError::new("git_parse", "Invalid submodule config name"))?;
        relative(Path::new(name))?;
        let path = path_from_bytes(record[newline + 1..].to_vec())?;
        relative(&path)?;
        if names.insert(path, name.into()).is_some() {
            return Err(AppError::new(
                "submodule_config",
                "More than one submodule name identifies the same path.",
            ));
        }
    }
    Ok(names)
}
pub fn inspect(git: &GitCli, root: &Path) -> Result<Vec<DiscoveredSubmodule>> {
    let mut queue = VecDeque::from([(root.to_path_buf(), PathBuf::new(), 0usize)]);
    let mut seen = HashSet::new();
    let mut result = vec![];
    seen.insert(git.discover(root)?.git_dir);
    while let Some((parent, prefix, depth)) = queue.pop_front() {
        let names = module_names(git, &parent)?;
        let bytes = git
            .read(
                &parent,
                &[
                    OsStr::new("ls-files"),
                    OsStr::new("--stage"),
                    OsStr::new("-z"),
                ],
                4 * 1024 * 1024,
            )?
            .bytes;
        let parent_status = git.status(&parent)?;
        let mut head_links = HashMap::new();
        if parent_status.head.is_some() {
            let tree = git
                .read(
                    &parent,
                    &[
                        OsStr::new("ls-tree"),
                        OsStr::new("-r"),
                        OsStr::new("-z"),
                        OsStr::new("HEAD"),
                    ],
                    4 * 1024 * 1024,
                )?
                .bytes;
            for entry in tree
                .split(|byte| *byte == 0)
                .filter(|entry| entry.starts_with(b"160000 "))
            {
                let tab = entry
                    .iter()
                    .position(|byte| *byte == b'\t')
                    .ok_or_else(|| AppError::new("git_parse", "Invalid tree gitlink"))?;
                let oid = String::from_utf8_lossy(&entry[..tab])
                    .split_whitespace()
                    .nth(2)
                    .unwrap_or("")
                    .to_owned();
                head_links.insert(path_from_bytes(entry[tab + 1..].to_vec())?, oid);
            }
        }
        for entry in bytes
            .split(|byte| *byte == 0)
            .filter(|entry| entry.starts_with(b"160000 "))
        {
            if result.len() >= 256 {
                return Err(AppError::new(
                    "safety_budget",
                    "Submodule discovery exceeds 256 relations.",
                ));
            }
            let tab = entry
                .iter()
                .position(|byte| *byte == b'\t')
                .ok_or_else(|| AppError::new("git_parse", "Invalid index gitlink"))?;
            let header = String::from_utf8_lossy(&entry[..tab]);
            let fields: Vec<_> = header.split_whitespace().collect();
            if fields.len() != 3 || fields[2] != "0" {
                continue;
            }
            let relative_path = path_from_bytes(entry[tab + 1..].to_vec())?;
            relative(&relative_path)?;
            let child = checked_child(&parent, &relative_path)?;
            let initialized = std::fs::symlink_metadata(child.join(".git")).is_ok();
            let mut inspection_error = None;
            let status = if initialized {
                match git.status(&child) {
                    Ok(status) => Some(status),
                    Err(error) => {
                        inspection_error = Some(error.message);
                        None
                    }
                }
            } else {
                None
            };
            let root_relative = prefix.join(&relative_path);
            if initialized && inspection_error.is_none() {
                let discovery = git.discover(&child)?;
                if !seen.insert(discovery.git_dir) {
                    inspection_error = Some(
                        "Repeated worktree or submodule cycle; nested discovery stopped.".into(),
                    );
                } else if depth >= 8 {
                    inspection_error =
                        Some("Nested discovery reached its eight-level limit.".into());
                } else {
                    queue.push_back((child, root_relative.clone(), depth + 1));
                }
            }
            result.push(DiscoveredSubmodule {
                parent_root: parent.clone(),
                state: Submodule {
                    relation_id: String::new(),
                    parent_relation_id: None,
                    parent_repository_id: String::new(),
                    parent_worktree_id: None,
                    repository_id: None,
                    worktree_id: None,
                    parent_path: NativePath::new(&prefix),
                    relative_path: NativePath::new(&relative_path),
                    depth,
                    config_name: names.get(&relative_path).cloned(),
                    inspection_error,
                    path: NativePath::new(&root_relative),
                    head_oid: head_links.remove(&relative_path),
                    index_oid: fields[1].into(),
                    checked_out_oid: status.as_ref().and_then(|status| status.head.clone()),
                    initialized,
                    dirty: status.as_ref().map(|status| status.dirty),
                    detached: status.map(|status| status.branch.is_none()),
                },
            });
        }
    }
    Ok(result)
}
