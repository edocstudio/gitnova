// IPC DTOs mirror gitnova-domain. Do not expose adapter or operating-system types.
export interface AppError { code: string; message: string }
export interface NativePath { display: string; encoding: string; hex: string }
export interface Repository { id: string; commonDir: NativePath; objectFormat: string; bare: boolean }
export interface Worktree { id: string; repositoryId: string; gitDir: NativePath; root: NativePath }
export interface Membership { id: string; repositoryId: string; worktreeId: string | null; name: string; pinned: boolean }
export interface Workspace { id: string; name: string; memberships: Membership[]; selectedId: string | null }
export interface Catalog { version: number; repositories: Repository[]; worktrees: Worktree[]; workspaces: Workspace[]; activeWorkspaceId: string }
export interface RepoStatus { branch: string | null; head: string | null; dirty: boolean; changedEntries: number; bare: boolean }
export interface GitRef { name: string; oid: string }
export interface BranchTracking { branch: string; oid: string; upstream: string | null; remote: string | null; remoteBranch: string | null; pushRemote: string | null; pushBranch: string | null; ahead: number | null; behind: number | null; state: 'tracked' | 'noUpstream' | 'gone' | 'shallow' }
export interface Commit { oid: string; parents: string[]; author: string; email: string; timestamp: number; subject: string }
export interface Edge { from: number; to: number; color: number }
export interface LaneStroke { lane: number; color: number }
export interface GraphLane { oid: string; color: number; active: boolean }
export interface GraphRow { commit: Commit; lane: number; color: number; incoming: LaneStroke[]; edges: Edge[]; width: number }
export interface GraphCursor { version: number; offset: number; lanes: (GraphLane | null)[]; nextColor: number; fingerprint: string }
export interface GraphPage { tracking?: BranchTracking[]; rows: GraphRow[]; refs: GitRef[]; status: RepoStatus; next: GraphCursor | null; fingerprint: string }
export interface CommitFile { path: NativePath; additions: number | null; deletions: number | null }
export interface CommitDetail { files: CommitFile[]; commit: Commit; message: string; diff: string; truncated: boolean; comparison: string }
export interface LinkedWorktree {missing?:boolean; path: NativePath; head: string | null; branch: string | null; bare: boolean; locked: boolean; prunable: boolean }
export interface Submodule { relationId: string; parentRelationId: string | null; parentRepositoryId: string; parentWorktreeId: string | null; repositoryId: string | null; worktreeId: string | null; parentPath: NativePath; relativePath: NativePath; depth: number; configName: string | null; inspectionError: string | null; detached: boolean | null; path: NativePath; headOid: string | null; indexOid: string; checkedOutOid: string | null; initialized: boolean; dirty: boolean | null }
export interface ChangedFile { path: NativePath; indexStatus: string; worktreeStatus: string; staged: boolean; unstaged: boolean; blockedReason: string | null }
export interface WorkingChanges { files: ChangedFile[]; status: RepoStatus; token: string }
export interface HunkPreview {file:FilePreview;headers:string[];token:string}
export interface FilePreview { path: NativePath; staged: boolean; token: string; diff: string; truncated: boolean }
export interface CommitPreview { completeManifest?:boolean; token: string; files: ChangedFile[]; status: RepoStatus; diff: string; truncated: boolean }
export interface MergeBranchCleanup {sourceRef:string;sourceOid:string;status:string;message:string;receiptId:string|null;recoveryRef:string|null}
export interface OperationReceipt { branchCleanup?:MergeBranchCleanup|null; merge?: MergeState | null; planned: PlannedOperation | null; graphAction: GraphAction | null; submodule: { relationId: string; path: NativePath; parentRepositoryId: string; repositoryId: string | null; headBefore: string | null; targetOid: string; headAfter: string | null } | null; repositoryId: string; worktreeId: string | null; membershipId: string; startedAt: number; finishedAt: number | null; id: string; kind: string; status: string; summary: string; headBefore: string | null; headAfter: string | null; recovery: string | null }
export interface RepositorySnapshot { tracking?: BranchTracking[]; merge: MergeState; fingerprint: string; changes: WorkingChanges }
export interface LocalBranch { name: string; oid: string; current: boolean; checkedOutAt: NativePath | null }
export interface BranchFileChange { path: NativePath; status: string }
export interface BranchSwitchPreview { branch: string; sourceBranch: string | null; sourceOid: string | null; targetOid: string; files: BranchFileChange[]; submodules: BranchSubmoduleChange[]; token: string; blockedReason: string | null }

