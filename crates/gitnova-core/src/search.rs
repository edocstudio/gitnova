use super::*;
use std::collections::BTreeSet;
impl Application {
    pub fn search_commits(&self, request: &CommitSearchRequest) -> Result<CommitSearchPage> {
        let catalog = self.catalog()?;
        if catalog.active_workspace_id != request.workspace_id {
            return Err(AppError::new(
                "stale_search",
                "The active workspace changed. Search again.",
            ));
        }
        let workspace = catalog
            .workspaces
            .iter()
            .find(|w| w.id == request.workspace_id)
            .ok_or_else(|| AppError::new("workspace", "Unknown workspace."))?;
        if workspace.selected_id.as_deref() != Some(&request.current_membership_id) {
            return Err(AppError::new(
                "stale_search",
                "The current repository changed. Search again.",
            ));
        }
        let ids = match request.scope.as_str() {
            "current" => vec![request.current_membership_id.clone()],
            "selected" => request
                .selected_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            "workspace" => workspace.memberships.iter().map(|m| m.id.clone()).collect(),
            _ => {
                return Err(AppError::new(
                    "search_scope",
                    "Choose Current repository, Selected repositories or Entire workspace.",
                ))
            }
        };
        let mut sources = vec![];
        for id in ids {
            let membership = workspace
                .memberships
                .iter()
                .find(|m| m.id == id)
                .ok_or_else(|| {
                    AppError::new(
                        "search_scope",
                        "A selected repository is not in the active workspace.",
                    )
                })?;
            let repository = catalog
                .repositories
                .iter()
                .find(|r| r.id == membership.repository_id)
                .ok_or_else(|| AppError::new("storage", "Missing repository identity."))?;
            let worktree = membership
                .worktree_id
                .as_ref()
                .map(|id| {
                    catalog
                        .worktrees
                        .iter()
                        .find(|w| &w.id == id)
                        .ok_or_else(|| AppError::new("storage", "Missing worktree identity."))
                })
                .transpose()?;
            let path = membership_path(&catalog, &membership.id)?;
            sources.push(gitnova_search::Source {
                location: SearchLocation {
                    membership_id: membership.id.clone(),
                    repository_id: membership.repository_id.clone(),
                    worktree_id: membership.worktree_id.clone(),
                    name: membership.name.clone(),
                    path: NativePath::new(&path),
                },
                path,
                common_dir: repository.common_dir.to_path()?,
                git_dir: worktree.map(|w| w.git_dir.to_path()).transpose()?,
            });
        }
        self.search_index
            .query(&self.git, &self.search_dir, request, &sources)
    }
    /// Locate read-only graph context without checking out or moving any ref.
    pub fn focus_commit(
        &self,
        workspace_id: &str,
        membership_id: &str,
        oid: &str,
    ) -> Result<GraphFocus> {
        gitnova_git::validate_oid(oid)?;
        let catalog = self.catalog()?;
        if catalog.active_workspace_id != workspace_id
            || !catalog.workspaces.iter().any(|w| {
                w.id == workspace_id && w.memberships.iter().any(|m| m.id == membership_id)
            })
        {
            return Err(AppError::new(
                "stale_search",
                "Search location is no longer in the active workspace.",
            ));
        }
        let path = membership_path(&catalog, membership_id)?;
        let discovery = self.git.discover(&path)?;
        if self
            .git
            .refs(&path)?
            .iter()
            .any(|r| r.name.starts_with("refs/replace/"))
            || discovery.common_dir.join("info/grafts").exists()
        {
            return Err(AppError::new(
                "search_history",
                "Replacement/grafted histories require a separately validated graph context.",
            ));
        }
        let fingerprint = self.git.fingerprint(&path)?;
        let commits = self.git.history_range(&path, 0, 20101)?;
        let index=commits.iter().position(|c|c.oid==oid).ok_or_else(||AppError::new("search_location","This commit is no longer reachable inside the supported graph range. Search again."))?;
        let base = index / 100 * 100;
        let commits = self.git.history_range(&path, 0, base + 101)?;
        if commits.get(index).is_none_or(|commit| commit.oid != oid) {
            return Err(AppError::new(
                "stale_search",
                "Matching graph position changed while opening. Search again.",
            ));
        }
        let more = commits.len() > base + 100;
        let mut cursor = GraphCursor {
            version: 2,
            fingerprint: fingerprint.clone(),
            ..Default::default()
        };
        let status = self.git.status(&path)?;
        if let Some(head) = &status.head {
            gitnova_graph::reserve_head(&mut cursor, head.clone());
        }
        for chunk in commits[..base].chunks(100) {
            gitnova_graph::layout(chunk.to_vec(), &mut cursor);
        }
        let rows = gitnova_graph::layout(
            commits[base..commits.len().min(base + 100)].to_vec(),
            &mut cursor,
        );
        if cursor.lanes.len() > 512 {
            return Err(AppError::new(
                "search_location",
                "Graph complexity exceeds the supported lane budget.",
            ));
        }
        cursor.offset = base + rows.len();
        let refs = self.git.refs(&path)?;
        let tracking = self
            .display_tracking(&path, &fingerprint)
            .unwrap_or_default();
        if self.git.fingerprint(&path)? != fingerprint {
            return Err(AppError::new(
                "stale_search",
                "Repository history changed while opening the result. Search again.",
            ));
        }
        Ok(GraphFocus {
            base,
            selected_index: index - base,
            page: GraphPage {
                tracking,
                rows,
                refs,
                status,
                fingerprint,
                next: more.then_some(cursor),
            },
        })
    }
}
