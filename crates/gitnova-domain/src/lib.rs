use serde::{Deserialize, Serialize};
use std::{
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}
impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for AppError {}
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new("io", e.to_string())
    }
}
pub type Result<T> = std::result::Result<T, AppError>;

/// The display string is never converted back to an operating-system path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePath {
    pub display: String,
    pub encoding: String,
    pub hex: String,
}
impl NativePath {
    pub fn new(path: &Path) -> Self {
        #[cfg(unix)]
        let (encoding, bytes) = {
            use std::os::unix::ffi::OsStrExt;
            ("unix", path.as_os_str().as_bytes().to_vec())
        };
        #[cfg(windows)]
        let (encoding, bytes) = {
            use std::os::windows::ffi::OsStrExt;
            (
                "windows",
                path.as_os_str()
                    .encode_wide()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
        };
        Self {
            display: path.to_string_lossy().into(),
            encoding: encoding.into(),
            hex: bytes.iter().map(|b| format!("{b:02x}")).collect(),
        }
    }
    pub fn to_path(&self) -> Result<PathBuf> {
        if !self.hex.len().is_multiple_of(2) || !self.hex.is_ascii() {
            return Err(AppError::new("path", "Invalid native path"));
        }
        let bytes: Vec<u8> = (0..self.hex.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&self.hex[i..i + 2], 16)
                    .map_err(|_| AppError::new("path", "Invalid path encoding"))
            })
            .collect::<Result<_>>()?;
        #[cfg(unix)]
        {
            use std::{ffi::OsString, os::unix::ffi::OsStringExt};
            if self.encoding != "unix" {
                return Err(AppError::new(
                    "path",
                    "Path belongs to a different operating system",
                ));
            }
            Ok(PathBuf::from(OsString::from_vec(bytes)))
        }
        #[cfg(windows)]
        {
            use std::{ffi::OsString, os::windows::ffi::OsStringExt};
            if self.encoding != "windows" || !bytes.len().is_multiple_of(2) {
                return Err(AppError::new("path", "Invalid Windows path"));
            }
            let wide: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            Ok(PathBuf::from(OsString::from_wide(&wide)))
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub id: String,
    pub common_dir: NativePath,
    pub object_format: String,
    pub bare: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub id: String,
    pub repository_id: String,
    pub git_dir: NativePath,
    pub root: NativePath,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Membership {
    pub id: String,
    pub repository_id: String,
    pub worktree_id: Option<String>,
    pub name: String,
    pub pinned: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub memberships: Vec<Membership>,
    pub selected_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub version: u32,
    pub repositories: Vec<Repository>,
    pub worktrees: Vec<Worktree>,
    pub workspaces: Vec<Workspace>,
    pub active_workspace_id: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub dirty: bool,
    pub changed_entries: usize,
    pub bare: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRef {
    pub name: String,
    pub oid: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub oid: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub subject: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub color: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaneStroke {
    pub lane: usize,
    pub color: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphLane {
    pub oid: String,
    pub color: usize,
    pub active: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRow {
    pub commit: Commit,
    pub lane: usize,
    pub color: usize,
    pub incoming: Vec<LaneStroke>,
    pub edges: Vec<Edge>,
    pub width: usize,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphCursor {
    pub version: u8,
    pub offset: usize,
    pub lanes: Vec<Option<GraphLane>>,
    pub next_color: usize,
    pub fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphPage {
    #[serde(default)]
    pub tracking: Vec<BranchTracking>,
    pub rows: Vec<GraphRow>,
    pub refs: Vec<GitRef>,
    pub status: RepoStatus,
    pub next: Option<GraphCursor>,
    pub fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitFile {
    pub path: NativePath,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalPatch {
    pub diff: String,
    pub truncated: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub files: Vec<CommitFile>,
    pub commit: Commit,
    pub message: String,
    pub diff: String,
    pub truncated: bool,
    pub comparison: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedWorktree {
    pub path: NativePath,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub bare: bool,
    pub locked: bool,
    pub prunable: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Submodule {
    pub relation_id: String,
    pub parent_relation_id: Option<String>,
    pub parent_repository_id: String,
    pub parent_worktree_id: Option<String>,
    pub repository_id: Option<String>,
    pub worktree_id: Option<String>,
    pub parent_path: NativePath,
    pub relative_path: NativePath,
    pub depth: usize,
    pub config_name: Option<String>,
    pub inspection_error: Option<String>,
    pub detached: Option<bool>,
    pub path: NativePath,
    pub head_oid: Option<String>,
    pub index_oid: String,
    pub checked_out_oid: Option<String>,
    pub initialized: bool,
    pub dirty: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: NativePath,
    pub index_status: String,
    pub worktree_status: String,
    pub staged: bool,
    pub unstaged: bool,
    pub blocked_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingChanges {
    pub files: Vec<ChangedFile>,
    pub status: RepoStatus,
    pub token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub path: NativePath,
    pub staged: bool,
    pub token: String,
    pub diff: String,
    pub truncated: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkPreview {
    pub file: FilePreview,
    pub headers: Vec<String>,
    pub token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitPreview {
    #[serde(default)]
    pub complete_manifest: bool,
    pub token: String,
    pub files: Vec<ChangedFile>,
    pub status: RepoStatus,
    pub diff: String,
    pub truncated: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationReceipt {
    #[serde(default)]
    pub branch_cleanup: Option<MergeBranchCleanup>,
    #[serde(default)]
    pub merge: Option<MergeState>,
    #[serde(default)]
    pub planned: Option<PlannedOperation>,
    #[serde(default)]
    pub graph_action: Option<GraphAction>,
    pub submodule: Option<SubmoduleOperationReceipt>,
    pub repository_id: String,
    pub worktree_id: Option<String>,
    pub membership_id: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub id: String,
    pub kind: String,
    pub status: String,
    pub summary: String,
    pub head_before: Option<String>,
    pub head_after: Option<String>,
    pub recovery: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySnapshot {
    #[serde(default)]
    pub tracking: Vec<BranchTracking>,
    pub merge: MergeState,
    pub fingerprint: String,
    pub changes: WorkingChanges,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchTracking {
    pub branch: String,
    pub oid: String,
    pub upstream: Option<String>,
    pub remote: Option<String>,
    pub remote_branch: Option<String>,
    pub push_remote: Option<String>,
    pub push_branch: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalBranch {
    pub name: String,
    pub oid: String,
    pub current: bool,
    pub checked_out_at: Option<NativePath>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchFileChange {
    pub path: NativePath,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSwitchPreview {
    pub branch: String,
    pub source_branch: Option<String>,
    pub source_oid: Option<String>,
    pub target_oid: String,
    pub files: Vec<BranchFileChange>,
    pub submodules: Vec<BranchSubmoduleChange>,
    pub token: String,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GraphActionKind {
    CheckoutCommit,
    CreateBranch,
    CreateTag,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphAction {
    pub kind: GraphActionKind,
    pub oid: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphActionPreview {
    pub action: GraphAction,
    pub switch: Option<BranchSwitchPreview>,
    pub token: String,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSubmoduleChange {
    pub path: NativePath,
    pub source_oid: String,
    pub target_oid: String,
    pub checked_out_oid: Option<String>,
    pub dirty: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranch {
    pub remote: String,
    pub name: String,
    pub oid: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranches {
    #[serde(default)]
    pub urls: std::collections::BTreeMap<String, Vec<String>>,
    pub remotes: Vec<String>,
    pub branches: Vec<RemoteBranch>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSwitchPreview {
    pub remote: String,
    pub remote_branch: String,
    pub remote_oid: String,
    pub create: bool,
    pub local_oid: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub switch: BranchSwitchPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleUpdatePreview {
    pub submodule: Submodule,
    pub mode: String,
    pub url: Option<String>,
    pub source_base: Option<String>,
    pub parent_status: RepoStatus,
    pub preserved_by: Vec<String>,
    pub network_required: bool,
    pub blocked_reason: Option<String>,
    pub token: String,
}

/// Display snapshot only; never a write authorization token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleSnapshot {
    pub status: RepoStatus,
    pub submodules: Vec<Submodule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleOperationReceipt {
    #[serde(default)]
    pub recovery_index: Option<NativePath>,
    pub relation_id: String,
    pub path: NativePath,
    pub parent_repository_id: String,
    pub repository_id: Option<String>,
    pub head_before: Option<String>,
    pub target_oid: String,
    pub head_after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmodulePointerPreview {
    pub version: u32,
    pub submodule: Submodule,
    pub stage: bool,
    pub target_oid: String,
    pub parent_status: RepoStatus,
    pub blocked_reason: Option<String>,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RefOperationKind {
    RenameBranch,
    DeleteBranch,
    DeleteTag,
    SetUpstream,
    UnsetUpstream,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefOperation {
    pub kind: RefOperationKind,
    pub name: String,
    pub oid: String,
    #[serde(default)]
    pub new_name: String,
    #[serde(default)]
    pub remote: String,
    #[serde(default)]
    pub remote_branch: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefOperationPreview {
    pub action: RefOperation,
    pub object_oid: String,
    pub recovery_ref: Option<String>,
    pub annotation: Option<String>,
    pub upstream_before: Option<String>,
    pub upstream_after: Option<String>,
    pub retained_by: Vec<String>,
    pub token: String,
    pub blocked_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushRequest {
    pub branch: String,
    pub oid: String,
    pub remote: String,
    pub remote_branch: String,
    #[serde(default)]
    pub set_upstream: bool,
    /// Per-operation bounded upload compatibility; never persisted to Git config.
    #[serde(default)]
    pub buffered_https: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushPreview {
    pub tracking_ref: Option<String>,
    pub tracking_oid: Option<String>,
    pub upstream_before: Option<String>,
    pub request: PushRequest,
    pub destination: String,
    pub remote_oid: Option<String>,
    pub outgoing: Vec<Commit>,
    pub outgoing_count: usize,
    pub token: String,
    pub blocked_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TagRequest {
    Create {
        name: String,
        oid: String,
        annotation: Option<String>,
    },
    Push {
        name: String,
        oid: String,
        remote: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagPreview {
    pub request: TagRequest,
    pub object_oid: Option<String>,
    pub commit_oid: String,
    pub annotation: Option<String>,
    pub tagger: Option<String>,
    pub destination: Option<String>,
    pub remote_oid: Option<String>,
    pub already_published: bool,
    pub token: String,
    pub blocked_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "preview", rename_all = "camelCase")]
pub enum PlannedOperation {
    SubmoduleSync(Box<SubmoduleSyncPreview>),
    WorktreeMaintenance(WorktreeMaintenancePreview),
    RepositoryCreate(Box<RepositoryCreatePreview>),
    StageAll(StageAllPreview),
    AddRemote(RemoteAddPreview),
    Amend(AmendPreview),
    RemoteDelete(RemoteDeletePreview),
    Hosting(Box<HostingPreview>),
    Tag(TagPreview),
    Ref(RefOperationPreview),
    Push(PushPreview),
    Merge(MergeReceipt),
    Stash(StashPreview),
    WorktreeCreate(WorktreeCreatePreview),
    WorktreeRemove(WorktreeRemovePreview),
    Discard(DiscardPreview),
}

pub fn default_change_operation() -> String {
    "merge".into()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MergeMethod {
    #[default]
    Auto,
    MergeCommit,
    FastForwardOnly,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequest {
    #[serde(default)]
    pub acknowledge_rewrite: bool,
    #[serde(default)]
    pub delete_source_branch: bool,
    #[serde(default)]
    pub method: MergeMethod,
    #[serde(default = "default_change_operation")]
    pub operation: String,
    #[serde(default)]
    pub mainline: Option<u32>,
    pub source_oid: String,
    pub source_ref: Option<String>,
    #[serde(default)]
    pub session_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebasePlan {
    pub base_oid: String,
    pub commits: Vec<String>,
    pub referenced_by: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeSubmoduleChange {
    pub path: NativePath,
    pub current_oid: Option<String>,
    pub incoming_oid: Option<String>,
    pub result_oid: Option<String>,
    pub conflicted: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeStartPreview {
    #[serde(default)]
    pub rebase: Option<RebasePlan>,
    #[serde(default)]
    pub preserved_submodules: Vec<PreservedSubmodule>,
    pub parents: Vec<String>,
    #[serde(default)]
    pub submodules: Vec<MergeSubmoduleChange>,
    pub suggested_message: String,
    pub request: MergeRequest,
    pub branch: String,
    pub original_oid: String,
    pub result_tree: Option<String>,
    pub mode: String,
    pub files: Vec<BranchFileChange>,
    pub conflicts: Vec<NativePath>,
    pub recovery_ref: String,
    pub token: String,
    pub blocked_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeStage {
    pub stage: u8,
    pub mode: String,
    pub oid: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeConflict {
    pub path: NativePath,
    pub stages: Vec<MergeStage>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeState {
    #[serde(default)]
    pub delete_source_branch: bool,
    #[serde(default)]
    pub source_ref: Option<String>,
    #[serde(default)]
    pub mainline: Option<u32>,
    #[serde(default = "default_change_operation")]
    pub operation: String,
    #[serde(default)]
    pub suggested_message: Option<String>,
    pub phase: String,
    pub session_id: Option<String>,
    pub branch: Option<String>,
    pub original_oid: Option<String>,
    pub source_oid: Option<String>,
    pub recovery_ref: Option<String>,
    pub conflicts: Vec<MergeConflict>,
    pub token: String,
    pub blocked_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeAction {
    pub kind: String,
    pub path: Option<NativePath>,
    pub choice: Option<String>,
    pub content: Option<String>,
    #[serde(default)]
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeActionPreview {
    #[serde(default)]
    pub truncated: bool,
    pub action: MergeAction,
    pub state: MergeState,
    pub diff: String,
    pub paths: Vec<NativePath>,
    pub recovery_dir: Option<NativePath>,
    pub expected_tree: Option<String>,
    pub token: String,
    pub blocked_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeFile {
    #[serde(default)]
    pub truncated: bool,
    pub path: NativePath,
    pub diff: String,
    pub working_text: Option<String>,
    pub ours_text: Option<String>,
    #[serde(default)]
    pub theirs_text: Option<String>,
    pub editable: bool,
    pub reason: Option<String>,
    pub token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeReceipt {
    #[serde(default = "default_change_operation")]
    pub operation: String,
    #[serde(default)]
    pub mainline: Option<u32>,
    pub kind: String,
    pub session_id: String,
    pub branch: String,
    pub original_oid: String,
    pub source_oid: String,
    pub path: Option<NativePath>,
    pub choice: Option<String>,
    pub expected_tree: Option<String>,
    pub recovery_ref: Option<String>,
    pub recovery_dir: Option<NativePath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSearchRequest {
    pub workspace_id: String,
    pub current_membership_id: String,
    pub scope: String,
    pub selected_ids: Vec<String>,
    pub query: String,
    pub cursor: Option<SearchCursor>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCursor {
    pub offset: usize,
    pub token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchLocation {
    pub membership_id: String,
    pub repository_id: String,
    pub worktree_id: Option<String>,
    pub name: String,
    pub path: NativePath,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSearchHit {
    pub commit: Commit,
    pub excerpt: String,
    pub locations: Vec<SearchLocation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCoverage {
    pub repository_id: String,
    pub membership_ids: Vec<String>,
    pub scanned: usize,
    pub indexed: usize,
    pub limited: bool,
    pub issue: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSearchPage {
    pub hits: Vec<CommitSearchHit>,
    pub coverage: Vec<SearchCoverage>,
    pub partial: bool,
    pub total_matches: usize,
    pub next: Option<SearchCursor>,
    pub token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphFocus {
    pub page: GraphPage,
    pub base: usize,
    pub selected_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    pub base_oid: Option<String>,
    pub index: usize,
    pub oid: String,
    pub message: String,
    pub timestamp: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashList {
    pub entries: Vec<StashEntry>,
    pub more: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StashKind {
    Push,
    Apply,
    Pop,
    Drop,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashRequest {
    pub kind: StashKind,
    pub index: Option<usize>,
    pub oid: Option<String>,
    pub message: String,
    pub include_untracked: bool,
    pub restore_index: bool,
    pub id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StashPreview {
    pub version: u8,
    pub request: StashRequest,
    pub stash_snapshot: Option<String>,
    pub files: Vec<NativePath>,
    pub diff: String,
    pub index_diff: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub recovery_ref: String,
    pub recovery_dir: NativePath,
    pub blocked_reason: Option<String>,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorktreeCreateMode {
    NewBranch,
    ExistingBranch,
    Detached,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeCreateRequest {
    pub oid: String,
    pub destination: String,
    pub mode: WorktreeCreateMode,
    pub branch: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeCreatePreview {
    pub version: u8,
    pub request: WorktreeCreateRequest,
    pub destination: NativePath,
    pub source: NativePath,
    pub files: usize,
    pub submodules: usize,
    pub blocked_reason: Option<String>,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemoveRequest {
    pub path: NativePath,
    pub id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeMembershipImpact {
    pub workspace_id: String,
    pub workspace_name: String,
    pub membership_id: String,
    pub name: String,
    pub worktree_id: String,
    pub selected: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemovePreview {
    pub worktree_id: Option<String>,
    pub version: u8,
    pub request: WorktreeRemoveRequest,
    pub source: NativePath,
    pub git_dir: Option<NativePath>,
    pub common_dir: NativePath,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub recovery_ref: String,
    pub memberships: Vec<WorktreeMembershipImpact>,
    pub git_token: String,
    pub token: String,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiscardScope {
    Unstaged,
    All,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscardRequest {
    pub paths: Vec<NativePath>,
    pub scope: DiscardScope,
    pub id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscardPreview {
    pub version: u8,
    pub source: NativePath,
    pub git_dir: NativePath,
    pub request: DiscardRequest,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub files: Vec<ChangedFile>,
    pub recovery_dir: NativePath,
    pub token: String,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedOperation {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub summary: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub current_checkout: bool,
    pub source: Option<NativePath>,
    pub recovery_dir: Option<NativePath>,
    pub recovery_ref: Option<String>,
    pub recovery: Option<String>,
    pub head_after: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedActivity {
    pub entries: Vec<RecordedOperation>,
    pub incomplete: bool,
    pub directory: NativePath,
}

/// Phase-local measurements; not an overall completion estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkProgress {
    pub phase: String,
    pub completed: u64,
    pub total: u64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRequest {
    pub workspace_id: String,
    pub source: String,
    pub destination: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClonePreview {
    pub version: u32,
    pub request: CloneRequest,
    pub destination: NativePath,
    pub token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneOutcome {
    pub id: String,
    pub workspace_id: String,
    pub source: String,
    pub destination: NativePath,
    pub status: String,
    pub summary: String,
    pub recovery: Option<String>,
    pub journal: NativePath,
    pub catalog: Option<Catalog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostingSelection {
    pub remote: String,
    pub branch: String,
    #[serde(default)]
    pub remote_branch: Option<String>,
    pub oid: String,
    pub provider: Option<String>,
    pub origin: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostingContext {
    pub selection: HostingSelection,
    pub provider: String,
    pub origin: String,
    pub project: String,
    pub account: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostingRequest {
    pub selection: HostingSelection,
    pub target_project: String,
    pub target_branch: String,
    pub title: String,
    pub body: String,
    pub draft: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedRequest {
    pub id: String,
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub draft: bool,
    pub url: String,
    pub source_branch: String,
    pub target_branch: String,
    pub source_oid: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostingPreview {
    pub request: HostingRequest,
    pub context: HostingContext,
    pub source_project_id: String,
    pub target_project_id: String,
    pub source_oid: Option<String>,
    pub target_oid: Option<String>,
    pub existing: Option<HostedRequest>,
    pub needs_push: bool,
    pub blocked_reason: Option<String>,
    pub token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostingOutcome {
    pub receipt: OperationReceipt,
    pub change_request: Option<HostedRequest>,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialPrompt {
    pub message: String,
    pub confirm: String,
    pub cancel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDeleteRequest {
    pub remote: String,
    pub branch: String,
    pub oid: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDeletePreview {
    pub request: RemoteDeleteRequest,
    pub destination: String,
    pub recovery_ref: String,
    pub tracking_ref: Option<String>,
    pub tracking_oid: Option<String>,
    pub local_branches: Vec<String>,
    pub token: String,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmendRequest {
    pub oid: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmendPreview {
    pub request: AmendRequest,
    pub commit: CommitPreview,
    pub original: Commit,
    pub original_message: String,
    pub original_signed: bool,
    pub expected_tree: String,
    pub empty_replacement: bool,
    pub referenced_by: Vec<String>,
    pub recovery_ref: String,
    pub token: String,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmendOriginal {
    pub commit: Commit,
    pub message: String,
    pub signed: bool,
}

// Read-only CI snapshots. Provider statuses stay distinct; no inferred percentage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineJob {
    pub id: String,
    pub name: String,
    pub stage: String,
    pub status: String,
    pub runner: Option<String>,
    pub duration: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRun {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_number: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub sha: String,
    pub reference: String,
    pub source: String,
    pub status: String,
    pub url: String,
    pub jobs: Vec<PipelineJob>,
    pub jobs_partial: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineSnapshot {
    pub oid: String,
    pub project: String,
    pub origin: String,
    pub runs: Vec<PipelineRun>,
    pub partial: bool,
    pub checked_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JenkinsSelection {
    pub remote: String,
    pub oid: String,
    pub controller: String,
    pub job_url: String,
    pub username: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JenkinsSnapshot {
    pub pipeline: PipelineSnapshot,
    pub scanned: usize,
    pub unlinked: usize,
    pub more_builds: bool,
    pub active_unlinked: bool,
}

/// A device-wide Git hosting account scope, independent of repository/commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostProfile {
    pub provider: String,
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreservedSubmodule {
    pub path: NativePath,
    pub recorded_oid: String,
    pub checkout_oid: String,
    /// Binds child identity, HEAD, branch and index without exposing raw data.
    pub state: String,
}

/// Separate post-merge local ref cleanup; merge success is never rolled back.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeBranchCleanup {
    pub source_ref: String,
    pub source_oid: String,
    pub status: String,
    pub message: String,
    pub receipt_id: Option<String>,
    pub recovery_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageAllPreview {
    pub files: Vec<ChangedFile>,
    pub excluded: Vec<ChangedFile>,
    pub token: String,
    pub blocked_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAddRequest {
    pub name: String,
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAddPreview {
    pub request: RemoteAddRequest,
    pub token: String,
    pub recovery_path: NativePath,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryNamespace {
    pub id: String,
    pub path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCreateOptions {
    pub account: String,
    pub namespaces: Vec<RepositoryNamespace>,
    pub partial: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCreateRequest {
    pub profile: HostProfile,
    pub namespace_id: String,
    pub name: String,
    pub description: String,
    pub visibility: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedRepository {
    pub project: String,
    pub clone_url: String,
    pub web_url: String,
    pub visibility: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCreatePreview {
    pub request: RepositoryCreateRequest,
    pub account: String,
    pub project: String,
    pub clone_url: String,
    pub token: String,
    #[serde(default)]
    pub created: Option<CreatedRepository>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCreateOutcome {
    pub receipt: OperationReceipt,
    pub repository: Option<CreatedRepository>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeMaintenanceRequest {
    pub path: NativePath,
    pub kind: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeMaintenancePreview {
    pub request: WorktreeMaintenanceRequest,
    pub tree: LinkedWorktree,
    pub affected: Vec<LinkedWorktree>,
    pub token: String,
    pub blocked_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleSyncPreview {
    pub submodule: Submodule,
    pub previous_url: Option<String>,
    pub child_previous_url: Option<String>,
    pub child_remote: Option<String>,
    pub url: String,
    pub recovery_dir: NativePath,
    pub token: String,
    pub blocked_reason: Option<String>,
}
