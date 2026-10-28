use gitnova_domain::*;
use uuid::Uuid;
pub fn id() -> String {
    Uuid::new_v4().to_string()
}
pub fn empty_catalog() -> Catalog {
    let workspace = Workspace {
        id: id(),
        name: "My workspace".into(),
        memberships: vec![],
        selected_id: None,
    };
    Catalog {
        version: 1,
        active_workspace_id: workspace.id.clone(),
        repositories: vec![],
        worktrees: vec![],
        workspaces: vec![workspace],
    }
}
pub fn active(catalog: &mut Catalog) -> Result<&mut Workspace> {
    catalog
        .workspaces
        .iter_mut()
        .find(|w| w.id == catalog.active_workspace_id)
        .ok_or_else(|| AppError::new("workspace", "Workspace does not exist"))
}
pub fn membership_path(catalog: &Catalog, membership_id: &str) -> Result<std::path::PathBuf> {
    let m = catalog
        .workspaces
        .iter()
        .flat_map(|w| &w.memberships)
        .find(|m| m.id == membership_id)
        .ok_or_else(|| AppError::new("membership", "Repository is not registered"))?;
    if let Some(id) = &m.worktree_id {
        catalog
            .worktrees
            .iter()
            .find(|w| &w.id == id)
            .ok_or_else(|| AppError::new("storage", "Missing worktree"))?
            .root
            .to_path()
    } else {
        catalog
            .repositories
            .iter()
            .find(|r| r.id == m.repository_id)
            .ok_or_else(|| AppError::new("storage", "Missing repository"))?
            .common_dir
            .to_path()
    }
}