export interface BranchSubmoduleChange { path: NativePath; sourceOid: string; targetOid: string; checkedOutOid: string | null; dirty: boolean | null }
export interface RemoteBranch { remote: string; name: string; oid: string }
export interface RemoteBranches { urls?: Record<string,string[]>; remotes: string[]; branches: RemoteBranch[] }
export interface RemoteSwitchPreview { remote: string; remoteBranch: string; remoteOid: string; create: boolean; localOid: string | null; ahead: number; behind: number; switch: BranchSwitchPreview }

export interface SubmoduleUpdatePreview { targetOid: string; remoteRef: string | null; changedPaths: NativePath[] | null; submodule: Submodule; mode: 'initialize' | 'restore' | 'remote'; url: string | null; sourceBase: string | null; parentStatus: RepoStatus; preservedBy: string[]; networkRequired: boolean; blockedReason: string | null; token: string }

export interface GraphAction { kind: 'checkoutCommit' | 'createBranch' | 'createTag'; oid: string; name: string }
export interface GraphActionPreview { action: GraphAction; switch: BranchSwitchPreview | null; token: string; blockedReason: string | null }

export interface RefOperation { kind: 'renameBranch' | 'deleteBranch' | 'deleteTag' | 'setUpstream' | 'unsetUpstream'; name: string; oid: string; newName: string; remote: string; remoteBranch: string }
export interface RefOperationPreview { action: RefOperation; objectOid: string; recoveryRef: string | null; annotation: string | null; upstreamBefore: string | null; upstreamAfter: string | null; retainedBy: string[]; token: string; blockedReason: string | null }
export interface PushRequest { branch: string; oid: string; remote: string; remoteBranch: string; setUpstream: boolean; bufferedHttps?: boolean }
export interface PushPreview { request: PushRequest; destination: string; remoteOid: string | null; outgoing: Commit[]; outgoingCount: number; trackingRef: string | null; trackingOid: string | null; upstreamBefore: string | null; token: string; blockedReason: string | null }
export type PlannedOperation = {type:'submoduleSync';preview:SubmoduleSyncPreview} | {type:'worktreeMaintenance';preview:WorktreeMaintenancePreview} | {type:'repositoryCreate';preview:RepositoryCreatePreview} | {type:'stageAll';preview:StageAllPreview} | {type:'addRemote';preview:RemoteAddPreview} | {type:'amend';preview:AmendPreview} | {type:'remoteDelete';preview:RemoteDeletePreview} | {type:'hosting';preview:HostingPreview} | {type:'tag';preview:TagPreview} | { type: 'discard'; preview: DiscardPreview } | { type: 'worktreeRemove'; preview: WorktreeRemovePreview } | { type: 'worktreeCreate'; preview: WorktreeCreatePreview } | { type: 'ref'; preview: RefOperationPreview } | { type: 'push'; preview: PushPreview } | { type: 'merge'; preview: MergeReceipt } | { type: 'stash'; preview: StashPreview };

