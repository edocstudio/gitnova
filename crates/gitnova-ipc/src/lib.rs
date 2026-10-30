//! Public command DTOs. No Tauri, gix or libgit2 types cross this boundary.
pub use gitnova_domain::{
    AppError, Catalog, CommitDetail, GraphCursor, GraphPage, LinkedWorktree, NativePath,
    RepoStatus, Submodule,
};

pub use gitnova_domain::{
    CommitPreview, FilePreview, OperationReceipt, RepositorySnapshot, WorkingChanges,
};

pub use gitnova_domain::{BranchSwitchPreview, BranchTracking, LocalBranch};

pub use gitnova_domain::{RemoteBranches, RemoteSwitchPreview};

pub use gitnova_domain::SubmoduleUpdatePreview;

pub use gitnova_domain::{GraphAction, GraphActionPreview};

pub use gitnova_domain::{PushPreview, PushRequest, RefOperation, RefOperationPreview};

pub use gitnova_domain::{
    MergeAction, MergeActionPreview, MergeFile, MergeRequest, MergeStartPreview, MergeState,
};

pub use gitnova_domain::{CommitSearchPage, CommitSearchRequest, GraphFocus};

pub use gitnova_domain::HistoricalPatch;

pub use gitnova_domain::{StashEntry, StashList, StashPreview, StashRequest};

pub use gitnova_domain::{WorktreeCreateMode, WorktreeCreatePreview, WorktreeCreateRequest};

pub use gitnova_domain::{WorktreeRemovePreview, WorktreeRemoveRequest};

pub use gitnova_domain::{DiscardPreview, DiscardRequest, DiscardScope};

pub use gitnova_domain::{RecordedActivity, RecordedOperation};

pub use gitnova_domain::WorkProgress;

pub use gitnova_domain::{CloneOutcome, ClonePreview, CloneRequest};

pub use gitnova_domain::SubmodulePointerPreview;
pub use gitnova_domain::SubmoduleSnapshot;

pub use gitnova_domain::{TagPreview, TagRequest};

pub use gitnova_domain::{
    HostingContext, HostingOutcome, HostingPreview, HostingRequest, HostingSelection,
};

pub use gitnova_domain::CredentialPrompt;

pub use gitnova_domain::{RemoteDeletePreview, RemoteDeleteRequest};

pub use gitnova_domain::{AmendPreview, AmendRequest};

pub use gitnova_domain::AmendOriginal;

pub use gitnova_domain::{PipelineJob, PipelineRun, PipelineSnapshot};

pub use gitnova_domain::{JenkinsSelection, JenkinsSnapshot};

pub use gitnova_domain::HostProfile;

pub use gitnova_domain::{RemoteAddPreview, RemoteAddRequest, StageAllPreview};

pub use gitnova_domain::{
    RepositoryCreateOptions, RepositoryCreateOutcome, RepositoryCreatePreview,
    RepositoryCreateRequest,
};

pub use gitnova_domain::{
    HunkPreview, RebasePlan, SubmoduleSyncPreview, WorktreeMaintenancePreview,
    WorktreeMaintenanceRequest,
};

pub use gitnova_domain::{InitPreview, InitRequest};

pub use gitnova_domain::{InvestigationEvidence, InvestigationPage, InvestigationRequest};

pub use gitnova_domain::{CommitIdentityContext, ConnectionCommitIdentity, GitCommitIdentity};

pub use gitnova_domain::{SubmoduleLifecyclePreview, SubmoduleLifecycleRequest};
