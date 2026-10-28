use gitnova_domain::*;
use std::{io::Write, path::PathBuf};
pub struct Store {
    path: PathBuf,
}
impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn load_or_initialize(&self, initialize: impl FnOnce() -> Catalog) -> Result<Catalog> {
        if let Some(catalog) = self.load()? {
            return Ok(catalog);
        }
        let catalog = initialize();
        let parent = self
            .path
            .parent()
            .ok_or_else(|| AppError::new("storage", "Invalid data directory"))?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut file, &catalog)
            .map_err(|e| AppError::new("storage", e.to_string()))?;
        file.flush()?;
        file.as_file().sync_all()?;
        match file.persist_noclobber(&self.path) {
            Ok(_) => Ok(catalog),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                self.load()?.ok_or_else(|| {
                    AppError::new(
                        "storage",
                        "Workspace initialization changed. Open the app again.",
                    )
                })
            }
            Err(error) => Err(AppError::new("storage", error.to_string())),
        }
    }
    pub fn load(&self) -> Result<Option<Catalog>> {
        match std::fs::read(&self.path) {
            Ok(data) => {
                let catalog: Catalog = serde_json::from_slice(&data).map_err(|e| {
                    AppError::new("storage", format!("Cannot read workspace file: {e}"))
                })?;
                if catalog.version != 1 {
                    return Err(AppError::new(
                        "storage",
                        "Unsupported workspace file version",
                    ));
                }
                Ok(Some(catalog))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self, catalog: &Catalog) -> Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| AppError::new("storage", "Invalid data directory"))?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut file, catalog)
            .map_err(|e| AppError::new("storage", e.to_string()))?;
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist(&self.path)
            .map_err(|e| AppError::new("storage", e.to_string()))?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn empty(name: &str) -> Catalog {
        Catalog {
            version: 1,
            active_workspace_id: name.into(),
            repositories: vec![],
            worktrees: vec![],
            workspaces: vec![Workspace {
                id: name.into(),
                name: name.into(),
                memberships: vec![],
                selected_id: None,
            }],
        }
    }
    #[test]
    fn concurrent_initialization_retains_one_workspace_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let workers: Vec<_> = ["first", "second"]
            .into_iter()
            .map(|name| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    Store::new(path)
                        .load_or_initialize(|| empty(name))
                        .unwrap()
                        .active_workspace_id
                })
            })
            .collect();
        let ids: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(ids[0], ids[1]);
        let store = Store::new(path);
        assert_eq!(
            store
                .load_or_initialize(|| panic!("must not initialize again"))
                .unwrap()
                .active_workspace_id,
            ids[0]
        );
    }
    #[test]
    fn corrupt_data_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, b"broken").unwrap();
        assert!(Store::new(path.clone()).load().is_err());
        assert!(Store::new(path.clone())
            .load_or_initialize(|| empty("replacement"))
            .is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"broken");
    }
}