export type ChangeOperation = 'merge' | 'cherryPick' | 'revert' | 'rebase';
export type MergeMethod = 'auto' | 'mergeCommit' | 'fastForwardOnly';
export interface RebaseEdit {oid:string;action:'pick'|'drop'|'fixup'}
export interface MergeRequest { rebaseSteps?:RebaseEdit[]; acknowledgeRewrite?:boolean; deleteSourceBranch?:boolean; method?: MergeMethod; operation: ChangeOperation; mainline: number | null; sourceOid: string; sourceRef: string | null; sessionId: string }
export interface MergeSubmoduleChange {path:NativePath;currentOid:string|null;incomingOid:string|null;resultOid:string|null;conflicted:boolean}
export interface PreservedSubmodule {path:NativePath;recordedOid:string;checkoutOid:string;state:string}
export interface MergeStartPreview { rebase?:{subjects?:Record<string,string>;preserveMerges?:boolean;root?:boolean;baseOid:string;commits:string[];referencedBy:string[]}|null; preservedSubmodules?:PreservedSubmodule[]; submodules?:MergeSubmoduleChange[]; parents: string[]; suggestedMessage: string; request: MergeRequest; branch: string; originalOid: string; resultTree: string | null; mode: string; files: BranchFileChange[]; conflicts: NativePath[]; recoveryRef: string; token: string; blockedReason: string | null }
export interface MergeStage { stage: number; mode: string; oid: string }
export interface MergeConflict { path: NativePath; stages: MergeStage[] }
export interface MergeState { deleteSourceBranch?:boolean;sourceRef?:string|null; operation: ChangeOperation; mainline: number | null; suggestedMessage: string | null; phase: string; sessionId: string | null; branch: string | null; originalOid: string | null; sourceOid: string | null; recoveryRef: string | null; conflicts: MergeConflict[]; token: string; blockedReason: string | null }
export interface MergeAction { kind: 'resolve' | 'finish' | 'abort' | 'release' | 'unstage' | 'skip'; path: NativePath | null; choice: string | null; content: string | null; message: string }
export interface MergeActionPreview { truncated: boolean; action: MergeAction; state: MergeState; diff: string; paths: NativePath[]; recoveryDir: NativePath | null; expectedTree: string | null; token: string; blockedReason: string | null }
export interface MergeFile { truncated: boolean; path: NativePath; diff: string; workingText: string | null; oursText: string | null; theirsText?: string | null; editable: boolean; reason: string | null; token: string }
export interface MergeReceipt { operation: ChangeOperation; mainline: number | null; kind: string; sessionId: string; branch: string; originalOid: string; sourceOid: string; path: NativePath | null; choice: string | null; expectedTree: string | null; recoveryRef: string | null; recoveryDir: NativePath | null }

export interface SearchCursor { offset: number; token: string }
export interface CommitSearchRequest { workspaceId: string; currentMembershipId: string; scope: 'current' | 'selected' | 'workspace'; selectedIds: string[]; query: string; cursor: SearchCursor | null }
export interface SearchLocation { membershipId: string; repositoryId: string; worktreeId: string | null; name: string; path: NativePath }
export interface CommitSearchHit { historicalPath?:NativePath|null; commit: Commit; excerpt: string; locations: SearchLocation[] }
export interface SearchCoverage { repositoryId: string; membershipIds: string[]; scanned: number; indexed: number; limited: boolean; issue: string | null }
export interface CommitSearchPage { hits: CommitSearchHit[]; coverage: SearchCoverage[]; partial: boolean; totalMatches: number; next: SearchCursor | null; token: string }
export interface GraphFocus { page: GraphPage; base: number; selectedIndex: number }
export interface StashEntry { baseOid: string | null; index: number; oid: string; message: string; timestamp: number }
export interface StashList { entries: StashEntry[]; more: boolean }
export type StashKind = 'push' | 'apply' | 'pop' | 'drop';
export interface StashRequest { kind: StashKind; index: number | null; oid: string | null; message: string; includeUntracked: boolean; restoreIndex: boolean; id: string }
export interface StashPreview { truncated: boolean; indexTruncated: boolean; version: number; stashSnapshot: string | null; request: StashRequest; files: NativePath[]; diff: string; indexDiff: string; branch: string | null; head: string | null; recoveryRef: string; recoveryDir: NativePath; blockedReason: string | null; token: string }

export interface WorktreeCreateRequest { oid: string; destination: string; mode: 'newBranch' | 'existingBranch' | 'detached'; branch: string }
export interface WorktreeCreatePreview { version: number; request: WorktreeCreateRequest; destination: NativePath; source: NativePath; files: number; submodules: number; blockedReason: string | null; token: string }

export interface WorktreeRemoveRequest { path: NativePath; id: string }
export interface WorktreeMembershipImpact { workspaceId: string; workspaceName: string; membershipId: string; name: string; worktreeId: string; selected: boolean }
export interface WorktreeRemovePreview { version: number; request: WorktreeRemoveRequest; worktreeId: string | null; source: NativePath; gitDir: NativePath | null; commonDir: NativePath; head: string | null; branch: string | null; recoveryRef: string; memberships: WorktreeMembershipImpact[]; gitToken: string; token: string; blockedReason: string | null }

