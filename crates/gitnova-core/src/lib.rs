use gitnova_domain::*;
pub use gitnova_git::with_progress;
use gitnova_git::{
    BranchReader, BranchWriter, CommitWriter, DiffReader, GitCli, HistoryReader, IndexWriter,
    RefReader, RepositoryReader, StatusReader, WorkingTreeReader,
};
use gitnova_git::{RemoteReader, RemoteWriter};
pub use gitnova_hosting::validated_ci_url;
use gitnova_storage::Store;
use gitnova_workspace::{active, id, membership_path};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

mod activity;
mod clone;
mod commit_identities;
mod display_cache;
pub mod git_credentials;
mod hosting;
mod init;
mod jenkins;
mod merge_cleanup;
mod search;

pub struct Application {
    commit_identities: Mutex<Vec<ConnectionCommitIdentity>>,
    tracking_cache: display_cache::TrackingCache,
    pipeline_cache:
        Mutex<std::collections::VecDeque<(String, std::time::Instant, PipelineSnapshot)>>,
    jenkins_cache: Mutex<std::collections::VecDeque<(String, std::time::Instant, JenkinsSnapshot)>>,
    search_index: gitnova_search::SearchIndex,
    search_dir: PathBuf,
    catalog: Mutex<Catalog>,
    store: Store,
    git: GitCli,
    receipt_dir: PathBuf,
}
impl Application {
    pub fn new(data_file: PathBuf) -> Result<Self> {
        let receipt_dir = data_file
            .parent()
            .ok_or_else(|| AppError::new("storage", "Invalid application data path"))?
            .join("operations");
        let search_dir = receipt_dir
            .parent()
            .ok_or_else(|| AppError::new("storage", "Missing app data path."))?
            .join("search");
        let identities =
            commit_identities::load(&data_file.parent().unwrap().join("commit-identities.json"))?;
        let store = Store::new(data_file);
        let catalog = store.load_or_initialize(gitnova_workspace::empty_catalog)?;
        Ok(Self {
            commit_identities: Mutex::new(identities),
            tracking_cache: display_cache::TrackingCache::default(),
            pipeline_cache: Mutex::new(std::collections::VecDeque::new()),
            jenkins_cache: Mutex::new(std::collections::VecDeque::new()),
            search_index: gitnova_search::SearchIndex::default(),
            search_dir,
            catalog: Mutex::new(catalog),
            store,
            git: GitCli,
            receipt_dir,
        })
    }
    pub fn catalog(&self) -> Result<Catalog> {
        Ok(self
            .catalog
            .lock()
            .map_err(|_| AppError::new("state", "Workspace state is unavailable"))?
            .clone())
    }
    fn update<T>(&self, f: impl FnOnce(&mut Catalog) -> Result<T>) -> Result<T> {
        let mut guard = self
            .catalog
            .lock()
            .map_err(|_| AppError::new("state", "Workspace state is unavailable"))?;
        let mut candidate = guard.clone();
        let value = f(&mut candidate)?;
        self.store.save(&candidate)?;
        *guard = candidate;
        Ok(value)
    }
    pub fn path(&self, membership_id: &str) -> Result<PathBuf> {
        membership_path(&self.catalog()?, membership_id)
    }
    pub fn create_workspace(&self, name: &str) -> Result<Catalog> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::new(
                "workspace",
                "Workspace name must contain 1–80 characters",
            ));
        }
        self.update(|c| {
            let workspace = Workspace {
                id: id(),
                name: name.into(),
                memberships: vec![],
                selected_id: None,
            };
            c.active_workspace_id = workspace.id.clone();
            c.workspaces.push(workspace);
            Ok(())
        })?;
        self.catalog()
    }
    pub fn switch_workspace(&self, workspace_id: &str) -> Result<Catalog> {
        self.update(|c| {
            if !c.workspaces.iter().any(|w| w.id == workspace_id) {
                return Err(AppError::new("workspace", "Unknown workspace"));
            }
            c.active_workspace_id = workspace_id.into();
            Ok(())
        })?;
        self.catalog()
    }
    pub fn select(&self, membership_id: &str) -> Result<Catalog> {
        self.update(|c| {
            let w = active(c)?;
            if !w.memberships.iter().any(|m| m.id == membership_id) {
                return Err(AppError::new(
                    "membership",
                    "Repository is not in this workspace",
                ));
            }
            if let Some(position) = w.memberships.iter().position(|m| m.id == membership_id) {
                let membership = w.memberships.remove(position);
                w.memberships.insert(0, membership);
            }
            w.selected_id = Some(membership_id.into());
            Ok(())
        })?;
        self.catalog()
    }
    pub fn pin(&self, membership_id: &str) -> Result<Catalog> {
        self.update(|c| {
            let m = active(c)?
                .memberships
                .iter_mut()
                .find(|m| m.id == membership_id)
                .ok_or_else(|| AppError::new("membership", "Unknown repository"))?;
            m.pinned = !m.pinned;
            Ok(())
        })?;
        self.catalog()
    }
    pub fn add_repository(&self, path: &Path) -> Result<Catalog> {
        let workspace_id = self.catalog()?.active_workspace_id;
        self.register_repository_in(path, &workspace_id)
    }
    fn register_repository_in(&self, path: &Path, workspace_id: &str) -> Result<Catalog> {
        let discovery = self.git.discover(path)?;
        self.update(|c| {
            let (repository_id, worktree_id) = register_identity(c, &discovery);
            let name = discovery
                .root
                .as_ref()
                .unwrap_or(&discovery.common_dir)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let workspace = c
                .workspaces
                .iter_mut()
                .find(|workspace| workspace.id == workspace_id)
                .ok_or_else(|| {
                    AppError::new("workspace", "The selected workspace no longer exists.")
                })?;
            let membership_id = if let Some(m) = workspace
                .memberships
                .iter()
                .find(|m| m.repository_id == repository_id && m.worktree_id == worktree_id)
            {
                m.id.clone()
            } else {
                let membership_id = id();
                workspace.memberships.push(Membership {
                    id: membership_id.clone(),
                    repository_id,
                    worktree_id,
                    name,
                    pinned: false,
                });
                membership_id
            };
            if let Some(position) = workspace
                .memberships
                .iter()
                .position(|m| m.id == membership_id)
            {
                let membership = workspace.memberships.remove(position);
                workspace.memberships.insert(0, membership);
            }
            workspace.selected_id = Some(membership_id);
            Ok(())
        })?;
        self.catalog()
    }
    pub fn status(&self, membership_id: &str) -> Result<RepoStatus> {
        self.git.status(&self.path(membership_id)?)
    }
    pub fn graph(&self, membership_id: &str, cursor: Option<GraphCursor>) -> Result<GraphPage> {
        let path = self.path(membership_id)?;
        let fingerprint = self.git.fingerprint(&path)?;
        let status = self.git.status(&path)?;
        let mut cursor = if let Some(cursor) = cursor {
            cursor
        } else {
            let mut cursor = GraphCursor {
                version: 2,
                fingerprint: fingerprint.clone(),
                ..Default::default()
            };
            if let Some(head) = status.head.clone() {
                gitnova_graph::reserve_head(&mut cursor, head);
            }
            cursor
        };
        if cursor.version != 2 || cursor.fingerprint != fingerprint {
            return Err(AppError::new(
                "stale_graph",
                "Repository refs or HEAD changed. Refresh the graph.",
            ));
        }
        if cursor.lanes.len() > 512 || cursor.offset > 1_000_000 || cursor.next_color > 2_000_000 {
            return Err(AppError::new(
                "pagination",
                "Graph cursor exceeds supported bounds",
            ));
        }
        let mut pending = std::collections::HashSet::new();
        for lane in cursor.lanes.iter().flatten() {
            gitnova_git::validate_oid(&lane.oid)?;
            if lane.color >= cursor.next_color || !pending.insert(&lane.oid) {
                return Err(AppError::new(
                    "pagination",
                    "Invalid graph lane cursor. Refresh history.",
                ));
            }
        }
        let mut commits = self.git.history(&path, cursor.offset, 101)?;
        let more = commits.len() > 100;
        commits.truncate(100);
        let count = commits.len();
        let rows = gitnova_graph::layout(commits, &mut cursor);
        cursor.offset += count;
        let refs = self.git.refs(&path)?;
        let tracking = self
            .display_tracking(&path, &fingerprint)
            .unwrap_or_default();
        if self.git.fingerprint(&path)? != fingerprint {
            return Err(AppError::new(
                "stale_graph",
                "Repository changed while loading. Refresh the graph.",
            ));
        }
        Ok(GraphPage {
            tracking,
            rows,
            refs,
            status,
            next: more.then_some(cursor),
            fingerprint,
        })
    }
    pub fn stashes(&self, membership_id: &str) -> Result<StashList> {
        self.with_repo_identity(membership_id, || {
            self.git.stashes(&self.path(membership_id)?)
        })
    }
    pub fn preview_stash(
        &self,
        membership_id: &str,
        request: &StashRequest,
    ) -> Result<StashPreview> {
        self.with_repo_identity(membership_id, || {
            self.git.preview_stash(&self.path(membership_id)?, request)
        })
    }
    pub fn apply_stash(
        &self,
        membership_id: &str,
        request: &StashRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.with_repo_identity(membership_id, || {
            let path = self.path(membership_id)?;
            let preview = self.git.preview_stash(&path, request)?;
            let kind = match request.kind {
                StashKind::Push => "stash_push",
                StashKind::Apply => "stash_apply",
                StashKind::Pop => "stash_pop",
                StashKind::Drop => "stash_drop",
            };
            self.execute_write_detailed(
                membership_id,
                kind,
                None,
                None,
                Some(PlannedOperation::Stash(preview)),
                |git, path| git.apply_stash(path, request, token),
            )
        })
    }
    pub fn detail(&self, membership_id: &str, oid: &str) -> Result<CommitDetail> {
        self.git.detail(&self.path(membership_id)?, oid)
    }
    pub fn historical_patch(
        &self,
        membership_id: &str,
        oid: &str,
        file: &NativePath,
    ) -> Result<HistoricalPatch> {
        self.git
            .historical_patch(&self.path(membership_id)?, oid, file)
    }
    pub fn submodule_snapshot(&self, membership_id: &str) -> Result<SubmoduleSnapshot> {
        let path = self.path(membership_id)?;
        let before = self.git.status(&path)?;
        if before.bare {
            return Ok(SubmoduleSnapshot {
                status: before,
                submodules: vec![],
            });
        }
        let index = self.git.index_display_fingerprint(&path)?;
        let submodules = self.submodules(membership_id)?;
        let status = self.git.status(&path)?;
        if before.branch != status.branch
            || before.head != status.head
            || index != self.git.index_display_fingerprint(&path)?
        {
            return Err(AppError::new(
                "stale_preview",
                "Branch context changed. Refresh the graph to inspect its submodules.",
            ));
        }
        Ok(SubmoduleSnapshot { status, submodules })
    }
    pub fn submodules(&self, membership_id: &str) -> Result<Vec<Submodule>> {
        let path = self.path(membership_id)?;
        if self.git.status(&path)?.bare {
            return Ok(vec![]);
        }
        let discovered = gitnova_submodule::inspect(&self.git, &path)?;
        let mut relations = std::collections::HashMap::new();
        let mut result = vec![];
        for discovered in discovered {
            let mut sub = discovered.state;
            let (parent_id, parent_worktree_id) =
                self.discovered_identity(&discovered.parent_root)?;
            sub.relation_id = format!(
                "submodule:{parent_id}:{}:{}",
                parent_worktree_id.as_deref().unwrap_or("bare"),
                sub.relative_path.hex
            );
            sub.parent_relation_id = relations.get(&sub.parent_path.hex).cloned();
            sub.parent_repository_id = parent_id;
            sub.parent_worktree_id = parent_worktree_id;
            if sub.initialized && sub.inspection_error.is_none() {
                let (repo_id, worktree_id) =
                    self.discovered_identity(&path.join(sub.path.to_path()?))?;
                sub.repository_id = Some(repo_id);
                sub.worktree_id = worktree_id;
            }
            relations.insert(sub.path.hex.clone(), sub.relation_id.clone());
            result.push(sub);
        }
        Ok(result)
    }
    fn discovered_identity(&self, path: &Path) -> Result<(String, Option<String>)> {
        let discovery = self.git.discover(path)?;
        let catalog = self.catalog()?;
        let existing = catalog
            .repositories
            .iter()
            .find(|repo| repo.common_dir == NativePath::new(&discovery.common_dir));
        let worktree = catalog
            .worktrees
            .iter()
            .find(|tree| tree.git_dir == NativePath::new(&discovery.git_dir));
        if let (Some(repo), Some(tree)) = (existing, worktree) {
            return Ok((repo.id.clone(), Some(tree.id.clone())));
        }
        self.update(|catalog| Ok(register_identity(catalog, &discovery)))
    }
    pub fn preview_submodule_pointer(
        &self,
        membership_id: &str,
        path: &NativePath,
        stage: bool,
    ) -> Result<SubmodulePointerPreview> {
        let sub = self
            .submodules(membership_id)?
            .into_iter()
            .find(|sub| &sub.path == path)
            .ok_or_else(|| {
                AppError::new(
                    "submodule_pointer",
                    "Choose a discovered submodule from this repository.",
                )
            })?;
        self.git
            .preview_submodule_pointer(&self.path(membership_id)?, &sub, stage)
    }
    pub fn change_submodule_pointer(
        &self,
        membership_id: &str,
        path: &NativePath,
        stage: bool,
        token: &str,
    ) -> Result<OperationReceipt> {
        let preview = self.preview_submodule_pointer(membership_id, path, stage)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Submodule pointer review changed. Review again.",
            ));
        }
        let recovery = self
            .receipt_dir
            .join("submodule-pointers")
            .join(format!("{}.index", id()));
        let details = SubmoduleOperationReceipt {
            recovery_index: Some(NativePath::new(&recovery)),
            relation_id: preview.submodule.relation_id.clone(),
            path: path.clone(),
            parent_repository_id: preview.submodule.parent_repository_id.clone(),
            repository_id: preview.submodule.repository_id.clone(),
            head_before: preview.submodule.checked_out_oid.clone(),
            target_oid: preview
                .submodule
                .checked_out_oid
                .clone()
                .unwrap_or_else(|| preview.submodule.index_oid.clone()),
            head_after: None,
        };
        self.execute_write_detailed(
            membership_id,
            if stage {
                "stage_submodule_pointer"
            } else {
                "unstage_submodule_pointer"
            },
            Some(details),
            None,
            None,
            |git, root| {
                git.apply_submodule_pointer(root, &preview.submodule, stage, token, &recovery)
            },
        )
    }
    pub fn preview_submodule_update(
        &self,
        membership_id: &str,
        path: &NativePath,
        mode: &str,
    ) -> Result<SubmoduleUpdatePreview> {
        let sub = self
            .submodules(membership_id)?
            .into_iter()
            .find(|sub| &sub.path == path)
            .ok_or_else(|| {
                AppError::new(
                    "submodule",
                    "Choose a discovered submodule from this repository.",
                )
            })?;
        gitnova_submodule::update::preview(&self.git, &self.path(membership_id)?, &sub, mode)
    }
    pub fn update_submodule(
        &self,
        membership_id: &str,
        path: &NativePath,
        mode: &str,
        token: &str,
    ) -> Result<OperationReceipt> {
        let sub = self
            .submodules(membership_id)?
            .into_iter()
            .find(|sub| &sub.path == path)
            .ok_or_else(|| {
                AppError::new(
                    "submodule",
                    "Submodule relation changed. Discover and review it again.",
                )
            })?;
        let target_oid = if mode == "remote" {
            gitnova_submodule::update::preview(&self.git, &self.path(membership_id)?, &sub, mode)?
                .target_oid
        } else {
            sub.index_oid.clone()
        };
        let details = SubmoduleOperationReceipt {
            recovery_index: None,
            relation_id: sub.relation_id.clone(),
            path: sub.path.clone(),
            parent_repository_id: sub.parent_repository_id.clone(),
            repository_id: sub.repository_id.clone(),
            head_before: sub.checked_out_oid.clone(),
            target_oid,
            head_after: None,
        };
        self.execute_write_detailed(
            membership_id,
            if mode == "initialize" {
                "initialize_submodule"
            } else if mode == "remote" {
                "update_submodule_remote"
            } else {
                "restore_submodule"
            },
            Some(details),
            None,
            None,
            |git, root| gitnova_submodule::update::apply(git, root, &sub, mode, token),
        )
    }
    pub fn preview_create_worktree(
        &self,
        membership_id: &str,
        request: &WorktreeCreateRequest,
    ) -> Result<WorktreeCreatePreview> {
        gitnova_worktree::preview_create(&self.git, &self.path(membership_id)?, request)
    }
    pub fn create_worktree(
        &self,
        membership_id: &str,
        request: &WorktreeCreateRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        let preview = self.preview_create_worktree(membership_id, request)?;
        self.execute_write_detailed(
            membership_id,
            "create_worktree",
            None,
            None,
            Some(PlannedOperation::WorktreeCreate(preview)),
            |git, root| gitnova_worktree::create(git, root, request, token),
        )
    }
    fn removal_impact(
        catalog: &Catalog,
        preview: &WorktreeRemovePreview,
    ) -> (Option<String>, Vec<WorktreeMembershipImpact>) {
        let repository = catalog
            .repositories
            .iter()
            .find(|r| r.common_dir == preview.common_dir);
        let worktree = repository.and_then(|repo| {
            catalog.worktrees.iter().find(|w| {
                w.repository_id == repo.id && Some(&w.git_dir) == preview.git_dir.as_ref()
            })
        });
        let mut impact = vec![];
        if let Some(tree) = worktree {
            for workspace in &catalog.workspaces {
                for member in &workspace.memberships {
                    if member.worktree_id.as_deref() == Some(tree.id.as_str())
                        && member.repository_id == tree.repository_id
                    {
                        impact.push(WorktreeMembershipImpact {
                            workspace_id: workspace.id.clone(),
                            workspace_name: workspace.name.clone(),
                            membership_id: member.id.clone(),
                            name: member.name.clone(),
                            worktree_id: tree.id.clone(),
                            selected: workspace.selected_id.as_deref() == Some(member.id.as_str()),
                        });
                    }
                }
            }
        }
        impact.sort_by(|a, b| {
            (&a.workspace_id, &a.membership_id).cmp(&(&b.workspace_id, &b.membership_id))
        });
        (worktree.map(|w| w.id.clone()), impact)
    }
    pub fn preview_remove_worktree(
        &self,
        membership_id: &str,
        request: &WorktreeRemoveRequest,
    ) -> Result<WorktreeRemovePreview> {
        let mut request = request.clone();
        if request.id.is_empty() {
            request.id = id();
        }
        let mut preview =
            gitnova_worktree::preview_remove(&self.git, &self.path(membership_id)?, &request)?;
        (preview.worktree_id, preview.memberships) =
            Self::removal_impact(&self.catalog()?, &preview);
        preview.token = gitnova_worktree::membership_token(
            &preview.git_token,
            &preview.worktree_id,
            &preview.memberships,
        )?;
        Ok(preview)
    }
    pub fn remove_worktree(
        &self,
        membership_id: &str,
        request: &WorktreeRemoveRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        let preview = self.preview_remove_worktree(membership_id, request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Worktree or workspace membership changed. Review removal again.",
            ));
        }
        self.execute_write_detailed(membership_id, "remove_worktree", None, None, Some(PlannedOperation::WorktreeRemove(preview.clone())), |git, root| {
            let latest = self.preview_remove_worktree(membership_id, request)?;
            if latest.token != token { return Err(AppError::new("stale_preview", "Worktree or workspace membership changed. Review removal again.")); }
            gitnova_worktree::remove(git, root, &latest.request, &latest.git_token)?;
            self.update(|catalog| {
                let (worktree_id, impact) = Self::removal_impact(catalog, &latest);
                if worktree_id != latest.worktree_id || impact != latest.memberships { return Err(AppError::new("operation_unknown", "Worktree removed, but workspace membership changed. Inspect the catalog and recovery ref before retrying.")); }
                for workspace in &mut catalog.workspaces {
                    workspace.memberships.retain(|member| !latest.memberships.iter().any(|item| item.membership_id == member.id && item.workspace_id == workspace.id));
                    if workspace.selected_id.as_ref().is_some_and(|id| !workspace.memberships.iter().any(|m| &m.id == id)) { workspace.selected_id = None; }
                }
                if let Some(id) = &latest.worktree_id { catalog.worktrees.retain(|w| &w.id != id); }
                Ok(())
            }).map_err(|e| AppError::new("operation_unknown", format!("Worktree removed, but workspace cleanup could not be published: {}", e.message)))
        })
    }
    pub fn worktrees(&self, membership_id: &str) -> Result<Vec<LinkedWorktree>> {
        gitnova_worktree::inspect(&self.git, &self.path(membership_id)?)
    }
    /// The UI may open only paths independently rediscovered from this registered repository.
    pub fn open_related(&self, membership_id: &str, path: &NativePath) -> Result<Catalog> {
        let root = self.path(membership_id)?;
        if self
            .worktrees(membership_id)?
            .iter()
            .any(|w| &w.path == path && !w.prunable)
        {
            return self.add_repository(&path.to_path()?);
        }
        for sub in self.submodules(membership_id)? {
            if sub.path == *path && sub.initialized {
                return self.add_repository(&root.join(path.to_path()?));
            }
        }
        Err(AppError::new(
            "path",
            "Related repository is unavailable or not initialized",
        ))
    }
    pub fn snapshot(&self, membership_id: &str) -> Result<RepositorySnapshot> {
        let path = self.path(membership_id)?;
        let fingerprint = self.git.fingerprint(&path)?;
        Ok(RepositorySnapshot {
            tracking: self
                .display_tracking(&path, &fingerprint)
                .unwrap_or_default(),
            merge: self.merge_state(membership_id)?,
            fingerprint,
            changes: self.git.working_changes(&path)?,
        })
    }
    pub fn working_changes(&self, membership_id: &str) -> Result<WorkingChanges> {
        self.git.working_changes(&self.path(membership_id)?)
    }
    pub fn file_preview(
        &self,
        membership_id: &str,
        file: &NativePath,
        staged: bool,
    ) -> Result<FilePreview> {
        self.git
            .file_preview(&self.path(membership_id)?, file, staged)
    }
    pub fn commit_preview(&self, membership_id: &str) -> Result<CommitPreview> {
        self.with_repo_identity(membership_id, || {
            self.git.commit_preview(&self.path(membership_id)?)
        })
    }
    fn save_receipt(&self, receipt: &OperationReceipt) -> Result<()> {
        use std::io::Write;
        std::fs::create_dir_all(&self.receipt_dir)?;
        let mut file = tempfile::NamedTempFile::new_in(&self.receipt_dir)?;
        serde_json::to_writer_pretty(&mut file, receipt)
            .map_err(|e| AppError::new("receipt", e.to_string()))?;
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist(self.receipt_dir.join(format!("{}.json", receipt.id)))
            .map_err(|e| AppError::new("receipt", e.to_string()))?;
        Ok(())
    }
    fn execute_write(
        &self,
        membership_id: &str,
        kind: &str,
        operation: impl FnOnce(&GitCli, &Path) -> Result<()>,
    ) -> Result<OperationReceipt> {
        self.execute_write_detailed(membership_id, kind, None, None, None, operation)
    }
    fn execute_write_detailed(
        &self,
        membership_id: &str,
        kind: &str,
        submodule: Option<SubmoduleOperationReceipt>,
        graph_action: Option<GraphAction>,
        planned: Option<PlannedOperation>,
        operation: impl FnOnce(&GitCli, &Path) -> Result<()>,
    ) -> Result<OperationReceipt> {
        let path = self.path(membership_id)?;
        let catalog = self.catalog()?;
        let membership = catalog
            .workspaces
            .iter()
            .flat_map(|w| &w.memberships)
            .find(|m| m.id == membership_id)
            .ok_or_else(|| AppError::new("membership", "Unknown membership"))?;
        let repository = catalog
            .repositories
            .iter()
            .find(|r| r.id == membership.repository_id)
            .ok_or_else(|| AppError::new("repository", "Unknown repository identity"))?;
        let discovered = self.git.discover(&path)?;
        let changed_repository = NativePath::new(&discovered.common_dir) != repository.common_dir;
        let changed_worktree = membership
            .worktree_id
            .as_ref()
            .and_then(|id| catalog.worktrees.iter().find(|w| &w.id == id))
            .is_none_or(|w| {
                NativePath::new(&discovered.git_dir) != w.git_dir
                    || discovered
                        .root
                        .as_ref()
                        .is_none_or(|root| NativePath::new(root) != w.root)
            });
        if changed_repository || changed_worktree {
            return Err(AppError::new(
                "repository_replaced",
                "Repository location or Git identity changed. Reopen it explicitly before writing.",
            ));
        }
        let _guard = self.git.acquire_operation(&path)?;
        let before = self.git.status(&path)?.head;
        let mut receipt = OperationReceipt {
            branch_cleanup: None,
            merge: None,
            planned,
            graph_action,
            submodule,
            repository_id: membership.repository_id.clone(),
            worktree_id: membership.worktree_id.clone(),
            membership_id: membership_id.into(),
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            finished_at: None,
            id: id(),
            kind: kind.into(),
            status: "started".into(),
            summary: format!("{kind} started"),
            head_before: before.clone(),
            head_after: before,
            recovery: None,
        };
        self.save_receipt(&receipt)?;
        let result = operation(&self.git, &path);
        let mut submodule_inspected = true;
        if let Some(details) = &mut receipt.submodule {
            submodule_inspected = false;
            if let Ok(relative) = details.path.to_path() {
                let child = path.join(relative);
                if child.join(".git").exists() {
                    if let Ok(status) = self.git.status(&child) {
                        details.head_after = status.head;
                        submodule_inspected =
                            details.head_after.as_ref() == Some(&details.target_oid);
                        if let Ok((repository_id, _)) = self.discovered_identity(&child) {
                            details.repository_id = Some(repository_id);
                        }
                    }
                }
            }
        }
        let inspected = self.git.status(&path);
        receipt.head_after = inspected.as_ref().ok().and_then(|s| s.head.clone());
        match result {
            Ok(()) if inspected.is_ok() && submodule_inspected => {
                receipt.status = "succeeded".into();
                receipt.summary = match kind {
                    "merge_begin" if matches!(&receipt.planned, Some(PlannedOperation::Merge(p)) if p.operation == "merge" && receipt.head_after.as_deref() == Some(p.source_oid.as_str()) && receipt.head_before != receipt.head_after) => "Branch fast-forwarded to the reviewed commit; no merge commit was created".into(),
                    "merge_begin" => "Merge prepared; inspect its resulting state".into(),
                    "merge_resolve" => "Resolution staged; review the remaining merge result".into(),
                    "merge_finish" => "Merge commit created from the reviewed staged result".into(),
                    "merge_release" => "Interrupted session closed; files, staging and recovery refs preserved".into(),
                    "merge_abort" => "Merge aborted; edited/staged work retained in its recovery snapshot".into(),
                    "ref_operation" => receipt.planned.as_ref().map_or_else(|| "Ref operation completed".into(), |planned| match planned { PlannedOperation::Ref(p) => match p.action.kind {
                        RefOperationKind::RenameBranch => format!("Renamed {} to {}",p.action.name,p.action.new_name),
                        RefOperationKind::DeleteBranch | RefOperationKind::DeleteTag => format!("Deleted local {}; recovery: {}",p.action.name,p.recovery_ref.as_deref().unwrap_or("inspect receipt")),
                        RefOperationKind::SetUpstream => format!("Upstream for {}: {}/{}",p.action.name,p.action.remote,p.action.remote_branch),
                        RefOperationKind::UnsetUpstream => format!("Upstream removed from {}",p.action.name), }, _ => "Ref operation completed".into() }),
                    "create_change_request" => "Provider request result verified".into(),
                    "delete_remote_branch" => "Remote branch deleted; local branches and recovery retained".into(),
                    "amend_commit" => "Last commit amended; original retained for recovery".into(),
                    "create_tag" => "Tag created at the reviewed commit".into(),
                    "push_tag" => "Selected tag publication verified".into(),
                    "push_branch" => receipt.planned.as_ref().map_or_else(|| "Push completed".into(), |planned| if let PlannedOperation::Push(p)=planned {format!("Pushed {} to {}/{} at {}{}",p.request.branch,p.request.remote,p.request.remote_branch,&p.request.oid[..8],if p.request.set_upstream {"; upstream set"}else{""})}else{"Push completed".into()}),
                    "discard_changes" => "Selected changes discarded; original content saved for recovery".into(),
                    "remove_worktree" => "Linked worktree removed; branch and commit retained for recovery".into(),
                    "create_worktree" => "Worktree created; open it from Related repositories".into(),
                    "stash_push" => "Changes saved to stash; saved work is retained for recovery".into(),
                    "stash_apply" => "Stash applied; saved entry retained".into(),
                    "stash_pop" => "Stash applied and removed from the list; recovery retained".into(),
                    "stash_drop" => "Stash removed from the list; recovery retained".into(),
                    "stage_submodule_pointer" => "Submodule pointer staged; files inside the submodule were preserved".into(),
                    "unstage_submodule_pointer" => "Submodule pointer unstaged; checkout and files were preserved".into(),
                    "stage_all" => "Reviewed files staged; excluded paths and working files preserved".into(),
                    "add_remote" => "Remote added; no fetch or push performed".into(),
                    "stage" => "File staged".into(),
                    "worktree_maintenance" => "Worktree metadata updated; checkout files preserved".into(),
                    "sync_submodule" => "Selected submodule URLs synchronized; commits and files preserved".into(),
                    "stage_hunk" => "Hunk staged; other changes preserved".into(),
                    "unstage_hunk" => "Hunk unstaged; working file preserved".into(),
                    "unstage" => "File unstaged; working file preserved".into(),
                    "initialize_submodule" => receipt.submodule.as_ref().map_or_else(|| "Submodule initialized".into(), |sub| format!("Initialized {} at recorded {}", sub.path.display, &sub.target_oid[..8])),
                    "submodule_registration_index" => "Registration staging updated; child files and branches retained.".into(),
                     "submodule_registration" => "Submodule registration staged; review .gitmodules and the pointer before committing. Child files and history retained.".into(),
                    "update_submodule_remote" => "Submodule updated from cached remote branch; parent staging preserved. Review and stage its pointer separately.".into(),
                    "restore_submodule" => receipt.submodule.as_ref().map_or_else(|| "Submodule restored".into(), |sub| format!("Restored {} to recorded {}; parent index preserved", sub.path.display, &sub.target_oid[..8])),
                    "graph_action" => receipt.graph_action.as_ref().map_or_else(|| "Graph action completed".into(), |action| match action.kind {
                        GraphActionKind::CheckoutCommit => format!("Checked out {} (detached HEAD)", &action.oid[..8]),
                        GraphActionKind::CreateBranch => format!("Created and opened {} at {}", action.name, &action.oid[..8]),
                        GraphActionKind::CreateTag => format!("Created lightweight tag {} at {}", action.name, &action.oid[..8]),
                    }),
                    "pull_current" => "Current branch pulled by fast-forward; local commits preserved".into(),
                    "fetch_remote" => "Remote branches fetched; local files and branches preserved".into(),
                    "switch_remote_branch" => "Local tracking branch ready; fetched changes integrated by fast-forward and local commits preserved".into(),
                    "switch_branch" => format!(
                        "Switched to {}",
                        inspected
                            .as_ref()
                            .ok()
                            .and_then(|status| status.branch.as_deref())
                            .unwrap_or("reviewed branch")
                    ),
                    _ => format!(
                        "Commit created: {}",
                        receipt.head_after.as_deref().unwrap_or("unknown")
                    ),
                };
            }
            Ok(()) => {
                receipt.status = "unknown".into();
                receipt.summary =
                    "Git completed, but its resulting state could not be inspected".into();
                receipt.recovery =
                    Some("Inspect the repository in Git before repeating this operation.".into());
            }
            Err(error) => {
                let remote_partial = matches!(
                    kind,
                    "discard_changes"
                        | "remove_worktree"
                        | "create_worktree"
                        | "merge_begin"
                        | "merge_resolve"
                        | "merge_unstage"
                        | "merge_discard"
                        | "merge_finish"
                        | "merge_abort"
                        | "merge_release"
                        | "add_remote"
                        | "stage_all"
                        | "ref_operation"
                        | "delete_remote_branch"
                        | "push_branch"
                        | "amend_commit"
                        | "create_tag"
                        | "push_tag"
                        | "create_hosted_repository"
                        | "create_change_request"
                        | "graph_action"
                        | "fetch_remote"
                        | "switch_remote_branch"
                        | "pull_current"
                        | "initialize_submodule"
                        | "restore_submodule"
                        | "submodule_registration"
                        | "submodule_registration_index"
                        | "update_submodule_remote"
                        | "stage_submodule_pointer"
                        | "unstage_submodule_pointer"
                ) && !matches!(
                    error.code.as_str(),
                    "stale_preview"
                        | "discard_blocked"
                        | "worktree_blocked"
                        | "worktree_request"
                        | "switch_blocked"
                        | "remote"
                        | "branch"
                        | "remote_branch"
                        | "operation_in_progress"
                        | "submodule_blocked"
                        | "submodule_pointer"
                        | "merge_blocked"
                        | "merge_resolve"
                        | "merge_path"
                        | "merge"
                        | "ref_blocked"
                        | "remote_delete_blocked"
                        | "push_blocked"
                        | "amend_blocked"
                        | "tag_blocked"
                        | "tag_request"
                        | "hosting"
                        | "hosting_network"
                        | "ref_operation"
                        | "push"
                        | "graph_blocked"
                        | "graph_action"
                );
                receipt.status = if error.code == "stash_conflict" {
                    "conflicted"
                } else if kind.starts_with("stash_")
                    && !matches!(
                        error.code.as_str(),
                        "stale_preview"
                            | "stash_blocked"
                            | "stash_request"
                            | "stash_shape"
                            | "stash_budget"
                    )
                    || error.code == "operation_unknown"
                    || remote_partial
                    || receipt.head_after != receipt.head_before
                {
                    "unknown"
                } else {
                    "failed"
                }
                .into();
                receipt.summary = error.message;
                if remote_partial && matches!(kind, "discard_changes" | "merge_discard") {
                    receipt.recovery = Some("Inspect the selected files, index and saved recovery folder before retrying. Discard is not atomic; no automatic rollback or retry was attempted.".into());
                } else if remote_partial && kind == "remove_worktree" {
                    receipt.recovery = Some("The checkout or metadata may already be removed, and workspace entries may need reconciliation. Inspect git worktree list, the reviewed folder and recovery ref before retrying. The branch was not selected for deletion; no rollback or automatic retry was attempted.".into());
                } else if remote_partial && kind == "create_worktree" {
                    receipt.recovery = Some("Worktree files, a new branch or administrative metadata may remain. Inspect the reviewed destination and git worktree list before retrying. No directory deletion, forced reset or automatic rollback was attempted.".into());
                } else if remote_partial && kind == "create_change_request" {
                    receipt.recovery=Some("The provider may already have created the request. Refresh existing requests before retrying; no API POST was replayed and branch publication was not rolled back.".into());
                } else if remote_partial && (kind == "create_tag" || kind == "push_tag") {
                    receipt.recovery=Some("Inspect the named local and remote tag before retrying. Tag creation/publication may have completed; the local tag was not rolled back.".into());
                } else if remote_partial && kind == "amend_commit" {
                    receipt.recovery=Some("The amendment may have completed. Inspect HEAD, index, hook output and the original recovery ref before retrying. No reset, push or automatic rollback was attempted.".into());
                } else if remote_partial && kind == "delete_remote_branch" {
                    receipt.recovery=Some("The remote branch may already be deleted. Inspect the live destination, cached tracking ref and saved recovery ref before retrying. Local branches were not selected for deletion; no automatic retry or rollback was attempted.".into());
                } else if remote_partial && kind == "push_branch" && error.code == "git_auth" {
                    receipt.recovery=Some("Authentication did not complete. Check the matching account in Settings and inspect the destination before retrying. No automatic retry was attempted.".into());
                } else if remote_partial && kind == "push_branch" {
                    receipt.recovery=Some("The remote may already contain the reviewed commit even if local tracking/upstream updates failed. Inspect the destination and local configuration before retrying. Push, cache refresh and upstream configuration are separate effects; no rollback or automatic retry was attempted.".into());
                } else if remote_partial && kind == "ref_operation" {
                    receipt.recovery=Some("Ref or configuration updates may have completed partially. Inspect the requested names and recovery ref in this receipt before retrying. No automatic rollback was attempted.".into());
                } else if remote_partial && kind.starts_with("merge_") {
                    receipt.recovery = Some("Merge checkout, index or commit changes may have completed partially. Inspect the merge state, retained tips and any recovery snapshot in this receipt before retrying. No forced reset, automatic rollback or retry was attempted.".into());
                } else if remote_partial && kind == "graph_action" {
                    receipt.recovery = Some("Checkout or ref creation may have completed partially. Inspect HEAD and the requested branch/tag before retrying; no rollback or automatic retry was attempted.".into());
                } else if remote_partial {
                    receipt.recovery = Some("Fetch, initialization or checkout may have completed partially. Fetched refs/objects, a created local branch or its tracking config may remain. Refresh and inspect the selected local branch before retrying; no rollback or automatic retry was attempted.".into());
                }
                if receipt.recovery.is_none() {
                    receipt.recovery=Some("Refresh and inspect HEAD, staged files and any Git hook output before retrying. No automatic rollback or retry was attempted.".into());
                }
            }
        }
        if kind.starts_with("merge_") {
            match self.git.merge_state(&path) {
                Ok(state) => {
                    if receipt.status == "succeeded" && state.phase == "pending" {
                        receipt.status = if state.conflicts.is_empty() {
                            "pending"
                        } else {
                            "conflicted"
                        }
                        .into();
                        receipt.summary = if state.conflicts.is_empty() {
                            "Merge staged; review and finish the merge commit".into()
                        } else {
                            format!("Merge paused with {} conflict paths; resolve them in Working changes",state.conflicts.len())
                        };
                    }
                    if state.operation != "merge" {
                        let label = if state.operation == "rebase" {
                            "Rebase"
                        } else if state.operation == "cherryPick" {
                            "Cherry-pick"
                        } else {
                            "Revert"
                        };
                        if receipt.status == "pending" {
                            receipt.summary = if state.operation == "rebase" {
                                "Rebase paused; review the staged result before continuing".into()
                            } else {
                                format!("{label} staged; review and finish the commit")
                            };
                        } else if receipt.status == "conflicted" {
                            receipt.summary = format!("{label} paused with {} conflict paths; resolve them in Working changes",state.conflicts.len());
                        }
                    }
                    if receipt.status == "succeeded" {
                        if let Some(PlannedOperation::Merge(plan)) = &receipt.planned {
                            if plan.operation != "merge" {
                                let label = if plan.operation == "rebase" {
                                    "Rebase"
                                } else if plan.operation == "cherryPick" {
                                    "Cherry-pick"
                                } else {
                                    "Revert"
                                };
                                receipt.summary = if kind == "merge_finish" {
                                    format!(
                                        "{label} commit created from the reviewed staged result"
                                    )
                                } else {
                                    format!("{label} aborted; edits saved in the recovery snapshot")
                                };
                            }
                        }
                    }
                    if kind == "merge_unstage" && receipt.status == "pending" {
                        receipt.summary="Selected files unstaged; working files preserved. Operation still pending.".into();
                    }
                    if kind == "merge_discard" && receipt.status == "pending" {
                        receipt.summary="Selected changes discarded; original content saved for recovery. Operation still pending.".into();
                    }
                    receipt.merge = Some(state);
                }
                Err(error) => {
                    receipt.status = "unknown".into();
                    receipt.recovery=Some(format!("Merge state could not be read: {}. Inspect the owned session and recovery location before retrying.",error.message));
                }
            }
            if let Some(PlannedOperation::Merge(plan)) = &receipt.planned {
                if let Some(path) = &plan.recovery_dir {
                    let message=format!("Worktree/index recovery location: {}. It may have been created before a partial operation; inspect its manifest before restoring.",path.display);
                    receipt.recovery = Some(match receipt.recovery.take() {
                        Some(prior) => format!("{prior} {message}"),
                        None => message,
                    });
                }
            }
        }
        if receipt.status != "succeeded" {
            if let Some(PlannedOperation::Ref(preview)) = &receipt.planned {
                if let Some(recovery) = &preview.recovery_ref {
                    let guidance = receipt.recovery.clone().unwrap_or_default();
                    receipt.recovery=Some(format!("{guidance} Recovery location: {recovery}. Verify whether it exists before restoring; the original ref may still be present."));
                }
            }
        }
        if let Some(index) = receipt
            .submodule
            .as_ref()
            .and_then(|sub| sub.recovery_index.as_ref())
        {
            if index.to_path().is_ok_and(|path| path.is_file()) {
                receipt.recovery = Some(format!("Original parent index saved at {}. Inspect current staging before restoring; child files and refs were not changed.", index.display));
            }
        }
        receipt.finished_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        if kind.starts_with("stash_") && receipt.status != "succeeded" {
            receipt.recovery=Some("The stash or recovery ref may already exist and files/index may have changed. Inspect Working changes, the stash list and recovery metadata before retrying. Pop never drops an entry after a failed/conflicted application. Resolve stash conflicts with Git; no automatic reset, rollback or retry is performed.".into());
        }
        self.save_receipt(&receipt).map_err(|e|AppError::new("receipt",format!("Git operation finished with status {} but receipt persistence failed: {}. Inspect HEAD {:?} before retrying.",receipt.status,e,receipt.head_after)))?;
        Ok(receipt)
    }
    pub fn preview_discard(
        &self,
        membership_id: &str,
        request: &DiscardRequest,
    ) -> Result<DiscardPreview> {
        let mut request = request.clone();
        if request.id.is_empty() {
            request.id = id();
        }
        self.git
            .preview_discard(&self.path(membership_id)?, &request)
    }
    pub fn preview_operation_discard(
        &self,
        membership_id: &str,
        request: &DiscardRequest,
        operation_token: &str,
    ) -> Result<DiscardPreview> {
        self.with_repo_identity(membership_id, || {
            let mut request = request.clone();
            if request.id.is_empty() {
                request.id = id();
            }
            self.git.preview_operation_discard(
                &self.path(membership_id)?,
                &request,
                operation_token,
            )
        })
    }
    pub fn discard_operation_changes(
        &self,
        membership_id: &str,
        request: &DiscardRequest,
        token: &str,
        operation_token: &str,
    ) -> Result<OperationReceipt> {
        self.with_repo_identity(membership_id, || {
            let p = self.preview_operation_discard(membership_id, request, operation_token)?;
            if p.token != token {
                return Err(AppError::new(
                    "stale_preview",
                    "Discard selection changed. Review the current files again.",
                ));
            }
            self.execute_write_detailed(
                membership_id,
                "merge_discard",
                None,
                None,
                Some(PlannedOperation::Discard(p)),
                |git, root| git.discard_operation_changes(root, request, token, operation_token),
            )
        })
    }
    pub fn discard_changes(
        &self,
        membership_id: &str,
        request: &DiscardRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        let preview = self.preview_discard(membership_id, request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Discard selection changed. Review the current files again.",
            ));
        }
        self.execute_write_detailed(
            membership_id,
            "discard_changes",
            None,
            None,
            Some(PlannedOperation::Discard(preview)),
            |git, root| git.discard_changes(root, request, token),
        )
    }
    pub fn preview_stage_all(&self, id: &str) -> Result<StageAllPreview> {
        self.git.preview_stage_all(&self.path(id)?)
    }
    pub fn stage_all(&self, id: &str, token: &str) -> Result<OperationReceipt> {
        let p = self.preview_stage_all(id)?;
        if p.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Stage all selection changed. Review again.",
            ));
        }
        self.execute_write_detailed(
            id,
            "stage_all",
            None,
            None,
            Some(PlannedOperation::StageAll(p)),
            |git, path| git.stage_all(path, token),
        )
    }
    pub fn commit_file_preview(
        &self,
        id: &str,
        path: &NativePath,
        token: &str,
    ) -> Result<FilePreview> {
        self.with_repo_identity(id, || {
            self.git.commit_file_preview(&self.path(id)?, path, token)
        })
    }
    pub fn preview_remote_add(
        &self,
        id: &str,
        request: &RemoteAddRequest,
    ) -> Result<RemoteAddPreview> {
        self.git.preview_remote_add(&self.path(id)?, request)
    }
    pub fn add_remote(
        &self,
        id: &str,
        request: &RemoteAddRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        let p = self.preview_remote_add(id, request)?;
        if p.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Remote configuration review changed. Review again.",
            ));
        }
        self.execute_write_detailed(
            id,
            "add_remote",
            None,
            None,
            Some(PlannedOperation::AddRemote(p)),
            |git, path| git.add_remote(path, request, token),
        )
    }
    pub fn preview_worktree_maintenance(
        &self,
        id: &str,
        request: &WorktreeMaintenanceRequest,
    ) -> Result<WorktreeMaintenancePreview> {
        self.git
            .preview_worktree_maintenance(&self.path(id)?, request)
    }
    pub fn maintain_worktree(
        &self,
        id: &str,
        request: &WorktreeMaintenanceRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        let preview = self.preview_worktree_maintenance(id, request)?;
        let actual_request = preview.request.clone();
        self.execute_write_detailed(
            id,
            "worktree_maintenance",
            None,
            None,
            Some(PlannedOperation::WorktreeMaintenance(preview)),
            |git, root| {
                let request=&actual_request;
                git.maintain_worktree(root, request, token)?;
                if request.kind=="reattach" || request.kind=="prune" {
                    let repository_id = self.catalog()?.workspaces.iter().flat_map(|w|&w.memberships).find(|m|m.id==id).map(|m|m.repository_id.clone()).ok_or_else(||AppError::new("operation_unknown","Worktree metadata changed but workspace identity is unavailable."))?;
                    let repaired = if request.kind=="reattach" {Some(git.discover(Path::new(&request.destination))?)} else {None};
                    self.update(|catalog|{
                        let removed = catalog.worktrees.iter().filter(|t|t.repository_id==repository_id && t.root==request.path).map(|t|t.id.clone()).collect::<Vec<_>>();
                        if let Some(target)=&repaired {
                            for tree in &mut catalog.worktrees {
                                if tree.repository_id==repository_id && tree.root==request.path && tree.git_dir==NativePath::new(&target.git_dir) {tree.root=NativePath::new(target.root.as_ref().ok_or_else(||AppError::new("operation_unknown","Repaired checkout root unavailable."))?);}
                            }
                        }else{
                            catalog.worktrees.retain(|t|!removed.contains(&t.id));
                            for workspace in &mut catalog.workspaces {
                                workspace.memberships.retain(|m|m.repository_id!=repository_id || !m.worktree_id.as_ref().is_some_and(|w|removed.contains(w)));
                                if workspace.selected_id.as_ref().is_some_and(|id|!workspace.memberships.iter().any(|m|&m.id==id)){workspace.selected_id=workspace.memberships.first().map(|m|m.id.clone());}
                            }
                        }
                        Ok(())
                    }).map_err(|e|AppError::new("operation_unknown",format!("Worktree metadata changed, but workspace paths could not be saved: {}. Inspect before retrying.",e.message)))?;
                }
                Ok(())
            },
        )
    }
    pub fn preview_submodule_lifecycle(
        &self,
        id: &str,
        request: &SubmoduleLifecycleRequest,
    ) -> Result<SubmoduleLifecyclePreview> {
        let mut request = request.clone();
        if request.id.is_empty() {
            request.id = gitnova_workspace::id().replace('-', "");
        }
        gitnova_submodule::lifecycle::preview(&self.git, &self.path(id)?, &request)
    }
    pub fn change_submodule_lifecycle(
        &self,
        id: &str,
        request: &SubmoduleLifecycleRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        let p = self.preview_submodule_lifecycle(id, request)?;
        self.execute_write_detailed(
            id,
            if ["stage", "unstage"].contains(&request.kind.as_str()) {
                "submodule_registration_index"
            } else {
                "submodule_registration"
            },
            None,
            None,
            Some(PlannedOperation::SubmoduleLifecycle(Box::new(p))),
            |git, root| gitnova_submodule::lifecycle::apply(git, root, request, token),
        )
    }
    pub fn preview_submodule_sync(
        &self,
        id: &str,
        path: &NativePath,
    ) -> Result<SubmoduleSyncPreview> {
        let root = self.path(id)?;
        let sub = gitnova_submodule::inspect(&self.git, &root)?
            .into_iter()
            .find(|v| v.state.path == *path)
            .ok_or_else(|| AppError::new("submodule", "Choose a discovered submodule."))?;
        gitnova_submodule::sync::preview(&self.git, &root, &sub.state)
    }
    pub fn sync_submodule(
        &self,
        id: &str,
        path: &NativePath,
        token: &str,
    ) -> Result<OperationReceipt> {
        let preview = self.preview_submodule_sync(id, path)?;
        self.execute_write_detailed(
            id,
            "sync_submodule",
            None,
            None,
            Some(PlannedOperation::SubmoduleSync(Box::new(preview))),
            |git, root| {
                let sub = gitnova_submodule::inspect(git, root)?
                    .into_iter()
                    .find(|v| v.state.path == *path)
                    .ok_or_else(|| AppError::new("submodule", "Choose a discovered submodule."))?;
                gitnova_submodule::sync::apply(git, root, &sub.state, token)
            },
        )
    }
    pub fn preview_hunks(&self, id: &str, path: &NativePath, staged: bool) -> Result<HunkPreview> {
        self.git.preview_hunks(&self.path(id)?, path, staged)
    }
    pub fn change_hunk(
        &self,
        id: &str,
        path: &NativePath,
        staged: bool,
        hunk: usize,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.execute_write(
            id,
            if staged { "unstage_hunk" } else { "stage_hunk" },
            |git, root| git.change_hunk(root, path, staged, hunk, token),
        )
    }
    pub fn change_index(
        &self,
        membership_id: &str,
        file: &NativePath,
        stage: bool,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.execute_write(
            membership_id,
            if stage { "stage" } else { "unstage" },
            |git, path| git.change_index(path, file, stage, token),
        )
    }
    pub fn amend_original(&self, membership_id: &str, oid: &str) -> Result<AmendOriginal> {
        self.with_repo_identity(membership_id, || {
            self.git.amend_original(&self.path(membership_id)?, oid)
        })
    }
    pub fn preview_amend(
        &self,
        membership_id: &str,
        request: &AmendRequest,
    ) -> Result<AmendPreview> {
        self.with_repo_identity(membership_id, || {
            self.git.preview_amend(&self.path(membership_id)?, request)
        })
    }
    pub fn amend_commit(
        &self,
        membership_id: &str,
        request: &AmendRequest,
        token: &str,
        acknowledge_rewrite: bool,
    ) -> Result<OperationReceipt> {
        self.with_repo_identity(membership_id, || {
            let preview = self.preview_amend(membership_id, request)?;
            if preview.token != token {
                return Err(AppError::new(
                    "stale_preview",
                    "Amend review changed. Review again.",
                ));
            }
            self.execute_write_detailed(
                membership_id,
                "amend_commit",
                None,
                None,
                Some(PlannedOperation::Amend(preview)),
                |git, path| git.apply_amend(path, request, token, acknowledge_rewrite),
            )
        })
    }
    pub fn create_commit(
        &self,
        membership_id: &str,
        message: &str,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.with_repo_identity(membership_id, || {
            self.execute_write(membership_id, "commit", |git, path| {
                git.create_commit(path, message, token)
            })
        })
    }
    pub fn merge_state(&self, id: &str) -> Result<MergeState> {
        self.with_repo_identity(id, || self.git.merge_state(&self.path(id)?))
    }
    pub fn preview_merge(&self, id: &str, request: &MergeRequest) -> Result<MergeStartPreview> {
        self.with_repo_identity(id, || self.git.preview_merge(&self.path(id)?, request))
    }
    pub fn begin_merge(
        &self,
        id: &str,
        request: &MergeRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.with_repo_identity(id, || {
            let p = self.preview_merge(id, request)?;
            if p.token != token {
                return Err(AppError::new(
                    "stale_preview",
                    "Operation review changed. Review again.",
                ));
            }
            let cleanup_request = p
                .request
                .delete_source_branch
                .then(|| (p.request.clone(), p.branch.clone()));
            let plan = MergeReceipt {
                operation: p.request.operation.clone(),
                mainline: p.request.mainline,
                kind: "begin".into(),
                session_id: p.request.session_id.clone(),
                branch: p.branch,
                original_oid: p.original_oid,
                source_oid: p.request.source_oid.clone(),
                path: None,
                choice: None,
                expected_tree: p.result_tree,
                recovery_ref: Some(p.recovery_ref),
                recovery_dir: None,
            };
            let receipt = self.execute_write_detailed(
                id,
                "merge_begin",
                None,
                None,
                Some(PlannedOperation::Merge(plan)),
                |git, path| git.begin_merge(path, request, token),
            )?;
            self.complete_merge_source_cleanup(id, receipt, cleanup_request)
        })
    }
    pub fn merge_file(&self, id: &str, path: &NativePath) -> Result<MergeFile> {
        self.with_repo_identity(id, || self.git.merge_file(&self.path(id)?, path))
    }
    pub fn merge_file_scope(&self, id: &str, path: &NativePath, staged: bool) -> Result<MergeFile> {
        self.with_repo_identity(id, || {
            self.git.merge_file_scope(&self.path(id)?, path, staged)
        })
    }
    pub fn preview_merge_action(
        &self,
        id: &str,
        action: &MergeAction,
    ) -> Result<MergeActionPreview> {
        self.with_repo_identity(id, || {
            self.git.preview_merge_action(&self.path(id)?, action)
        })
    }
    pub fn apply_merge_action(
        &self,
        id: &str,
        action: &MergeAction,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.with_repo_identity(id, || {
            let p = self.preview_merge_action(id, action)?;
            if p.token != token {
                return Err(AppError::new(
                    "stale_preview",
                    "Operation action changed. Review again.",
                ));
            }
            let cleanup_request = if action.kind == "finish" && p.state.delete_source_branch {
                Some((
                    MergeRequest {
                        acknowledge_rewrite: false,
                        delete_source_branch: true,
                        method: MergeMethod::Auto,
                        operation: p.state.operation.clone(),
                        mainline: None,
                        source_oid: p.state.source_oid.clone().unwrap_or_default(),
                        source_ref: p.state.source_ref.clone(),
                        session_id: p.state.session_id.clone().unwrap_or_default(),
                    },
                    p.state.branch.clone().unwrap_or_default(),
                ))
            } else {
                None
            };
            let plan = MergeReceipt {
                operation: p.state.operation.clone(),
                mainline: p.state.mainline,
                kind: action.kind.clone(),
                session_id: p.state.session_id.clone().unwrap_or_default(),
                branch: p.state.branch.clone().unwrap_or_default(),
                original_oid: p.state.original_oid.clone().unwrap_or_default(),
                source_oid: p.state.source_oid.clone().unwrap_or_default(),
                path: action.path.clone(),
                choice: action.choice.clone(),
                expected_tree: p.expected_tree,
                recovery_ref: p.state.recovery_ref,
                recovery_dir: p.recovery_dir,
            };
            let receipt = self.execute_write_detailed(
                id,
                &format!("merge_{}", action.kind),
                None,
                None,
                Some(PlannedOperation::Merge(plan)),
                |git, path| git.apply_merge_action(path, action, token),
            )?;
            self.complete_merge_source_cleanup(id, receipt, cleanup_request)
        })
    }
    pub fn preview_ref_operation(
        &self,
        membership_id: &str,
        action: &RefOperation,
    ) -> Result<RefOperationPreview> {
        self.git
            .preview_ref_operation(&self.path(membership_id)?, action)
    }
    pub fn apply_ref_operation(
        &self,
        membership_id: &str,
        action: &RefOperation,
        token: &str,
    ) -> Result<OperationReceipt> {
        let preview = self.preview_ref_operation(membership_id, action)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Ref/configuration changed. Review again.",
            ));
        }
        self.execute_write_detailed(
            membership_id,
            "ref_operation",
            None,
            None,
            Some(PlannedOperation::Ref(preview)),
            |git, path| git.apply_ref_operation(path, action, token),
        )
    }
    pub fn preview_tag(&self, membership_id: &str, request: &TagRequest) -> Result<TagPreview> {
        self.with_repo_identity(membership_id, || {
            self.git.preview_tag(&self.path(membership_id)?, request)
        })
    }
    pub fn change_tag(
        &self,
        membership_id: &str,
        request: &TagRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.with_repo_identity(membership_id, || {
            let preview = self.preview_tag(membership_id, request)?;
            if preview.token != token {
                return Err(AppError::new(
                    "stale_preview",
                    "Tag review changed. Review again.",
                ));
            }
            let kind = if matches!(request, TagRequest::Create { .. }) {
                "create_tag"
            } else {
                "push_tag"
            };
            self.execute_write_detailed(
                membership_id,
                kind,
                None,
                None,
                Some(PlannedOperation::Tag(preview)),
                |git, path| git.apply_tag(path, request, token),
            )
        })
    }
    pub fn preview_remote_delete(
        &self,
        membership_id: &str,
        request: &RemoteDeleteRequest,
    ) -> Result<RemoteDeletePreview> {
        self.git
            .preview_remote_delete(&self.path(membership_id)?, request)
    }
    pub fn delete_remote_branch(
        &self,
        membership_id: &str,
        request: &RemoteDeleteRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        let preview = self.preview_remote_delete(membership_id, request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Remote deletion review changed. Review again.",
            ));
        }
        self.execute_write_detailed(
            membership_id,
            "delete_remote_branch",
            None,
            None,
            Some(PlannedOperation::RemoteDelete(preview)),
            |git, path| git.apply_remote_delete(path, request, token),
        )
    }
    pub fn preview_push(&self, membership_id: &str, request: &PushRequest) -> Result<PushPreview> {
        self.git.preview_push(&self.path(membership_id)?, request)
    }
    pub fn push_branch(
        &self,
        membership_id: &str,
        request: &PushRequest,
        token: &str,
    ) -> Result<OperationReceipt> {
        let preview = self.preview_push(membership_id, request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Push source, destination or configuration changed. Review again.",
            ));
        }
        self.execute_write_detailed(
            membership_id,
            "push_branch",
            None,
            None,
            Some(PlannedOperation::Push(preview)),
            |git, path| git.apply_push(path, request, token),
        )
    }
    pub fn preview_graph_action(
        &self,
        membership_id: &str,
        action: &GraphAction,
    ) -> Result<GraphActionPreview> {
        self.git
            .preview_graph_action(&self.path(membership_id)?, action)
    }
    pub fn apply_graph_action(
        &self,
        membership_id: &str,
        action: &GraphAction,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.execute_write_detailed(
            membership_id,
            "graph_action",
            None,
            Some(action.clone()),
            None,
            |git, path| git.apply_graph_action(path, action, token),
        )
    }
    pub fn local_branches(&self, membership_id: &str) -> Result<Vec<LocalBranch>> {
        self.git.local_branches(&self.path(membership_id)?)
    }
    pub fn preview_branch_switch(
        &self,
        membership_id: &str,
        branch: &str,
    ) -> Result<BranchSwitchPreview> {
        self.git
            .preview_branch_switch(&self.path(membership_id)?, branch)
    }
    pub fn switch_branch(
        &self,
        membership_id: &str,
        branch: &str,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.execute_write(membership_id, "switch_branch", |git, path| {
            git.switch_branch(path, branch, token)
        })
    }
    pub fn remote_branches(&self, membership_id: &str) -> Result<RemoteBranches> {
        self.git.remote_branches(&self.path(membership_id)?)
    }
    /// Background refresh has no per-minute receipt/recovery archive. Only
    /// remote-tracking refs/object data may change; the normal app write lock
    /// and repository identity validation still apply.
    pub fn auto_fetch(&self, membership_id: &str) -> Result<Option<String>> {
        let path = self.path(membership_id)?;
        let catalog = self.catalog()?;
        let membership = catalog
            .workspaces
            .iter()
            .flat_map(|w| &w.memberships)
            .find(|m| m.id == membership_id)
            .ok_or_else(|| AppError::new("membership", "Unknown membership"))?;
        let repository = catalog
            .repositories
            .iter()
            .find(|r| r.id == membership.repository_id)
            .ok_or_else(|| AppError::new("repository", "Unknown repository identity"))?;
        let discovered = self.git.discover(&path)?;
        let changed_repository = NativePath::new(&discovered.common_dir) != repository.common_dir;
        if discovered.root.is_none() {
            return Ok(None);
        }
        let changed_worktree = membership
            .worktree_id
            .as_ref()
            .and_then(|id| catalog.worktrees.iter().find(|w| &w.id == id))
            .is_none_or(|w| {
                NativePath::new(&discovered.git_dir) != w.git_dir
                    || discovered
                        .root
                        .as_ref()
                        .is_none_or(|root| NativePath::new(root) != w.root)
            });
        if changed_repository || changed_worktree {
            return Err(AppError::new(
                "repository_replaced",
                "Repository location or Git identity changed. Reopen it explicitly before writing.",
            ));
        }
        let _guard = self.git.acquire_operation(&path)?;
        if self.git.merge_state(&path)?.phase != "none" {
            return Ok(None);
        }
        let status = self.git.status(&path)?;
        if status.bare {
            return Ok(None);
        }
        let remotes = self.git.remote_branches(&path)?;
        let upstream_remote = status
            .branch
            .as_ref()
            .and_then(|branch| {
                self.git
                    .text(
                        &path,
                        &["config", "--get", &format!("branch.{branch}.remote")],
                    )
                    .ok()
            })
            .map(|name| name.trim().to_owned());
        let remote = status
            .branch
            .as_ref()
            .and(upstream_remote)
            .filter(|name| remotes.remotes.contains(name))
            .or_else(|| {
                remotes
                    .remotes
                    .iter()
                    .find(|name| *name == "origin")
                    .cloned()
            })
            .or_else(|| {
                if remotes.remotes.len() == 1 {
                    remotes.remotes.first().cloned()
                } else {
                    None
                }
            });
        let Some(remote) = remote else {
            return Ok(None);
        };
        self.git.fetch_remote_background(&path, &remote)?;
        Ok(Some(remote))
    }
    pub fn fetch_remote(
        &self,
        membership_id: &str,
        remote: &str,
        branch: Option<&str>,
    ) -> Result<OperationReceipt> {
        self.execute_write(membership_id, "fetch_remote", |git, path| {
            git.fetch_remote(path, remote, branch)
        })
    }
    pub fn pull_upstream(&self, membership_id: &str) -> Result<BranchTracking> {
        self.git.pull_upstream(&self.path(membership_id)?)
    }
    pub fn preview_pull_current(&self, membership_id: &str) -> Result<RemoteSwitchPreview> {
        self.git.preview_pull_current(&self.path(membership_id)?)
    }
    pub fn pull_current(&self, membership_id: &str, token: &str) -> Result<OperationReceipt> {
        self.execute_write(membership_id, "pull_current", |git, path| {
            git.pull_current(path, token)
        })
    }
    pub fn preview_remote_switch(
        &self,
        membership_id: &str,
        remote: &str,
        branch: &str,
    ) -> Result<RemoteSwitchPreview> {
        self.git
            .preview_remote_switch(&self.path(membership_id)?, remote, branch)
    }
    pub fn switch_remote_branch(
        &self,
        membership_id: &str,
        remote: &str,
        branch: &str,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.execute_write(membership_id, "switch_remote_branch", |git, path| {
            git.switch_remote_branch(path, remote, branch, token)
        })
    }
}

fn register_identity(
    c: &mut Catalog,
    discovery: &gitnova_git::Discovery,
) -> (String, Option<String>) {
    let common = NativePath::new(&discovery.common_dir);
    let git_dir = NativePath::new(&discovery.git_dir);
    let root = discovery.root.as_deref().map(NativePath::new);
    let repository_id =
        if let Some(repository) = c.repositories.iter().find(|r| r.common_dir == common) {
            repository.id.clone()
        } else {
            let repository_id = id();
            c.repositories.push(Repository {
                id: repository_id.clone(),
                common_dir: common.clone(),
                object_format: discovery.object_format.clone(),
                bare: root.is_none(),
            });
            repository_id
        };
    let worktree_id = if let Some(root) = &root {
        if let Some(w) = c.worktrees.iter().find(|w| w.git_dir == git_dir) {
            Some(w.id.clone())
        } else {
            let worktree_id = id();
            c.worktrees.push(Worktree {
                id: worktree_id.clone(),
                repository_id: repository_id.clone(),
                git_dir: git_dir.clone(),
                root: root.clone(),
            });
            Some(worktree_id)
        }
    } else {
        None
    };
    (repository_id, worktree_id)
}

mod stale_lock;
