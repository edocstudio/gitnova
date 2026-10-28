import type {RepositoryCreateRequest,RepositoryCreateOptions,RepositoryCreatePreview,RepositoryCreateOutcome} from './contracts';
import type {StageAllPreview,RemoteAddRequest,RemoteAddPreview} from './contracts';
import type { JenkinsSelection, JenkinsSnapshot } from './contracts';
import type { HostProfile, PipelineSnapshot } from './contracts';
import type { AmendRequest, AmendPreview, AmendOriginal } from './contracts';
import type { RemoteDeleteRequest, RemoteDeletePreview } from './contracts';
import { t } from './i18n';
import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import type { HostingSelection, HostingContext, HostingRequest, HostingPreview, HostingOutcome, TagRequest, TagPreview, SubmoduleSnapshot, SubmodulePointerPreview, CloneRequest, ClonePreview, CloneOutcome, BranchTracking, WorkProgress, RecordedActivity, DiscardRequest, DiscardPreview, WorktreeRemoveRequest, WorktreeRemovePreview, WorktreeCreateRequest, WorktreeCreatePreview, StashList, StashPreview, StashRequest, CommitSearchRequest, CommitSearchPage, GraphFocus, MergeRequest, MergeStartPreview, MergeState, MergeAction, MergeActionPreview, MergeFile, RefOperation, RefOperationPreview, PushRequest, PushPreview, GraphAction, GraphActionPreview, SubmoduleUpdatePreview, RemoteBranches, RemoteSwitchPreview, LocalBranch, BranchSwitchPreview, WorkingChanges, FilePreview, CommitPreview, OperationReceipt, RepositorySnapshot, Catalog, CommitDetail, GraphCursor, GraphPage, LinkedWorktree, NativePath, RepoStatus, Submodule } from './contracts';
export const desktop = isTauri();
function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!desktop) return Promise.reject({ code: 'desktop_required', message: 'Open GitNova with npm run tauri dev to read local repositories. The browser preview has no filesystem access.' });
  return invoke<T>(command, args);
}
function callProgress<T>(command:string,args:Record<string,unknown>,onProgress?: (event:WorkProgress)=>void):Promise<T>{
  if(!desktop || !onProgress)return call<T>(command,args);
  const channel=new Channel<WorkProgress>(onProgress);
  return call<T>(command,{...args,onProgress:channel}).finally(()=>{channel.onmessage=()=>{};});
}
export const api = {
  previewWorktreeMaintenance:(membershipId:string,request:import('./contracts').WorktreeMaintenanceRequest)=>call<import('./contracts').WorktreeMaintenancePreview>('preview_worktree_maintenance',{membershipId,request}),
  maintainWorktree:(membershipId:string,request:import('./contracts').WorktreeMaintenanceRequest,token:string)=>call<OperationReceipt>('maintain_worktree',{membershipId,request,token}),
  previewSubmoduleSync:(membershipId:string,path:NativePath)=>call<import('./contracts').SubmoduleSyncPreview>('preview_submodule_sync',{membershipId,path}),
  syncSubmodule:(membershipId:string,path:NativePath,token:string)=>call<OperationReceipt>('sync_submodule',{membershipId,path,token}),
  previewHunks:(membershipId:string,path:NativePath,staged:boolean)=>call<import('./contracts').HunkPreview>('preview_hunks',{membershipId,path,staged}),
  changeHunk:(membershipId:string,path:NativePath,staged:boolean,hunk:number,token:string)=>call<OperationReceipt>('change_hunk',{membershipId,path,staged,hunk,token}),
  previewStageAll:(membershipId:string)=>call<StageAllPreview>('preview_stage_all',{membershipId}),
  stageAll:(membershipId:string,token:string)=>call<OperationReceipt>('stage_all',{membershipId,token}),
  commitFilePreview:(membershipId:string,path:NativePath,token:string)=>call<FilePreview>('preview_commit_file',{membershipId,path,token}),
  previewRemoteAdd:(membershipId:string,request:RemoteAddRequest)=>call<RemoteAddPreview>('preview_remote_add',{membershipId,request}),
  addRemote:(membershipId:string,request:RemoteAddRequest,token:string)=>call<OperationReceipt>('add_remote',{membershipId,request,token}),
  amendOriginal: (membershipId:string,oid:string)=>call<AmendOriginal>('amend_original',{membershipId,oid}),
  previewAmend: (membershipId:string,request:AmendRequest)=>call<AmendPreview>('preview_amend',{membershipId,request}),
  amendCommit: (membershipId:string,request:AmendRequest,token:string,acknowledgeRewrite:boolean)=>call<OperationReceipt>('amend_commit',{membershipId,request,token,acknowledgeRewrite}),
  previewRemoteDelete: (membershipId: string, request: RemoteDeleteRequest) => call<RemoteDeletePreview>('preview_remote_delete', { membershipId, request }),
  deleteRemoteBranch: (membershipId: string, request: RemoteDeleteRequest, token: string) => call<OperationReceipt>('delete_remote_branch', { membershipId, request, token }),
  previewClone: (request: CloneRequest) => call<ClonePreview>('preview_clone', { request }),
  cloneRepository: (request: CloneRequest, token: string, onProgress?: (event: WorkProgress) => void) => callProgress<CloneOutcome>('clone_repository', { request, token }, onProgress),
  recentClones: (workspaceId: string) => call<CloneOutcome[]>('recent_clones', { workspaceId }),
  recordedActivity: (membershipId: string) => call<RecordedActivity>('recorded_activity', { membershipId }),
  previewDiscard: (membershipId: string, request: DiscardRequest, operationToken?: string, onProgress?: (event:WorkProgress)=>void) => callProgress<DiscardPreview>(operationToken ? 'preview_operation_discard' : 'preview_discard', { membershipId, request, ...(operationToken ? {operationToken} : {}) },onProgress).then(preview => { if (preview.version !== 1) throw new Error('Restart GitNova to load discard support. Nothing has been applied.'); return preview; }),
  discardChanges: (membershipId: string, request: DiscardRequest, token: string, operationToken?: string, onProgress?: (event:WorkProgress)=>void) => callProgress<OperationReceipt>(operationToken ? 'discard_operation_changes' : 'discard_changes', { membershipId, request, token, ...(operationToken ? {operationToken} : {}) },onProgress),
  previewRemoveWorktree: (membershipId: string, request: WorktreeRemoveRequest) => call<WorktreeRemovePreview>('preview_remove_worktree', { membershipId, request }).then(preview => { if (preview.version !== 1) throw new Error('Restart GitNova to load the native backend for worktree removal. Nothing has been applied.'); return preview; }),
  removeWorktree: (membershipId: string, request: WorktreeRemoveRequest, token: string) => call<OperationReceipt>('remove_worktree', { membershipId, request, token }),
  previewCreateWorktree: (membershipId: string, request: WorktreeCreateRequest) => call<WorktreeCreatePreview>('preview_create_worktree', { membershipId, request }).then(preview => { if (preview.version !== 1) throw new Error('Restart GitNova to load the native backend for worktree creation. Nothing has been applied.'); return preview; }),
  createWorktree: (membershipId: string, request: WorktreeCreateRequest, token: string) => call<OperationReceipt>('create_worktree', { membershipId, request, token }),
  stashes: (membershipId: string) => call<StashList>('stashes', { membershipId }),
  previewStash: (membershipId: string, request: StashRequest) => call<StashPreview>('preview_stash', { membershipId, request }).then(preview => { if (preview.version !== 1) throw new Error('Restart GitNova to load the native backend for this stash workflow. Nothing has been applied.'); return preview; }),
  applyStash: (membershipId: string, request: StashRequest, token: string) => call<OperationReceipt>('apply_stash', { membershipId, request, token }),
  historicalPatch: (membershipId: string, oid: string, path: NativePath) => call<{diff: string; truncated: boolean}>('historical_patch', { membershipId, oid, path }),
  searchCommits: (request: CommitSearchRequest) => call<CommitSearchPage>('search_commits', { request }),
  focusCommit: (workspaceId: string, membershipId: string, oid: string) => call<GraphFocus>('focus_commit', { workspaceId, membershipId, oid }),
  mergeState: (membershipId: string) => call<MergeState>('merge_state', { membershipId }),
  previewMerge: (membershipId: string, request: MergeRequest) => call<MergeStartPreview>('preview_merge', { membershipId, request }),
  beginMerge: (membershipId: string, request: MergeRequest, token: string) => call<OperationReceipt>('begin_merge', { membershipId, request, token }),
  mergeFile: (membershipId: string, path: NativePath, staged = true) => call<MergeFile>('merge_file', { membershipId, path, staged }),
  previewMergeAction: (membershipId: string, action: MergeAction) => call<MergeActionPreview>('preview_merge_action', { membershipId, action }),
  applyMergeAction: (membershipId: string, action: MergeAction, token: string) => call<OperationReceipt>('apply_merge_action', { membershipId, action, token }),
  previewRefOperation: (membershipId: string, action: RefOperation) => call<RefOperationPreview>('preview_ref_operation', { membershipId, action }),
  applyRefOperation: (membershipId: string, action: RefOperation, token: string) => call<OperationReceipt>('apply_ref_operation', { membershipId, action, token }),
  commitLink: (membershipId:string,selection:HostingSelection)=>call<string>('commit_link',{membershipId,selection}),
  openCiUrl: (url:string)=>call<void>('open_ci_url',{url}),
  connectJenkins: (membershipId:string,selection:JenkinsSelection)=>call<string>('connect_jenkins',{membershipId,selection,prompt:{message:t('Enter a Jenkins API token. It stays in macOS Keychain, separate from GitLab.'),confirm:t('Connect'),cancel:t('Cancel')}}),
  jenkinsBuilds: (membershipId:string,selection:JenkinsSelection,refresh=false)=>call<JenkinsSnapshot>('jenkins_builds',{membershipId,selection,refresh}),
  repositoryCreateOptions:(profile:HostProfile)=>call<RepositoryCreateOptions>('repository_create_options',{profile}),
  previewRepositoryCreate:(membershipId:string,request:RepositoryCreateRequest)=>call<RepositoryCreatePreview>('preview_repository_create',{membershipId,request}),
  createRepository:(membershipId:string,request:RepositoryCreateRequest,token:string)=>call<RepositoryCreateOutcome>('create_repository',{membershipId,request,token}),
  hostAccount: (profile:HostProfile)=>call<HostingContext>('host_account',{profile}),
  connectHost: (profile:HostProfile)=>call<HostingContext>('connect_host',{profile,prompt:{message:t('Enter a provider access token. The verified token will be stored in macOS Keychain.'),confirm:t('Connect'),cancel:t('Cancel')}}),
  disconnectHost: (profile:HostProfile)=>call<void>('disconnect_host',{profile}),
  ciConnection: (membershipId:string,oid:string,profiles:HostProfile[])=>call<HostingSelection|null>('ci_connection',{membershipId,oid,profiles}),
  jenkinsAccount: (controller:string,username:string)=>call<string|null>('jenkins_account',{controller,username}),
  connectJenkinsAccount: (controller:string,username:string)=>call<string>('connect_jenkins_account',{controller,username,prompt:{message:t('Enter a Jenkins API token. It stays in macOS Keychain.'),confirm:t('Connect'),cancel:t('Cancel')}}),
  disconnectJenkinsAccount: (controller:string,username:string)=>call<void>('disconnect_jenkins_account',{controller,username}),
  pipelineContext: (membershipId:string,selection:HostingSelection)=>call<HostingContext>('pipeline_context',{membershipId,selection}),
  connectPipeline: (membershipId:string,selection:HostingSelection)=>call<HostingContext>('connect_pipeline',{membershipId,selection,prompt:{message:t('Enter a provider access token. The verified token will be stored in macOS Keychain.'),confirm:t('Connect'),cancel:t('Cancel')}}),
  ciSummary: (membershipId:string,selection:HostingSelection)=>call<PipelineSnapshot>('ci_summary',{membershipId,selection}),
  pipelines: (membershipId:string,selection:HostingSelection,refresh=false)=>call<PipelineSnapshot>('pipelines',{membershipId,selection,refresh}),
  hostingContext: (membershipId:string,selection:HostingSelection)=>call<HostingContext>('hosting_context',{membershipId,selection}),
  disconnectHosting: (membershipId:string,selection:HostingSelection)=>call<HostingContext>('disconnect_hosting',{membershipId,selection}),
  connectHosting: (membershipId:string,selection:HostingSelection)=>call<HostingContext>('connect_hosting',{membershipId,selection,prompt:{message:t('Enter a provider access token. The verified token will be stored in macOS Keychain.'),confirm:t('Connect'),cancel:t('Cancel')}}),
  previewHosting: (membershipId:string,request:HostingRequest)=>call<HostingPreview>('preview_hosting',{membershipId,request}),
  createHosting: (membershipId:string,request:HostingRequest,token:string)=>call<HostingOutcome>('create_hosting',{membershipId,request,token}),
  previewTag: (membershipId:string,request:TagRequest)=>call<TagPreview>('preview_tag',{membershipId,request}),
  changeTag: (membershipId:string,request:TagRequest,token:string)=>call<OperationReceipt>('change_tag',{membershipId,request,token}),
  previewPush: (membershipId: string, request: PushRequest) => call<PushPreview>('preview_push', { membershipId, request }),
  pushBranch: (membershipId: string, request: PushRequest, token: string) => call<OperationReceipt>('push_branch', { membershipId, request, token }),
  previewGraphAction: (membershipId: string, action: GraphAction) => call<GraphActionPreview>('preview_graph_action', { membershipId, action }),
  applyGraphAction: (membershipId: string, action: GraphAction, token: string) => call<OperationReceipt>('apply_graph_action', { membershipId, action, token }),
  previewSubmodulePointer: (membershipId: string, path: NativePath, stage: boolean) => call<SubmodulePointerPreview>('preview_submodule_pointer', {membershipId, path, stage}),
  changeSubmodulePointer: (membershipId: string, path: NativePath, stage: boolean, token: string) => call<OperationReceipt>('change_submodule_pointer', {membershipId, path, stage, token}),
  previewSubmoduleUpdate: (membershipId: string, path: NativePath, mode: 'initialize' | 'restore') => call<SubmoduleUpdatePreview>('preview_submodule_update', { membershipId, path, mode }),
  updateSubmodule: (membershipId: string, path: NativePath, mode: 'initialize' | 'restore', token: string) => call<OperationReceipt>('update_submodule', { membershipId, path, mode, token }),
  remoteBranches: (membershipId: string) => call<RemoteBranches>('remote_branches', { membershipId }),
  fetchRemote: (membershipId: string, remote: string, branch: string | null = null) => call<OperationReceipt>('fetch_remote', { membershipId, remote, branch }),
  pullUpstream: (membershipId: string) => call<BranchTracking>('pull_upstream', { membershipId }),
  previewPullCurrent: (membershipId: string) => call<RemoteSwitchPreview>('preview_pull_current', { membershipId }),
  pullCurrent: (membershipId: string, token: string) => call<OperationReceipt>('pull_current', { membershipId, token }),
  previewRemoteSwitch: (membershipId: string, remote: string, branch: string) => call<RemoteSwitchPreview>('preview_remote_switch', { membershipId, remote, branch }),
  switchRemoteBranch: (membershipId: string, remote: string, branch: string, token: string) => call<OperationReceipt>('switch_remote_branch', { membershipId, remote, branch, token }),
  branches: (membershipId: string) => call<LocalBranch[]>('local_branches', { membershipId }),
  previewBranchSwitch: (membershipId: string, branch: string) => call<BranchSwitchPreview>('preview_branch_switch', { membershipId, branch }),
  switchBranch: (membershipId: string, branch: string, token: string) => call<OperationReceipt>('switch_branch', { membershipId, branch, token }),
  snapshot: (membershipId: string) => call<RepositorySnapshot>('repository_snapshot', { membershipId }),
  changes: (membershipId: string) => call<WorkingChanges>('working_changes', { membershipId }),
  filePreview: (membershipId: string, path: NativePath, staged: boolean) => call<FilePreview>('file_preview', { membershipId, path, staged }),
  commitPreview: (membershipId: string) => call<CommitPreview>('preview_commit', { membershipId }),
  changeIndex: (membershipId: string, path: NativePath, stage: boolean, token: string) => call<OperationReceipt>('change_index', { membershipId, path, stage, token }),
  commit: (membershipId: string, message: string, token: string) => call<OperationReceipt>('create_commit', { membershipId, message, token }),
  catalog: () => call<Catalog>('catalog'),
  add: (path: string) => call<Catalog>('add_repository', { path }),
  createWorkspace: (name: string) => call<Catalog>('create_workspace', { name }),
  switchWorkspace: (workspaceId: string) => call<Catalog>('switch_workspace', { workspaceId }),
  select: (membershipId: string) => call<Catalog>('select_repository', { membershipId }),
  pin: (membershipId: string) => call<Catalog>('pin_repository', { membershipId }),
  status: (membershipId: string) => call<RepoStatus>('repository_status', { membershipId }),
  graph: (membershipId: string, cursor: GraphCursor | null = null) => call<GraphPage>('graph_page', { membershipId, cursor }),
  detail: (membershipId: string, oid: string) => call<CommitDetail>('commit_detail', { membershipId, oid }),
  submoduleSnapshot: (membershipId: string) => call<SubmoduleSnapshot>('submodule_snapshot', { membershipId }),
  submodules: (membershipId: string) => call<Submodule[]>('submodules', { membershipId }),
  worktrees: (membershipId: string) => call<LinkedWorktree[]>('worktrees', { membershipId }),
  openRelated: (membershipId: string, path: NativePath) => call<Catalog>('open_related', { membershipId, path }),
};
export function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error) return String(error.message);
  return String(error);
}