export interface DiscardRequest { paths: NativePath[]; scope: 'unstaged' | 'all'; id: string }
export interface DiscardPreview { version: number; source: NativePath; gitDir: NativePath; request: DiscardRequest; branch: string | null; head: string | null; files: ChangedFile[]; recoveryDir: NativePath; token: string; blockedReason: string | null }

export interface RecordedOperation { id: string; kind: string; status: string; summary: string; startedAt: number; finishedAt: number | null; currentCheckout: boolean; source: NativePath | null; recoveryDir: NativePath | null; recoveryRef: string | null; recovery: string | null; headAfter: string | null }
export interface RecordedActivity { entries: RecordedOperation[]; incomplete: boolean; directory: NativePath }

export interface WorkProgress { phase: string; completed: number; total: number; unit: 'files' | 'bytes' }

export interface CloneRequest { workspaceId: string; source: string; destination: string }
export interface ClonePreview { version: number; request: CloneRequest; destination: NativePath; token: string }
export interface CloneOutcome { id: string; workspaceId: string; source: string; destination: NativePath; status: 'pending' | 'failed' | 'cloned' | 'succeeded'; summary: string; recovery: string | null; journal: NativePath; catalog: Catalog | null }

export interface SubmodulePointerPreview { version: number; submodule: Submodule; stage: boolean; targetOid: string; parentStatus: RepoStatus; blockedReason: string | null; token: string }

export interface SubmoduleSnapshot { status: RepoStatus; submodules: Submodule[] }

export type TagRequest = {kind:'create';name:string;oid:string;annotation:string|null}|{kind:'push';name:string;oid:string;remote:string};
export interface TagPreview {request:TagRequest;objectOid:string|null;commitOid:string;annotation:string|null;tagger:string|null;destination:string|null;remoteOid:string|null;alreadyPublished:boolean;token:string;blockedReason:string|null}

export interface HostingSelection {remote:string;branch:string;remoteBranch?:string|null;oid:string;provider:string|null;origin:string|null}
export interface HostingContext {selection:HostingSelection;provider:string;origin:string;project:string;account:string|null}
export interface HostingRequest {selection:HostingSelection;targetProject:string;targetBranch:string;title:string;body:string;draft:boolean}
export interface HostedRequest {id:string;number:number;title:string;body:string;state:string;draft:boolean;url:string;sourceBranch:string;targetBranch:string;sourceOid:string|null}
export interface HostingPreview {request:HostingRequest;context:HostingContext;sourceProjectId:string;targetProjectId:string;sourceOid:string|null;targetOid:string|null;existing:HostedRequest|null;needsPush:boolean;blockedReason:string|null;token:string}
export interface HostingOutcome {receipt:OperationReceipt;changeRequest:HostedRequest|null;created:boolean}

export interface RemoteDeleteRequest { remote: string; branch: string; oid: string }
export interface RemoteDeletePreview { request: RemoteDeleteRequest; destination: string; recoveryRef: string; trackingRef: string | null; trackingOid: string | null; localBranches: string[]; token: string; blockedReason: string | null }

export interface AmendRequest {oid:string;message:string}
export interface AmendOriginal {commit:Commit;message:string;signed:boolean}
export interface AmendPreview {request:AmendRequest;commit:CommitPreview;original:Commit;originalMessage:string;originalSigned:boolean;expectedTree:string;emptyReplacement:boolean;referencedBy:string[];recoveryRef:string;token:string;blockedReason:string|null}

export interface PipelineJob {id:string;name:string;stage:string;status:string;runner:string|null;duration:number|null}
export interface PipelineRun {id:string;displayNumber?:number;name?:string;sha:string;reference:string;source:string;status:string;url:string;jobs:PipelineJob[];jobsPartial:boolean}
export interface PipelineSnapshot {oid:string;project:string;origin:string;runs:PipelineRun[];partial:boolean;checkedAt:number}

