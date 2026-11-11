use crate::{register_identity, Application};
use gitnova_domain::{AppError, Catalog, NativePath, Result};
use gitnova_git::RepositoryReader;
use gitnova_workspace::{active, id, membership_path};

impl Application {
    /// Changes catalog metadata only. A different checkout gets a fresh membership
    /// identity so graph selections and unsaved drafts cannot follow it by mistake.
    pub fn update_membership(
        &self,
        membership_id: &str,
        name: &str,
        path: &str,
    ) -> Result<Catalog> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err(AppError::new(
                "membership",
                "Repository display name must contain 1–80 characters.",
            ));
        }
        let catalog = self.catalog()?;
        let old_path = membership_path(&catalog, membership_id)?;
        // Renaming an unavailable entry must work without accessing its old folder.
        let discovery = if path == NativePath::new(&old_path).display {
            None
        } else {
            let requested = std::fs::canonicalize(path).map_err(|_| {
                AppError::new("membership", "Choose an existing repository root folder.")
            })?;
            if requested == old_path {
                None
            } else {
                let discovered = self.git.discover(&requested)?;
                if &requested != discovered.root.as_ref().unwrap_or(&discovered.common_dir) {
                    return Err(AppError::new(
                        "membership",
                        "Choose the repository root, not a folder inside it.",
                    ));
                }
                Some(discovered)
            }
        };
        self.update(|catalog| {
            let identity = discovery
                .as_ref()
                .map(|discovery| register_identity(catalog, discovery));
            let workspace = active(catalog)?;
            let position = workspace
                .memberships
                .iter()
                .position(|entry| entry.id == membership_id)
                .ok_or_else(|| {
                    AppError::new("membership", "Repository is not in this workspace")
                })?;
            if let Some((repository_id, worktree_id)) = identity {
                if workspace.memberships.iter().any(|entry| {
                    entry.id != membership_id
                        && entry.repository_id == repository_id
                        && entry.worktree_id == worktree_id
                }) {
                    return Err(AppError::new(
                        "membership",
                        "This checkout is already in this workspace.",
                    ));
                }
                let entry = &mut workspace.memberships[position];
                // A linked checkout may have moved while keeping the same gitdir.
                // Use a new membership even then; never reuse its old UI context.
                entry.id = id();
                entry.repository_id = repository_id;
                entry.worktree_id = worktree_id;
                if workspace.selected_id.as_deref() == Some(membership_id) {
                    workspace.selected_id = Some(entry.id.clone());
                }
            }
            workspace.memberships[position].name = name.into();
            Ok(())
        })?;
        self.catalog()
    }

    /// Forget only this workspace membership. Never remove files, refs or other
    /// workspace registrations, including when the checkout is unavailable.
    pub fn remove_membership(&self, membership_id: &str) -> Result<Catalog> {
        self.update(|catalog| {
            let workspace = active(catalog)?;
            let position = workspace
                .memberships
                .iter()
                .position(|entry| entry.id == membership_id)
                .ok_or_else(|| {
                    AppError::new("membership", "Repository is not in this workspace")
                })?;
            workspace.memberships.remove(position);
            if workspace.selected_id.as_deref() == Some(membership_id) {
                workspace.selected_id = workspace.memberships.first().map(|entry| entry.id.clone());
            }
            Ok(())
        })?;
        self.catalog()
    }
}
