use super::*;
use std::collections::BTreeSet;
impl Application {
    pub fn search_commits(&self, request: &CommitSearchRequest) -> Result<CommitSearchPage> {
        let sources = self.search_sources(request)?;
        self.search_index
            .query(&self.git, &self.search_dir, request, &sources)
    }
    fn search_sources(&self, request: &CommitSearchRequest) -> Result<Vec<gitnova_search::Source>> {
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
        if sources.is_empty() || sources.len() > 32 {
            return Err(AppError::new(
                "search_scope",
                "Select between one and 32 repository locations.",
            ));
        }
        Ok(sources)
    }
    pub fn investigate_history(&self, request: &InvestigationRequest) -> Result<InvestigationPage> {
        self.git.validate_investigation(request)?;
        let sources = self.search_sources(&request.selection)?;
        let mut hits: Vec<CommitSearchHit> = vec![];
        let mut evidence = vec![];
        let started = std::time::Instant::now();
        // No persistent content index; each result is bound to a live ref snapshot.
        for source in sources {
            let mut item = InvestigationEvidence {
                location: source.location.clone(),
                source_oid: None,
                target_oid: None,
                source_only: None,
                target_only: None,
                changed_files: None,
                dirty: None,
                scanned: 0,
                limited: false,
                shallow: false,
                issue: None,
            };
            if started.elapsed().as_secs() >= 20 {
                item.limited = true;
                item.issue =
                    Some("Investigation time budget reached; narrow the repository scope.".into());
            } else {
                let read = self.git.discover(&source.path).and_then(|discovery| {
                    if discovery.common_dir != source.common_dir || source.git_dir.as_ref().is_some_and(|dir| *dir != discovery.git_dir) {
                        return Err(AppError::new("stale_search", "Repository identity changed. Reopen this location before investigating."));
                    }
                    self.git.investigate_history(&source.path, request, &mut item)
                });
                match read {
                    Ok(commits) => {
                        for commit in commits {
                            if let Some(hit) = hits.iter_mut().find(|h| {
                                h.commit.oid == commit.oid
                                    && h.locations[0].repository_id == source.location.repository_id
                            }) {
                                hit.locations.push(source.location.clone());
                            } else if hits.len() < 100 {
                                hits.push(CommitSearchHit {
                                    commit,
                                    excerpt: String::new(),
                                    locations: vec![source.location.clone()],
                                });
                            } else {
                                item.limited = true;
                            }
                        }
                    }
                    Err(error) => {
                        item.limited = true;
                        item.issue = Some(error.to_string());
                    }
                }
            }
            evidence.push(item);
        }
        // Membership context may change while a bounded Git reader is running.
        self.search_sources(&request.selection)?;
        let partial = evidence
            .iter()
            .any(|e| e.limited || e.shallow || e.issue.is_some());
        Ok(InvestigationPage {
            hits,
            evidence,
            partial,
        })
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