export interface JenkinsSelection {remote:string;oid:string;controller:string;jobUrl:string;username:string}
export interface JenkinsSnapshot {pipeline:PipelineSnapshot;scanned:number;unlinked:number;moreBuilds:boolean;activeUnlinked:boolean}

export interface HostProfile {provider:'gitlab'|'github';origin:string}

export interface StageAllPreview {files:ChangedFile[];excluded:ChangedFile[];token:string;blockedReason:string|null}
export interface RemoteAddRequest {name:string;url:string}
export interface RemoteAddPreview {request:RemoteAddRequest;token:string;recoveryPath:NativePath}

export interface RepositoryCreateRequest {profile:HostProfile;namespaceId:string;name:string;description:string;visibility:'public'|'private'}
export interface RepositoryCreateOptions {account:string;namespaces:{id:string;path:string}[];partial:boolean}
export interface CreatedRepository {project:string;cloneUrl:string;webUrl:string;visibility:string}
export interface RepositoryCreatePreview {request:RepositoryCreateRequest;account:string;project:string;cloneUrl:string;token:string;created?:CreatedRepository|null}
export interface RepositoryCreateOutcome {receipt:OperationReceipt;repository:CreatedRepository|null}

export interface WorktreeMaintenanceRequest {destination?:string;path:NativePath;kind:'lock'|'unlock'|'repair'|'reattach'|'prune'|'move';reason:string}
export interface WorktreeMaintenancePreview {recoveryDir?:NativePath|null;request:WorktreeMaintenanceRequest;tree:LinkedWorktree;affected:LinkedWorktree[];token:string;blockedReason:string|null}
export interface SubmoduleSyncPreview {submodule:Submodule;previousUrl:string|null;childPreviousUrl:string|null;childRemote:string|null;url:string;recoveryDir:NativePath;token:string;blockedReason:string|null}

export type InitRequest = {workspaceId: string; destination: string; branch: string};
export type InitPreview = {request: InitRequest; destination: NativePath; existingFolder: boolean; token: string};

export interface InvestigationRequest { selection: CommitSearchRequest; mode: 'history' | 'follow' | 'blame' | 'compare'; lineStart?:number; lineEnd?:number; path: string; needle: string; source: string; target: string }
export interface DependencyEvidence {path:NativePath;sourceOid:string|null;targetOid:string|null;location:SearchLocation|null;relation:string;sourceOnly:number|null;targetOnly:number|null;cachedRemoteContains:boolean|null;issue:string|null}
export interface LineAttribution {oid:string;path:NativePath;line:number;originalLine:number;content:string}
export interface PackageLocationEvidence {location:SearchLocation;oid:string;version:string|null}
export interface PackageRequirementEvidence {name:string;kind:string;source:string|null;target:string|null;locations:PackageLocationEvidence[];issue:string|null}
export interface PackageManifestEvidence {sourceName:string|null;targetName:string|null;sourceVersion:string|null;targetVersion:string|null;requirements:PackageRequirementEvidence[];issue:string|null}
export interface InvestigationEvidence {packageManifest?:PackageManifestEvidence|null; lineAttribution?:LineAttribution[]; dependencies?:DependencyEvidence[]; location: SearchLocation; sourceOid: string | null; targetOid: string | null; sourceOnly: number | null; targetOnly: number | null; changedFiles: number | null; dirty: boolean | null; scanned: number; limited: boolean; shallow: boolean; issue: string | null }
export interface InvestigationPage { hits: CommitSearchHit[]; evidence: InvestigationEvidence[]; partial: boolean }

export interface GitCommitIdentity {name:string;email:string}
export interface CommitIdentityContext {identity:GitCommitIdentity;connection:HostProfile|null}

export interface SubmoduleLifecycleRequest {kind:'add'|'remove'|'stage'|'unstage';path:string;source:string;id:string}
export interface SubmoduleLifecyclePreview { manifestBefore?:string|null; manifestAfter?:string|null;request:SubmoduleLifecycleRequest;branch:string|null;childHead:string|null;download:boolean;recoveryDir:NativePath;token:string;blockedReason:string|null}

export interface StaleLockPreview {path:NativePath;ownerPid:number|null;exists:boolean;token:string;blockedReason:string|null}
