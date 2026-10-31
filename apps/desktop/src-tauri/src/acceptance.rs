//! Installed-binary diagnostics may read only a disposable fixture catalog.
use gitnova_ipc::{AppError, Catalog};
use std::path::{Path, PathBuf};

pub fn validate_fixture_catalog(path: &Path, temporary: &Path) -> Result<PathBuf, AppError> {
    let reject = || {
        AppError::new(
            "smoke",
            "Acceptance requires an isolated temporary fixture catalog",
        )
    };
    let canonical = path.canonicalize()?;
    let temporary = temporary.canonicalize()?;
    let root = canonical.parent().ok_or_else(reject)?;
    if !path.is_absolute()
        || !root.starts_with(&temporary)
        || canonical.file_name() != Some(std::ffi::OsStr::new("state.json"))
        || !root
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("gitnova-native-smoke-"))
        || std::fs::metadata(&canonical)?.len() > 4 * 1024 * 1024
    {
        return Err(reject());
    }
    let catalog: Catalog =
        serde_json::from_slice(&std::fs::read(&canonical)?).map_err(|_| reject())?;
    if catalog.version != 1 || catalog.repositories.is_empty() {
        return Err(reject());
    }
    for native in catalog
        .repositories
        .iter()
        .map(|r| &r.common_dir)
        .chain(catalog.worktrees.iter().flat_map(|w| [&w.root, &w.git_dir]))
    {
        let resolved = native.to_path()?.canonicalize()?;
        if !resolved.starts_with(root) || resolved == root {
            return Err(reject());
        }
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitnova_ipc::NativePath;
    use serde_json::json;

    #[test]
    fn catalog_never_routes_acceptance_outside_its_fixture() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("gitnova-native-smoke-test");
        let repo = root.join("repository");
        std::fs::create_dir_all(&repo).unwrap();
        let path = root.join("state.json");
        let mut value = json!({"version":1,"repositories":[{"id":"repo","commonDir":NativePath::new(&repo),"objectFormat":"sha1","bare":true}],"worktrees":[],"workspaces":[],"activeWorkspaceId":"fixture"});
        let write = |value: &serde_json::Value| {
            std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap()
        };
        write(&value);
        assert_eq!(
            validate_fixture_catalog(&path, parent.path()).unwrap(),
            path.canonicalize().unwrap()
        );
        value["repositories"][0]["commonDir"] =
            serde_json::to_value(NativePath::new(parent.path())).unwrap();
        write(&value);
        assert!(validate_fixture_catalog(&path, parent.path()).is_err());
        value["repositories"][0]["commonDir"] =
            serde_json::to_value(NativePath::new(&repo)).unwrap();
        value["worktrees"] = json!([{"id":"checkout","repositoryId":"repo","root":NativePath::new(parent.path()),"gitDir":NativePath::new(&repo)}]);
        write(&value);
        assert!(validate_fixture_catalog(&path, parent.path()).is_err());
        assert!(validate_fixture_catalog(&path, &repo).is_err());
    }
}
