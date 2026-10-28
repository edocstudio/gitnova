//! Only unstaged, clean child checkout differences may be carried through an
//! unrelated cherry-pick/revert. Never stage, recurse, back up or restore a child.
use super::*;
fn evidence(
    git: &GitCli,
    root: &Path,
    path: &NativePath,
    recorded: &str,
) -> Result<PreservedSubmodule> {
    let child = safe_path(root, path)?;
    let discovery = git.discover(&child)?;
    if discovery.root.as_ref() != Some(&fs::canonicalize(&child)?) {
        return Err(AppError::new(
            "merge_blocked",
            "Inspect this submodule checkout before starting the operation.",
        ));
    }
    let status = git.status(&child)?;
    if status.dirty || status.head.is_none() {
        return Err(AppError::new("merge_blocked","This submodule has uncommitted work. Open it and preserve that work before starting the operation."));
    }
    let state = digest(
        &serde_json::to_vec(&(
            &status,
            git.head_index_snapshot(&child)?,
            NativePath::new(&discovery.git_dir),
            NativePath::new(&discovery.common_dir),
        ))
        .map_err(|e| AppError::new("snapshot", e.to_string()))?,
    );
    Ok(PreservedSubmodule {
        path: path.clone(),
        recorded_oid: recorded.into(),
        checkout_oid: status.head.unwrap(),
        state,
    })
}
pub(super) fn discover(git: &GitCli, root: &Path) -> Result<Option<Vec<PreservedSubmodule>>> {
    let raw = git
        .read(
            root,
            &[
                "status".as_ref(),
                "--porcelain=v2".as_ref(),
                "-z".as_ref(),
                "--untracked-files=all".as_ref(),
                "--no-renames".as_ref(),
                "--ignore-submodules=none".as_ref(),
            ],
            4 * 1024 * 1024,
        )?
        .bytes;
    let mut values = Vec::new();
    for row in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let f = row.splitn(9, |b| *b == b' ').collect::<Vec<_>>();
        if f.len() != 9
            || f[0] != b"1"
            || f[1] != b".M"
            || f[2] != b"SC.."
            || f[3] != b"160000"
            || f[4] != b"160000"
            || f[5] != b"160000"
            || f[6] != f[7]
        {
            return Ok(None);
        }
        let recorded = std::str::from_utf8(f[7])
            .map_err(|_| AppError::new("git_parse", "Invalid gitlink revision."))?;
        crate::validate_oid(recorded)?;
        let path = NativePath::new(&crate::path_from_bytes(f[8].to_vec())?);
        values.push(evidence(git, root, &path, recorded)?);
        if values.len() > 128 {
            return Err(AppError::new(
                "safety_budget",
                "Submodule preservation exceeds 128 checkouts.",
            ));
        }
    }
    Ok((!values.is_empty()).then_some(values))
}
pub(super) fn verify(git: &GitCli, root: &Path, marker: &Marker) -> Result<()> {
    for saved in &marker.preserved_submodules {
        let path = saved.path.to_path()?;
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
        let expected = format!("160000 {} 0\t", saved.recorded_oid);
        let mut entry = expected.into_bytes();
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            entry.extend_from_slice(path.as_os_str().as_bytes());
        }
        #[cfg(windows)]
        {
            entry.extend_from_slice(
                path.to_str()
                    .ok_or_else(|| AppError::new("encoding", "Invalid submodule path."))?
                    .as_bytes(),
            );
        }
        entry.push(0);
        if raw != entry || evidence(git, root, &saved.path, &saved.recorded_oid)? != *saved {
            return Err(AppError::new("stale_preview","A preserved submodule changed. Restore its reviewed checkout or inspect the operation before continuing."));
        }
    }
    Ok(())
}
