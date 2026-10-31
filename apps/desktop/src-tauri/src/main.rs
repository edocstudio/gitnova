#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod browser;
mod terminal;
use gitnova_core::Application;
use gitnova_ipc::*;
use std::{path::PathBuf, sync::Arc};
use tauri::Manager;
type State<'a> = tauri::State<'a, Arc<Application>>;
// Bound native command concurrency before allocating blocking workers. Keep
// permits inside the worker even if the invoking WebView disappears.
static COMMAND_SLOTS: std::sync::LazyLock<Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(2)));
static COMMAND_QUEUE: std::sync::LazyLock<Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(32)));
// Debug-only native smoke profile. It cannot fall back to the user's catalog.
fn native_smoke() -> bool {
    cfg!(debug_assertions)
        && std::env::var_os("GITNOVA_NATIVE_SMOKE").as_deref() == Some(std::ffi::OsStr::new("1"))
}
fn smoke_marker(marker: &str) {
    if native_smoke() {
        eprintln!("GITNOVA_NATIVE_SMOKE:{marker}");
    }
}
fn native_smoke_state() -> Result<Option<PathBuf>, AppError> {
    if !native_smoke() {
        return Ok(None);
    }
    let path = PathBuf::from(
        std::env::var_os("GITNOVA_SMOKE_STATE")
            .ok_or_else(|| AppError::new("smoke", "Missing isolated smoke state"))?,
    );
    let canonical = path.canonicalize()?;
    let temporary = std::env::temp_dir().canonicalize()?;
    let valid = path.is_absolute()
        && canonical.starts_with(&temporary)
        && canonical.file_name() == Some(std::ffi::OsStr::new("state.json"))
        && canonical
            .parent()
            .and_then(|p| p.file_name())
            .is_some_and(|name| name.to_string_lossy().starts_with("gitnova-native-smoke-"));
    if !valid {
        return Err(AppError::new(
            "smoke",
            "Smoke state must be inside its private temporary fixture",
        ));
    }
    Ok(Some(canonical))
}

// Fixture-only telemetry. Never enabled for a normal user catalog or release.
#[tauri::command]
fn native_soak_report(sample: serde_json::Value) -> Result<(), AppError> {
    use std::io::Write;
    if std::env::var_os("GITNOVA_NATIVE_SOAK_SECONDS").is_none() {
        return Err(AppError::new("soak", "Isolated soak profile required"));
    }
    let state = native_smoke_state()?
        .ok_or_else(|| AppError::new("soak", "Isolated smoke state required"))?;
    let bytes = serde_json::to_vec(&sample).map_err(|e| AppError::new("soak", e.to_string()))?;
    if bytes.len() > 16384 {
        return Err(AppError::new("soak", "Sample exceeds its telemetry budget"));
    }
    let file = state
        .parent()
        .ok_or_else(|| AppError::new("soak", "Missing fixture directory"))?
        .join("ui-soak.jsonl");
    if std::fs::metadata(&file).is_ok_and(|m| m.len() > 4 * 1024 * 1024) {
        return Err(AppError::new("soak", "Telemetry budget reached"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut out = options.open(file)?;
    out.write_all(&bytes)?;
    out.write_all(b"\n")?;
    Ok(())
}

async fn run<T: Send + 'static>(
    state: State<'_>,
    f: impl FnOnce(&Application) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let app = state.inner().clone();
    let queued = COMMAND_QUEUE.clone().try_acquire_owned().map_err(|_| {
        AppError::new(
            "busy",
            "GitNova is busy; wait for the current operation to finish.",
        )
    })?;
    let slot = COMMAND_SLOTS
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::new("state", "Native command queue is unavailable."))?;
    tauri::async_runtime::spawn_blocking(move || {
        let (_queued, _slot) = (queued, slot);
        f(&app)
    })
    .await
    .map_err(|e| AppError::new("worker", e.to_string()))?
}
async fn run_progress<T: Send + 'static>(
    state: State<'_>,
    on_progress: Option<tauri::ipc::Channel<WorkProgress>>,
    f: impl FnOnce(&Application) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    run(state, move |app| {
        let last = std::cell::RefCell::new(None::<(String, std::time::Instant)>);
        gitnova_core::with_progress(
            move |event| {
                let mut previous = last.borrow_mut();
                let send = event.completed == 0
                    || event.completed == event.total
                    || previous.as_ref().is_none_or(|(phase, time)| {
                        phase != &event.phase
                            || time.elapsed() >= std::time::Duration::from_millis(100)
                    });
                if send {
                    if let Some(channel) = &on_progress {
                        let _ = channel.send(event.clone());
                    }
                    *previous = Some((event.phase, std::time::Instant::now()));
                }
            },
            || f(app),
        )
    })
    .await
}
#[tauri::command]
async fn preview_stale_lock(
    state: State<'_>,
    membership_id: String,
) -> Result<StaleLockPreview, AppError> {
    run(state, move |a| a.preview_stale_lock(&membership_id)).await
}
#[tauri::command]
async fn recover_stale_lock(
    state: State<'_>,
    membership_id: String,
    token: String,
) -> Result<NativePath, AppError> {
    run(state, move |a| a.recover_stale_lock(&membership_id, &token)).await
}
#[tauri::command]
async fn recorded_activity(
    state: State<'_>,
    membership_id: String,
) -> Result<RecordedActivity, AppError> {
    run(state, move |a| a.recorded_activity(&membership_id)).await
}
#[tauri::command]
async fn preview_operation_discard<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    state: State<'_>,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
    membership_id: String,
    request: DiscardRequest,
    operation_token: String,
) -> Result<DiscardPreview, AppError> {
    run_progress(
        state,
        on_progress.map(|id| id.channel_on(webview)),
        move |a| a.preview_operation_discard(&membership_id, &request, &operation_token),
    )
    .await
}
#[tauri::command]
async fn discard_operation_changes<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    state: State<'_>,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
    membership_id: String,
    request: DiscardRequest,
    token: String,
    operation_token: String,
) -> Result<OperationReceipt, AppError> {
    run_progress(
        state,
        on_progress.map(|id| id.channel_on(webview)),
        move |a| a.discard_operation_changes(&membership_id, &request, &token, &operation_token),
    )
    .await
}
#[tauri::command]
async fn preview_discard<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    state: State<'_>,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
    membership_id: String,
    request: DiscardRequest,
) -> Result<DiscardPreview, AppError> {
    run_progress(
        state,
        on_progress.map(|id| id.channel_on(webview)),
        move |a| a.preview_discard(&membership_id, &request),
    )
    .await
}
#[tauri::command]
async fn discard_changes<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    state: State<'_>,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
    membership_id: String,
    request: DiscardRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run_progress(
        state,
        on_progress.map(|id| id.channel_on(webview)),
        move |a| a.discard_changes(&membership_id, &request, &token),
    )
    .await
}
#[tauri::command]
async fn preview_worktree_maintenance(
    state: State<'_>,
    membership_id: String,
    request: WorktreeMaintenanceRequest,
) -> Result<WorktreeMaintenancePreview, AppError> {
    run(state, move |a| {
        a.preview_worktree_maintenance(&membership_id, &request)
    })
    .await
}
#[tauri::command]
async fn maintain_worktree(
    state: State<'_>,
    membership_id: String,
    request: WorktreeMaintenanceRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.maintain_worktree(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn preview_submodule_lifecycle(
    state: State<'_>,
    membership_id: String,
    request: SubmoduleLifecycleRequest,
) -> Result<SubmoduleLifecyclePreview, AppError> {
    run(state, move |a| {
        a.preview_submodule_lifecycle(&membership_id, &request)
    })
    .await
}
#[tauri::command]
async fn change_submodule_lifecycle(
    state: State<'_>,
    membership_id: String,
    request: SubmoduleLifecycleRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.change_submodule_lifecycle(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn preview_submodule_sync(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
) -> Result<SubmoduleSyncPreview, AppError> {
    run(state, move |a| {
        a.preview_submodule_sync(&membership_id, &path)
    })
    .await
}
#[tauri::command]
async fn sync_submodule(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.sync_submodule(&membership_id, &path, &token)
    })
    .await
}
#[tauri::command]
async fn preview_remove_worktree(
    state: State<'_>,
    membership_id: String,
    request: WorktreeRemoveRequest,
) -> Result<WorktreeRemovePreview, AppError> {
    run(state, move |a| {
        a.preview_remove_worktree(&membership_id, &request)
    })
    .await
}
#[tauri::command]
async fn remove_worktree(
    state: State<'_>,
    membership_id: String,
    request: WorktreeRemoveRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.remove_worktree(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn preview_create_worktree(
    state: State<'_>,
    membership_id: String,
    request: WorktreeCreateRequest,
) -> Result<WorktreeCreatePreview, AppError> {
    run(state, move |a| {
        a.preview_create_worktree(&membership_id, &request)
    })
    .await
}
#[tauri::command]
async fn create_worktree(
    state: State<'_>,
    membership_id: String,
    request: WorktreeCreateRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.create_worktree(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn stashes(state: State<'_>, membership_id: String) -> Result<StashList, AppError> {
    run(state, move |a| a.stashes(&membership_id)).await
}
#[tauri::command]
async fn preview_stash(
    state: State<'_>,
    membership_id: String,
    request: StashRequest,
) -> Result<StashPreview, AppError> {
    run(state, move |a| a.preview_stash(&membership_id, &request)).await
}
#[tauri::command]
async fn stash_patch(
    state: State<'_>,
    membership_id: String,
    request: StashRequest,
    token: String,
    path: NativePath,
    index: bool,
) -> Result<HistoricalPatch, AppError> {
    run(state, move |a| {
        a.stash_patch(&membership_id, &request, &token, &path, index)
    })
    .await
}
#[tauri::command]
async fn apply_stash(
    state: State<'_>,
    membership_id: String,
    request: StashRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.apply_stash(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn historical_patch(
    state: State<'_>,
    membership_id: String,
    oid: String,
    path: NativePath,
) -> Result<HistoricalPatch, AppError> {
    run(state, move |a| {
        a.historical_patch(&membership_id, &oid, &path)
    })
    .await
}
#[tauri::command]
async fn search_commits(
    state: State<'_>,
    request: CommitSearchRequest,
) -> Result<CommitSearchPage, AppError> {
    run(state, move |a| a.search_commits(&request)).await
}
#[tauri::command]
async fn investigate_history(
    state: State<'_>,
    request: InvestigationRequest,
) -> Result<InvestigationPage, AppError> {
    run(state, move |a| a.investigate_history(&request)).await
}
#[tauri::command]
async fn focus_commit(
    state: State<'_>,
    workspace_id: String,
    membership_id: String,
    oid: String,
) -> Result<GraphFocus, AppError> {
    run(state, move |a| {
        a.focus_commit(&workspace_id, &membership_id, &oid)
    })
    .await
}
#[tauri::command]
async fn catalog(state: State<'_>) -> Result<Catalog, AppError> {
    let result = run(state, |a| a.catalog()).await;
    if result.is_ok() {
        smoke_marker("CATALOG_OK");
    }
    result
}
#[tauri::command]
async fn preview_init(state: State<'_>, request: InitRequest) -> Result<InitPreview, AppError> {
    run(state, move |a| a.preview_init(&request)).await
}
#[tauri::command]
async fn init_repository(
    state: State<'_>,
    request: InitRequest,
    token: String,
) -> Result<Catalog, AppError> {
    run(state, move |a| a.init_repository(&request, &token)).await
}
#[tauri::command]
async fn preview_clone(state: State<'_>, request: CloneRequest) -> Result<ClonePreview, AppError> {
    run(state, move |a| a.preview_clone(&request)).await
}
#[tauri::command]
async fn recent_clones(
    state: State<'_>,
    workspace_id: String,
) -> Result<Vec<CloneOutcome>, AppError> {
    run(state, move |a| a.recent_clones(&workspace_id)).await
}
#[tauri::command]
async fn clone_repository<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    state: State<'_>,
    request: CloneRequest,
    token: String,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
) -> Result<CloneOutcome, AppError> {
    run_progress(
        state,
        on_progress.map(|id| id.channel_on(webview)),
        move |a| a.clone_repository(&request, &token),
    )
    .await
}
#[tauri::command]
async fn add_repository(state: State<'_>, path: String) -> Result<Catalog, AppError> {
    run(state, move |a| a.add_repository(&PathBuf::from(path))).await
}
#[tauri::command]
async fn create_workspace(state: State<'_>, name: String) -> Result<Catalog, AppError> {
    run(state, move |a| a.create_workspace(&name)).await
}
#[tauri::command]
async fn switch_workspace(state: State<'_>, workspace_id: String) -> Result<Catalog, AppError> {
    run(state, move |a| a.switch_workspace(&workspace_id)).await
}
#[tauri::command]
async fn select_repository(state: State<'_>, membership_id: String) -> Result<Catalog, AppError> {
    run(state, move |a| a.select(&membership_id)).await
}
#[tauri::command]
async fn pin_repository(state: State<'_>, membership_id: String) -> Result<Catalog, AppError> {
    run(state, move |a| a.pin(&membership_id)).await
}
#[tauri::command]
async fn repository_status(
    state: State<'_>,
    membership_id: String,
) -> Result<RepoStatus, AppError> {
    run(state, move |a| a.status(&membership_id)).await
}
#[tauri::command]
async fn graph_page(
    state: State<'_>,
    membership_id: String,
    cursor: Option<GraphCursor>,
) -> Result<GraphPage, AppError> {
    let result = run(state, move |a| a.graph(&membership_id, cursor)).await;
    if result.is_ok() {
        smoke_marker("GRAPH_OK");
    }
    result
}
#[tauri::command]
async fn commit_detail(
    state: State<'_>,
    membership_id: String,
    oid: String,
) -> Result<CommitDetail, AppError> {
    run(state, move |a| a.detail(&membership_id, &oid)).await
}
#[tauri::command]
async fn submodules(state: State<'_>, membership_id: String) -> Result<Vec<Submodule>, AppError> {
    run(state, move |a| a.submodules(&membership_id)).await
}
#[tauri::command]
async fn submodule_snapshot(
    state: State<'_>,
    membership_id: String,
) -> Result<SubmoduleSnapshot, AppError> {
    run(state, move |a| a.submodule_snapshot(&membership_id)).await
}
#[tauri::command]
async fn worktrees(
    state: State<'_>,
    membership_id: String,
) -> Result<Vec<LinkedWorktree>, AppError> {
    run(state, move |a| a.worktrees(&membership_id)).await
}
#[tauri::command]
async fn open_related(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
) -> Result<Catalog, AppError> {
    run(state, move |a| a.open_related(&membership_id, &path)).await
}

#[tauri::command]
async fn repository_snapshot(
    state: State<'_>,
    membership_id: String,
) -> Result<RepositorySnapshot, AppError> {
    run(state, move |a| a.snapshot(&membership_id)).await
}
#[tauri::command]
async fn working_changes(
    state: State<'_>,
    membership_id: String,
) -> Result<WorkingChanges, AppError> {
    run(state, move |a| a.working_changes(&membership_id)).await
}
#[tauri::command]
async fn file_preview(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    staged: bool,
) -> Result<FilePreview, AppError> {
    run(state, move |a| {
        a.file_preview(&membership_id, &path, staged)
    })
    .await
}
#[tauri::command]
async fn amend_original(
    state: State<'_>,
    membership_id: String,
    oid: String,
) -> Result<AmendOriginal, AppError> {
    run(state, move |a| a.amend_original(&membership_id, &oid)).await
}
#[tauri::command]
async fn preview_amend(
    state: State<'_>,
    membership_id: String,
    request: AmendRequest,
) -> Result<AmendPreview, AppError> {
    run(state, move |a| a.preview_amend(&membership_id, &request)).await
}
#[tauri::command]
async fn amend_commit(
    state: State<'_>,
    membership_id: String,
    request: AmendRequest,
    token: String,
    acknowledge_rewrite: bool,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.amend_commit(&membership_id, &request, &token, acknowledge_rewrite)
    })
    .await
}
#[tauri::command]
async fn preview_stage_all(
    state: State<'_>,
    membership_id: String,
) -> Result<StageAllPreview, AppError> {
    run(state, move |a| a.preview_stage_all(&membership_id)).await
}
#[tauri::command]
async fn stage_all(
    state: State<'_>,
    membership_id: String,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| a.stage_all(&membership_id, &token)).await
}
#[tauri::command]
async fn preview_commit_file(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    token: String,
) -> Result<FilePreview, AppError> {
    run(state, move |a| {
        a.commit_file_preview(&membership_id, &path, &token)
    })
    .await
}
#[tauri::command]
async fn preview_remote_add(
    state: State<'_>,
    membership_id: String,
    request: RemoteAddRequest,
) -> Result<RemoteAddPreview, AppError> {
    run(state, move |a| {
        a.preview_remote_add(&membership_id, &request)
    })
    .await
}
#[tauri::command]
async fn add_remote(
    state: State<'_>,
    membership_id: String,
    request: RemoteAddRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.add_remote(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn preview_commit(
    state: State<'_>,
    membership_id: String,
) -> Result<CommitPreview, AppError> {
    run(state, move |a| a.commit_preview(&membership_id)).await
}
#[tauri::command]
async fn preview_hunks(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    staged: bool,
) -> Result<HunkPreview, AppError> {
    run(state, move |a| {
        a.preview_hunks(&membership_id, &path, staged)
    })
    .await
}
#[tauri::command]
async fn change_hunk(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    staged: bool,
    hunk: usize,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.change_hunk(&membership_id, &path, staged, hunk, &token)
    })
    .await
}
#[tauri::command]
async fn change_index(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    stage: bool,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.change_index(&membership_id, &path, stage, &token)
    })
    .await
}
#[tauri::command]
async fn create_commit(
    state: State<'_>,
    membership_id: String,
    message: String,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.create_commit(&membership_id, &message, &token)
    })
    .await
}

#[tauri::command]
async fn local_branches(
    state: State<'_>,
    membership_id: String,
) -> Result<Vec<LocalBranch>, AppError> {
    run(state, move |a| a.local_branches(&membership_id)).await
}
#[tauri::command]
async fn preview_branch_switch(
    state: State<'_>,
    membership_id: String,
    branch: String,
) -> Result<BranchSwitchPreview, AppError> {
    run(state, move |a| {
        a.preview_branch_switch(&membership_id, &branch)
    })
    .await
}
#[tauri::command]
async fn switch_branch(
    state: State<'_>,
    membership_id: String,
    branch: String,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.switch_branch(&membership_id, &branch, &token)
    })
    .await
}
#[tauri::command]
async fn remote_branches(
    state: State<'_>,
    membership_id: String,
) -> Result<RemoteBranches, AppError> {
    run(state, move |a| a.remote_branches(&membership_id)).await
}
#[tauri::command]
async fn auto_fetch(state: State<'_>, membership_id: String) -> Result<Option<String>, AppError> {
    run(state, move |a| a.auto_fetch(&membership_id)).await
}
#[tauri::command]
async fn fetch_remote(
    state: State<'_>,
    membership_id: String,
    remote: String,
    branch: Option<String>,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.fetch_remote(&membership_id, &remote, branch.as_deref())
    })
    .await
}
#[tauri::command]
async fn pull_upstream(
    state: State<'_>,
    membership_id: String,
) -> Result<BranchTracking, AppError> {
    run(state, move |a| a.pull_upstream(&membership_id)).await
}
#[tauri::command]
async fn preview_pull_current(
    state: State<'_>,
    membership_id: String,
) -> Result<RemoteSwitchPreview, AppError> {
    run(state, move |a| a.preview_pull_current(&membership_id)).await
}
#[tauri::command]
async fn pull_current(
    state: State<'_>,
    membership_id: String,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| a.pull_current(&membership_id, &token)).await
}
#[tauri::command]
async fn preview_remote_switch(
    state: State<'_>,
    membership_id: String,
    remote: String,
    branch: String,
) -> Result<RemoteSwitchPreview, AppError> {
    run(state, move |a| {
        a.preview_remote_switch(&membership_id, &remote, &branch)
    })
    .await
}
#[tauri::command]
async fn switch_remote_branch(
    state: State<'_>,
    membership_id: String,
    remote: String,
    branch: String,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.switch_remote_branch(&membership_id, &remote, &branch, &token)
    })
    .await
}
#[tauri::command]
async fn preview_submodule_pointer(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    stage: bool,
) -> Result<SubmodulePointerPreview, AppError> {
    run(state, move |a| {
        a.preview_submodule_pointer(&membership_id, &path, stage)
    })
    .await
}
#[tauri::command]
async fn change_submodule_pointer(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    stage: bool,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.change_submodule_pointer(&membership_id, &path, stage, &token)
    })
    .await
}
#[tauri::command]
async fn preview_submodule_update(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    mode: String,
) -> Result<SubmoduleUpdatePreview, AppError> {
    run(state, move |a| {
        a.preview_submodule_update(&membership_id, &path, &mode)
    })
    .await
}
#[tauri::command]
async fn update_submodule(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    mode: String,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.update_submodule(&membership_id, &path, &mode, &token)
    })
    .await
}
#[tauri::command]
async fn preview_graph_action(
    state: State<'_>,
    membership_id: String,
    action: GraphAction,
) -> Result<GraphActionPreview, AppError> {
    run(state, move |a| {
        a.preview_graph_action(&membership_id, &action)
    })
    .await
}
#[tauri::command]
async fn apply_graph_action(
    state: State<'_>,
    membership_id: String,
    action: GraphAction,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.apply_graph_action(&membership_id, &action, &token)
    })
    .await
}
#[tauri::command]
async fn preview_remote_delete(
    state: State<'_>,
    membership_id: String,
    request: RemoteDeleteRequest,
) -> Result<RemoteDeletePreview, AppError> {
    run(state, move |a| {
        a.preview_remote_delete(&membership_id, &request)
    })
    .await
}
#[tauri::command]
async fn delete_remote_branch(
    state: State<'_>,
    membership_id: String,
    request: RemoteDeleteRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.delete_remote_branch(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn preview_ref_operation(
    state: State<'_>,
    membership_id: String,
    action: RefOperation,
) -> Result<RefOperationPreview, AppError> {
    run(state, move |a| {
        a.preview_ref_operation(&membership_id, &action)
    })
    .await
}
#[tauri::command]
async fn apply_ref_operation(
    state: State<'_>,
    membership_id: String,
    action: RefOperation,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.apply_ref_operation(&membership_id, &action, &token)
    })
    .await
}
#[tauri::command]
async fn commit_link(
    state: State<'_>,
    membership_id: String,
    selection: HostingSelection,
) -> Result<String, AppError> {
    run(state, move |a| a.commit_link(&membership_id, &selection)).await
}
#[tauri::command]
async fn open_ci_url(state: State<'_>, url: String) -> Result<(), AppError> {
    run(state, move |_| {
        let url = gitnova_core::validated_ci_url(&url)?;
        browser::open(&url)
    })
    .await
}
#[tauri::command]
async fn connect_jenkins(
    state: State<'_>,
    membership_id: String,
    selection: JenkinsSelection,
    prompt: CredentialPrompt,
) -> Result<String, AppError> {
    run(state, move |a| {
        a.connect_jenkins(&membership_id, &selection, &prompt)
    })
    .await
}
#[tauri::command]
async fn jenkins_builds(
    state: State<'_>,
    membership_id: String,
    selection: JenkinsSelection,
    refresh: bool,
) -> Result<JenkinsSnapshot, AppError> {
    run(state, move |a| {
        a.jenkins_builds(&membership_id, &selection, refresh)
    })
    .await
}
#[tauri::command]
async fn repository_create_options(
    state: State<'_>,
    profile: HostProfile,
) -> Result<RepositoryCreateOptions, AppError> {
    run(state, move |a| a.repository_create_options(&profile)).await
}
#[tauri::command]
async fn preview_repository_create(
    state: State<'_>,
    membership_id: String,
    request: RepositoryCreateRequest,
) -> Result<RepositoryCreatePreview, AppError> {
    run(state, move |a| {
        a.preview_repository_create(&membership_id, &request)
    })
    .await
}
#[tauri::command]
async fn create_repository(
    state: State<'_>,
    membership_id: String,
    request: RepositoryCreateRequest,
    token: String,
) -> Result<RepositoryCreateOutcome, AppError> {
    run(state, move |a| {
        a.create_repository(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn connection_commit_identity(
    state: State<'_>,
    profile: HostProfile,
) -> Result<Option<GitCommitIdentity>, AppError> {
    run(state, move |a| a.connection_commit_identity(&profile)).await
}
#[tauri::command]
async fn set_connection_commit_identity(
    state: State<'_>,
    profile: HostProfile,
    identity: Option<GitCommitIdentity>,
) -> Result<(), AppError> {
    run(state, move |a| {
        a.set_connection_commit_identity(&profile, identity)
    })
    .await
}
#[tauri::command]
async fn commit_identity(
    state: State<'_>,
    membership_id: String,
) -> Result<CommitIdentityContext, AppError> {
    run(state, move |a| a.commit_identity(&membership_id)).await
}
#[tauri::command]
async fn host_account(state: State<'_>, profile: HostProfile) -> Result<HostingContext, AppError> {
    run(state, move |a| a.host_account(&profile)).await
}
#[tauri::command]
async fn connect_host(
    state: State<'_>,
    profile: HostProfile,
    prompt: CredentialPrompt,
) -> Result<HostingContext, AppError> {
    run(state, move |a| a.connect_host(&profile, &prompt)).await
}
#[tauri::command]
async fn disconnect_host(state: State<'_>, profile: HostProfile) -> Result<(), AppError> {
    run(state, move |a| a.disconnect_host(&profile)).await
}
#[tauri::command]
async fn ci_connection(
    state: State<'_>,
    membership_id: String,
    oid: String,
    profiles: Vec<HostProfile>,
) -> Result<Option<HostingSelection>, AppError> {
    run(state, move |a| {
        a.ci_connection(&membership_id, &oid, &profiles)
    })
    .await
}
#[tauri::command]
async fn jenkins_account(
    state: State<'_>,
    controller: String,
    username: String,
) -> Result<Option<String>, AppError> {
    run(state, move |a| a.jenkins_account(&controller, &username)).await
}
#[tauri::command]
async fn connect_jenkins_account(
    state: State<'_>,
    controller: String,
    username: String,
    prompt: CredentialPrompt,
) -> Result<String, AppError> {
    run(state, move |a| {
        a.connect_jenkins_account(&controller, &username, &prompt)
    })
    .await
}
#[tauri::command]
async fn disconnect_jenkins_account(
    state: State<'_>,
    controller: String,
    username: String,
) -> Result<(), AppError> {
    run(state, move |a| {
        a.disconnect_jenkins_account(&controller, &username)
    })
    .await
}
#[tauri::command]
async fn pipeline_context(
    state: State<'_>,
    membership_id: String,
    selection: HostingSelection,
) -> Result<HostingContext, AppError> {
    run(state, move |a| {
        a.pipeline_context(&membership_id, &selection)
    })
    .await
}
#[tauri::command]
async fn connect_pipeline(
    state: State<'_>,
    membership_id: String,
    selection: HostingSelection,
    prompt: CredentialPrompt,
) -> Result<HostingContext, AppError> {
    run(state, move |a| {
        a.connect_pipeline(&membership_id, &selection, &prompt)
    })
    .await
}
#[tauri::command]
async fn ci_summary(
    state: State<'_>,
    membership_id: String,
    selection: HostingSelection,
) -> Result<PipelineSnapshot, AppError> {
    run(state, move |a| a.ci_summary(&membership_id, &selection)).await
}
#[tauri::command]
async fn pipelines(
    state: State<'_>,
    membership_id: String,
    selection: HostingSelection,
    refresh: bool,
) -> Result<PipelineSnapshot, AppError> {
    run(state, move |a| {
        a.pipelines(&membership_id, &selection, refresh)
    })
    .await
}
#[tauri::command]
async fn hosting_context(
    state: State<'_>,
    membership_id: String,
    selection: HostingSelection,
) -> Result<HostingContext, AppError> {
    run(state, move |a| {
        a.hosting_context(&selection, &membership_id)
    })
    .await
}
#[tauri::command]
async fn disconnect_hosting(
    state: State<'_>,
    membership_id: String,
    selection: HostingSelection,
) -> Result<HostingContext, AppError> {
    run(state, move |a| {
        a.disconnect_hosting(&membership_id, &selection)
    })
    .await
}
#[tauri::command]
async fn connect_hosting(
    state: State<'_>,
    membership_id: String,
    selection: HostingSelection,
    prompt: CredentialPrompt,
) -> Result<HostingContext, AppError> {
    run(state, move |a| {
        a.connect_hosting(&membership_id, &selection, &prompt)
    })
    .await
}
#[tauri::command]
async fn preview_hosting(
    state: State<'_>,
    membership_id: String,
    request: HostingRequest,
) -> Result<HostingPreview, AppError> {
    run(state, move |a| a.preview_hosting(&membership_id, &request)).await
}
#[tauri::command]
async fn create_hosting(
    state: State<'_>,
    membership_id: String,
    request: HostingRequest,
    token: String,
) -> Result<HostingOutcome, AppError> {
    run(state, move |a| {
        a.create_hosting(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn preview_tag(
    state: State<'_>,
    membership_id: String,
    request: TagRequest,
) -> Result<TagPreview, AppError> {
    run(state, move |a| a.preview_tag(&membership_id, &request)).await
}
#[tauri::command]
async fn change_tag(
    state: State<'_>,
    membership_id: String,
    request: TagRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.change_tag(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn preview_push(
    state: State<'_>,
    membership_id: String,
    request: PushRequest,
) -> Result<PushPreview, AppError> {
    run(state, move |a| a.preview_push(&membership_id, &request)).await
}
#[tauri::command]
async fn push_branch(
    state: State<'_>,
    membership_id: String,
    request: PushRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.push_branch(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn merge_state(state: State<'_>, membership_id: String) -> Result<MergeState, AppError> {
    run(state, move |a| a.merge_state(&membership_id)).await
}
#[tauri::command]
async fn preview_merge(
    state: State<'_>,
    membership_id: String,
    request: MergeRequest,
) -> Result<MergeStartPreview, AppError> {
    run(state, move |a| a.preview_merge(&membership_id, &request)).await
}
#[tauri::command]
async fn begin_merge(
    state: State<'_>,
    membership_id: String,
    request: MergeRequest,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.begin_merge(&membership_id, &request, &token)
    })
    .await
}
#[tauri::command]
async fn merge_file(
    state: State<'_>,
    membership_id: String,
    path: NativePath,
    staged: Option<bool>,
) -> Result<MergeFile, AppError> {
    run(state, move |a| {
        a.merge_file_scope(&membership_id, &path, staged.unwrap_or(true))
    })
    .await
}
#[tauri::command]
async fn preview_merge_action(
    state: State<'_>,
    membership_id: String,
    action: MergeAction,
) -> Result<MergeActionPreview, AppError> {
    run(state, move |a| {
        a.preview_merge_action(&membership_id, &action)
    })
    .await
}
#[tauri::command]
async fn apply_merge_action(
    state: State<'_>,
    membership_id: String,
    action: MergeAction,
    token: String,
) -> Result<OperationReceipt, AppError> {
    run(state, move |a| {
        a.apply_merge_action(&membership_id, &action, &token)
    })
    .await
}
fn commands<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        terminal::terminal_open,
        terminal::terminal_read,
        terminal::terminal_write,
        terminal::terminal_resize,
        terminal::terminal_close,
        native_soak_report,
        recorded_activity,
        preview_stale_lock,
        recover_stale_lock,
        preview_operation_discard,
        discard_operation_changes,
        preview_discard,
        discard_changes,
        preview_remove_worktree,
        preview_worktree_maintenance,
        maintain_worktree,
        preview_submodule_lifecycle,
        change_submodule_lifecycle,
        preview_submodule_sync,
        sync_submodule,
        remove_worktree,
        preview_create_worktree,
        create_worktree,
        stashes,
        preview_stash,
        stash_patch,
        apply_stash,
        historical_patch,
        search_commits,
        investigate_history,
        focus_commit,
        merge_state,
        preview_merge,
        begin_merge,
        merge_file,
        preview_merge_action,
        apply_merge_action,
        preview_remote_delete,
        delete_remote_branch,
        preview_ref_operation,
        apply_ref_operation,
        commit_link,
        open_ci_url,
        connect_jenkins,
        jenkins_builds,
        repository_create_options,
        preview_repository_create,
        create_repository,
        connection_commit_identity,
        set_connection_commit_identity,
        commit_identity,
        host_account,
        connect_host,
        disconnect_host,
        ci_connection,
        jenkins_account,
        connect_jenkins_account,
        disconnect_jenkins_account,
        pipeline_context,
        connect_pipeline,
        pipelines,
        ci_summary,
        hosting_context,
        connect_hosting,
        disconnect_hosting,
        preview_hosting,
        create_hosting,
        preview_tag,
        change_tag,
        preview_push,
        push_branch,
        preview_graph_action,
        apply_graph_action,
        preview_submodule_pointer,
        change_submodule_pointer,
        preview_submodule_update,
        update_submodule,
        remote_branches,
        fetch_remote,
        auto_fetch,
        pull_upstream,
        preview_pull_current,
        pull_current,
        preview_remote_switch,
        switch_remote_branch,
        local_branches,
        preview_branch_switch,
        switch_branch,
        repository_snapshot,
        working_changes,
        file_preview,
        amend_original,
        preview_amend,
        amend_commit,
        preview_stage_all,
        stage_all,
        preview_commit_file,
        preview_remote_add,
        add_remote,
        preview_commit,
        change_index,
        preview_hunks,
        change_hunk,
        create_commit,
        catalog,
        preview_init,
        init_repository,
        preview_clone,
        clone_repository,
        recent_clones,
        add_repository,
        create_workspace,
        switch_workspace,
        select_repository,
        pin_repository,
        repository_status,
        graph_page,
        commit_detail,
        submodules,
        submodule_snapshot,
        worktrees,
        open_related
    ]
}
fn verify_embedded_ui(context: &tauri::Context<tauri::Wry>) -> Result<(), String> {
    if !cfg!(feature = "custom-protocol") {
        return Err("This development binary has no embedded UI. Use build:local.".into());
    }
    let index = context
        .assets
        .get(&"index.html".into())
        .ok_or("Missing embedded index.html")?;
    let html = std::str::from_utf8(&index).map_err(|e| e.to_string())?;
    if !html.contains("id=\"root\"") {
        return Err("Missing frontend root element".into());
    }
    let mut scripts = 0;
    let mut styles = 0;
    for attribute in ["src=\"", "href=\""] {
        for suffix in html.split(attribute).skip(1) {
            let path = suffix
                .split('"')
                .next()
                .unwrap_or("")
                .trim_start_matches("./")
                .trim_start_matches('/');
            if !path.starts_with("assets/") {
                continue;
            }
            if context.assets.get(&path.into()).is_none() {
                return Err(format!("Missing embedded asset: {path}"));
            }
            scripts += usize::from(path.ends_with(".js"));
            styles += usize::from(path.ends_with(".css"));
        }
    }
    if scripts == 0 || styles == 0 {
        return Err("Missing bundled JavaScript or stylesheet".into());
    }
    Ok(())
}
fn main() {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments
        .first()
        .is_some_and(|v| v == "--gitnova-credential-helper")
    {
        use std::io::{Read, Write};
        // Git owns this stdout pipe. Never start a window, open the catalog,
        // log credentials or print errors in this mode.
        let result = (|| -> Result<(), AppError> {
            if arguments.len() != 2 {
                return Err(AppError::new("git_auth", "Invalid helper invocation."));
            }
            let mut input = Vec::new();
            std::io::stdin().take(16385).read_to_end(&mut input)?;
            if input.len() > 16384 {
                return Err(AppError::new("git_auth", "Credential input too large."));
            }
            let operation = arguments[1]
                .to_str()
                .ok_or_else(|| AppError::new("git_auth", "Invalid helper operation."))?;
            let response = gitnova_core::git_credentials::git_credential_helper(operation, &input)?;
            std::io::stdout().write_all(&response)?;
            Ok(())
        })();
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    if !native_smoke() {
        if let Err(error) = std::env::current_exe()
            .map_err(AppError::from)
            .and_then(|path| {
                gitnova_core::git_credentials::install_transport_credential_helper(&path)
            })
        {
            eprintln!("Could not initialize Git credential bridge: {}", error.code);
        }
    }
    let mut context = tauri::generate_context!();
    if std::env::args_os()
        .skip(1)
        .any(|arg| arg == "--verify-bundle")
    {
        match verify_embedded_ui(&context) {
            Ok(()) => {
                println!(
                    "GitNova {}: embedded UI assets verified; no dev server or workspace opened; native-smoke={}.",
                    env!("CARGO_PKG_VERSION"), if cfg!(debug_assertions) { "enabled" } else { "disabled" }
                );
                return;
            }
            Err(message) => {
                eprintln!("Bundle verification failed: {message}");
                std::process::exit(1);
            }
        }
    }
    if native_smoke() {
        for window in &mut context.config_mut().app.windows {
            window.visible = false;
            window.focus = false;
            window.incognito = true;
        }
    }
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                terminal::close_all();
            }
        })
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if native_smoke() {
                smoke_marker("REUSED_WINDOW");
                return;
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .on_page_load(|window, payload| {
            if window.label() == "main"
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Started)
            {
                terminal::close_all();
            }
            if native_smoke() && matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                if let Ok(seconds) = std::env::var("GITNOVA_NATIVE_SOAK_SECONDS")
                    .unwrap_or_default()
                    .parse::<u64>()
                {
                    if (60..=7200).contains(&seconds) {
                        let script = format!(
                            "window.__gitnovaSoakSeconds={seconds};\n{}",
                            include_str!("../../scripts/native-soak-driver.js")
                        );
                        let _ = window.eval(&script);
                    }
                }
            }
        })
        .setup(|app| {
            let file =
                native_smoke_state()?.unwrap_or(app.path().app_data_dir()?.join("workspaces.json"));
            app.manage(Arc::new(Application::new(file)?));
            Ok(())
        })
        .invoke_handler(commands())
        .run(context)
        .expect("Could not start GitNova");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    // Both IPC tests share the process-wide queue; one intentionally fills it.
    static IPC_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    #[test]
    fn ci_and_commit_link_commands_are_read_only_and_reject_external_origins() {
        let _guard = IPC_TEST_LOCK.lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args([
                    "-c",
                    "user.name=CI IPC",
                    "-c",
                    "user.email=ci@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/dev/null",
                ])
                .args(args)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        git(&["init", "-b", "main"]);
        git(&["commit", "--allow-empty", "-m", "CI IPC"]);
        let oid = git(&["rev-parse", "HEAD"]);
        git(&["remote", "add", "origin", "git@gitlab.com:team/project.git"]);
        let app = tauri::test::mock_builder()
            .manage(Arc::new(
                Application::new(directory.path().join("state.json")).unwrap(),
            ))
            .invoke_handler(commands())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let invoke = |command: &str, args: Value, url: &str| {
            tauri::test::get_ipc_response(
                &window,
                tauri::webview::InvokeRequest {
                    cmd: command.into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: url.parse().unwrap(),
                    body: tauri::ipc::InvokeBody::Json(args),
                    headers: Default::default(),
                    invoke_key: tauri::test::INVOKE_KEY.into(),
                },
            )
            .map(|body| body.deserialize::<Value>().unwrap())
        };
        let catalog = invoke("add_repository", json!({"path":repo}), "tauri://localhost").unwrap();
        let membership = &catalog["workspaces"][0]["selectedId"];
        for command in [
            "terminal_open",
            "terminal_read",
            "terminal_write",
            "terminal_resize",
            "terminal_close",
        ] {
            assert!(invoke(command,json!({"membershipId":membership,"id":"missing","cols":80,"rows":24,"data":"echo blocked"}),"https://example.invalid").is_err());
        }
        assert!(invoke(
            "terminal_open",
            json!({"membershipId":"not-in-catalog","cols":80,"rows":24}),
            "tauri://localhost"
        )
        .is_err());
        let identity_profile = json!({"provider":"gitlab","origin":"https://gitlab.com"});
        let identity_args = json!({"profile":identity_profile,"identity":{"name":"Personal Fixture","email":"personal@example.invalid"}});
        assert!(invoke(
            "set_connection_commit_identity",
            identity_args.clone(),
            "https://example.invalid"
        )
        .is_err());
        assert_eq!(
            invoke(
                "connection_commit_identity",
                json!({"profile":identity_profile}),
                "tauri://localhost"
            )
            .unwrap(),
            Value::Null
        );
        invoke(
            "set_connection_commit_identity",
            identity_args,
            "tauri://localhost",
        )
        .unwrap();
        assert_eq!(
            invoke(
                "commit_identity",
                json!({"membershipId":membership}),
                "tauri://localhost"
            )
            .unwrap()["identity"]["email"],
            "personal@example.invalid"
        );
        assert!(invoke(
            "commit_identity",
            json!({"membershipId":membership}),
            "https://example.invalid"
        )
        .is_err());
        invoke(
            "set_connection_commit_identity",
            json!({"profile":identity_profile,"identity":null}),
            "tauri://localhost",
        )
        .unwrap();
        let selection =
            json!({"remote":"origin","branch":"","oid":oid,"provider":"gitlab","origin":null});
        let args = json!({"membershipId":membership,"selection":selection});
        assert_eq!(
            invoke("commit_link", args.clone(), "tauri://localhost").unwrap(),
            format!("https://gitlab.com/team/project/-/commit/{oid}")
        );
        assert!(invoke("commit_link", args, "https://example.invalid").is_err());
        // Global accounts require no membership/SHA and reject invalid hosts before
        // Keychain access or native credential prompts.
        let invalid_host = json!({"profile":{"provider":"gitlab","origin":"http://git.example.com"},"prompt":{"message":"test","confirm":"test","cancel":"test"}});
        for command in ["host_account", "connect_host", "disconnect_host"] {
            assert!(invoke(command, invalid_host.clone(), "https://example.invalid").is_err());
            assert!(invoke(command, invalid_host.clone(), "tauri://localhost").is_err());
        }
        let invalid_jenkins = json!({"controller":"http://ci.example.com","username":"alice","prompt":{"message":"test","confirm":"test","cancel":"test"}});
        for command in [
            "jenkins_account",
            "connect_jenkins_account",
            "disconnect_jenkins_account",
        ] {
            assert!(invoke(command, invalid_jenkins.clone(), "https://example.invalid").is_err());
            assert!(invoke(command, invalid_jenkins.clone(), "tauri://localhost").is_err());
        }
        assert!(invoke("ci_connection",json!({"membershipId":membership,"oid":oid,"profiles":[{"provider":"gitlab","origin":"http://invalid.example"}]}),"tauri://localhost").is_err());
        // Invalid scope is rejected before any Keychain access, prompt or network call.
        let invalid = json!({"membershipId":membership,"selection":{"remote":"missing","branch":"","oid":oid,"provider":"gitlab","origin":null},"refresh":false,"prompt":{"message":"test","confirm":"test","cancel":"test"}});
        for command in [
            "pipeline_context",
            "connect_pipeline",
            "pipelines",
            "ci_summary",
        ] {
            assert!(invoke(command, invalid.clone(), "https://example.invalid").is_err());
            let error = invoke(command, invalid.clone(), "tauri://localhost").unwrap_err();
            assert_eq!(error["code"], "remote");
        }
        assert!(invoke(
            "open_ci_url",
            json!({"url":"https://ci.example.com/job/app/9/"}),
            "https://example.invalid"
        )
        .is_err());
        let bad_link = invoke(
            "open_ci_url",
            json!({"url":"file:///tmp/private"}),
            "tauri://localhost",
        )
        .unwrap_err();
        assert_eq!(bad_link["code"], "hosting");
        let jenkins = json!({"membershipId":membership,"selection":{"remote":"missing","oid":oid,"controller":"https://ci.example.com/jenkins/","jobUrl":"https://ci.example.com/jenkins/job/app/job/main/","username":"alice"},"refresh":false,"prompt":{"message":"test","confirm":"test","cancel":"test"}});
        for command in ["connect_jenkins", "jenkins_builds"] {
            assert!(invoke(command, jenkins.clone(), "https://example.invalid").is_err());
            let error = invoke(command, jenkins.clone(), "tauri://localhost").unwrap_err();
            assert_eq!(error["code"], "remote");
        }
        assert_eq!(git(&["rev-parse", "HEAD"]), oid);
        assert_eq!(git(&["status", "--porcelain"]), "");
    }
    #[test]
    fn real_git_flows_through_tauri_commands_and_remote_origins_are_rejected() {
        let _guard = IPC_TEST_LOCK.lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec![
                "-c",
                "user.name=GitNova Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "--allow-empty",
                "-m",
                "IPC fixture",
            ],
        ] {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        }
        let app = tauri::test::mock_builder()
            .manage(Arc::new(
                Application::new(directory.path().join("state.json")).unwrap(),
            ))
            .invoke_handler(commands())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let invoke = |command: &str, args: Value, url: &str| {
            tauri::test::get_ipc_response(
                &window,
                tauri::webview::InvokeRequest {
                    cmd: command.into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: url.parse().unwrap(),
                    body: tauri::ipc::InvokeBody::Json(args),
                    headers: Default::default(),
                    invoke_key: tauri::test::INVOKE_KEY.into(),
                },
            )
            .map(|body| body.deserialize::<Value>().unwrap())
        };
        let catalog = invoke("add_repository", json!({"path":repo}), "tauri://localhost").unwrap();
        let full_queue = COMMAND_QUEUE.clone().try_acquire_many_owned(32).unwrap();
        let busy = invoke("catalog", json!({}), "tauri://localhost").unwrap_err();
        assert_eq!(busy["code"], "busy");
        drop(full_queue);
        assert!(invoke("catalog", json!({}), "tauri://localhost").is_ok());
        let membership = catalog["workspaces"][0]["selectedId"].as_str().unwrap();
        let clone_path = directory.path().join("ipc-clone");
        let clone_request = json!({"workspaceId":catalog["activeWorkspaceId"],"source":repo,"destination":clone_path});
        let clone_preview = invoke(
            "preview_clone",
            json!({"request":clone_request}),
            "tauri://localhost",
        )
        .unwrap();
        assert_eq!(clone_preview["version"], 1);
        assert!(!clone_path.exists());
        assert!(invoke(
            "preview_clone",
            json!({"request":clone_request}),
            "https://example.invalid"
        )
        .is_err());
        let clone_args = json!({"request":clone_preview["request"],"token":clone_preview["token"],"onProgress":null});
        assert!(invoke(
            "clone_repository",
            clone_args.clone(),
            "https://example.invalid"
        )
        .is_err());
        assert!(!clone_path.exists());
        let cloned = invoke("clone_repository", clone_args, "tauri://localhost").unwrap();
        assert_eq!(cloned["status"], "succeeded");
        assert_eq!(
            cloned["catalog"]["workspaces"][0]["memberships"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let journal_args = json!({"workspaceId":catalog["activeWorkspaceId"]});
        assert_eq!(
            invoke("recent_clones", journal_args.clone(), "tauri://localhost")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(invoke("recent_clones", journal_args, "https://example.invalid").is_err());

        // Clone intentionally selected its new membership; restore the source context
        // before exercising the pre-existing search/working-change scenarios below.
        invoke(
            "select_repository",
            json!({"membershipId":membership}),
            "tauri://localhost",
        )
        .unwrap();
        let page = invoke(
            "graph_page",
            json!({"membershipId":membership,"cursor":null}),
            "tauri://localhost",
        )
        .unwrap();
        assert_eq!(page["rows"][0]["commit"]["subject"], "IPC fixture");
        assert_eq!(page["tracking"][0]["branch"], "main");
        assert_eq!(page["tracking"][0]["state"], "noUpstream");
        assert!(page["tracking"][0]["ahead"].is_null());
        assert!(page["tracking"][0]["behind"].is_null());
        for command in ["pull_upstream", "preview_pull_current"] {
            assert!(invoke(
                command,
                json!({"membershipId":membership}),
                "tauri://localhost"
            )
            .is_err());
            assert!(invoke(
                command,
                json!({"membershipId":membership}),
                "https://example.invalid"
            )
            .is_err());
        }
        let pull_args = json!({"membershipId":membership,"token":"unreviewed"});
        let pull = invoke("pull_current", pull_args.clone(), "tauri://localhost").unwrap();
        assert_eq!(pull["status"], "failed");
        assert!(invoke("pull_current", pull_args, "https://example.invalid").is_err());
        let oid = page["rows"][0]["commit"]["oid"].as_str().unwrap();
        for method in ["auto", "mergeCommit", "fastForwardOnly"] {
            let args = json!({"membershipId":membership,"request":{"operation":"merge","method":method,"mainline":null,"sourceOid":oid,"sourceRef":null,"sessionId":""}});
            let preview = invoke("preview_merge", args.clone(), "tauri://localhost").unwrap();
            assert_eq!(preview["request"]["method"], method);
            assert_eq!(preview["mode"], "up_to_date");
            assert!(invoke("preview_merge", args, "https://example.invalid").is_err());
        }
        let invalid_method = json!({"membershipId":membership,"request":{"operation":"merge","method":"silentlyFallback","sourceOid":oid,"sourceRef":null,"sessionId":""}});
        assert!(invoke("preview_merge", invalid_method, "tauri://localhost").is_err());
        let detail = invoke(
            "commit_detail",
            json!({"membershipId":membership,"oid":oid}),
            "tauri://localhost",
        )
        .unwrap();
        assert_eq!(detail["commit"]["oid"], oid);
        assert_eq!(detail["files"], json!([]));
        let patch_args = json!({"membershipId":membership,"oid":oid,"path":NativePath::new(std::path::Path::new("absent.txt"))});
        let patch = invoke("historical_patch", patch_args.clone(), "tauri://localhost").unwrap();
        assert_eq!(patch["diff"], "");
        assert_eq!(patch["truncated"], false);
        assert!(invoke("historical_patch", patch_args, "https://example.invalid").is_err());
        let saved = invoke(
            "stashes",
            json!({"membershipId":membership}),
            "tauri://localhost",
        )
        .unwrap();
        assert_eq!(saved["entries"], json!([]));
        let stash_args = json!({"membershipId":membership,"request":{"kind":"push","index":null,"oid":null,"message":"IPC stash","includeUntracked":false,"restoreIndex":true,"id":""}});
        let stash_review =
            invoke("preview_stash", stash_args.clone(), "tauri://localhost").unwrap();
        assert!(stash_review["blockedReason"]
            .as_str()
            .unwrap()
            .contains("No changes"));
        assert_eq!(stash_review["version"], 2);
        let missing_patch = json!({"membershipId":membership,"request":stash_review["request"],"token":stash_review["token"],"path":NativePath::new(std::path::Path::new("absent.txt")),"index":false});
        assert!(invoke("stash_patch", missing_patch.clone(), "tauri://localhost").is_err());
        assert!(invoke("stash_patch", missing_patch, "https://example.invalid").is_err());
        let saved_path = repo.join("stash-ipc.txt");
        std::fs::write(&saved_path, "IPC saved preview\n").unwrap();
        let positive=invoke("preview_stash",json!({"membershipId":membership,"request":{"kind":"push","index":null,"oid":null,"message":"IPC preview","includeUntracked":true,"restoreIndex":false,"id":""}}),"tauri://localhost").unwrap();
        let args = json!({"membershipId":membership,"request":positive["request"],"token":positive["token"],"path":NativePath::new(std::path::Path::new("stash-ipc.txt")),"index":false});
        let selected = invoke("stash_patch", args.clone(), "tauri://localhost").unwrap();
        assert!(selected["diff"]
            .as_str()
            .unwrap()
            .contains("+IPC saved preview"));
        assert!(invoke("stash_patch", args, "https://example.invalid").is_err());
        std::fs::remove_file(saved_path).unwrap();
        let stash_apply = json!({"membershipId":membership,"request":stash_review["request"],"token":stash_review["token"]});
        let refused = invoke("apply_stash", stash_apply.clone(), "tauri://localhost").unwrap();
        assert_eq!(refused["status"], "failed");
        assert!(invoke(
            "stashes",
            json!({"membershipId":membership}),
            "https://example.invalid"
        )
        .is_err());
        assert!(invoke("preview_stash", stash_args, "https://example.invalid").is_err());
        assert!(invoke("apply_stash", stash_apply, "https://example.invalid").is_err());
        let worktree_args = json!({"membershipId":membership,"request":{"oid":oid,"destination":directory.path().join("ipc-worktree").to_string_lossy(),"mode":"existingBranch","branch":"main"}});
        let tree_review = invoke(
            "preview_create_worktree",
            worktree_args.clone(),
            "tauri://localhost",
        )
        .unwrap();
        assert!(tree_review["blockedReason"]
            .as_str()
            .unwrap()
            .contains("already checked out"));
        let tree_apply = json!({"membershipId":membership,"request":tree_review["request"],"token":tree_review["token"]});
        assert_eq!(
            invoke("create_worktree", tree_apply.clone(), "tauri://localhost").unwrap()["status"],
            "failed"
        );
        assert!(invoke(
            "preview_create_worktree",
            worktree_args,
            "https://example.invalid"
        )
        .is_err());
        assert!(invoke("create_worktree", tree_apply, "https://example.invalid").is_err());
        let removal_args = json!({"membershipId":membership,"request":{"path":NativePath::new(&repo.canonicalize().unwrap()),"id":""}});
        let removal_review = invoke(
            "preview_remove_worktree",
            removal_args.clone(),
            "tauri://localhost",
        )
        .unwrap();
        assert!(removal_review["blockedReason"]
            .as_str()
            .unwrap()
            .contains("main worktree"));
        let removal_apply = json!({"membershipId":membership,"request":removal_review["request"],"token":removal_review["token"]});
        assert_eq!(
            invoke(
                "remove_worktree",
                removal_apply.clone(),
                "tauri://localhost"
            )
            .unwrap()["status"],
            "failed"
        );
        assert!(repo.exists());
        assert!(invoke(
            "preview_remove_worktree",
            removal_args,
            "https://example.invalid"
        )
        .is_err());
        assert!(invoke("remove_worktree", removal_apply, "https://example.invalid").is_err());
        std::fs::write(repo.join("discard.txt"), "IPC unsaved work").unwrap();
        let discard_args = json!({"membershipId":membership,"onProgress":"__CHANNEL__:91","request":{"paths":[NativePath::new(std::path::Path::new("discard.txt"))],"scope":"unstaged","id":""}});
        let discard_review =
            invoke("preview_discard", discard_args.clone(), "tauri://localhost").unwrap();
        assert!(discard_review["blockedReason"].is_null());
        let discard_apply = json!({"membershipId":membership,"request":discard_review["request"],"token":discard_review["token"]});
        assert!(invoke("preview_discard", discard_args, "https://example.invalid").is_err());
        assert!(invoke(
            "discard_changes",
            discard_apply.clone(),
            "https://example.invalid"
        )
        .is_err());
        assert!(repo.join("discard.txt").exists());
        assert_eq!(
            invoke("discard_changes", discard_apply, "tauri://localhost").unwrap()["status"],
            "succeeded"
        );
        assert!(!repo.join("discard.txt").exists());
        let operation_discard_args = json!({"membershipId":membership,"request":{"paths":[NativePath::new(std::path::Path::new("discard.txt"))],"scope":"all","id":""},"operationToken":"not-current","token":"not-current"});
        for command in ["preview_operation_discard", "discard_operation_changes"] {
            let error =
                invoke(command, operation_discard_args.clone(), "tauri://localhost").unwrap_err();
            assert_eq!(error["code"], "discard_blocked");
            assert!(invoke(
                command,
                operation_discard_args.clone(),
                "https://example.invalid"
            )
            .is_err());
        }

        let activity_args = json!({"membershipId":membership});
        assert!(invoke(
            "recorded_activity",
            activity_args.clone(),
            "https://example.invalid"
        )
        .is_err());
        let activity = invoke("recorded_activity", activity_args, "tauri://localhost").unwrap();
        assert!(activity["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["kind"] == "discard_changes"));

        let workspace = catalog["activeWorkspaceId"].as_str().unwrap();
        let search_args = json!({"request": {
            "workspaceId": workspace, "currentMembershipId": membership,
            "scope": "workspace", "selectedIds": [], "query": "IPC fixture", "cursor": null
        }});
        let search = invoke("search_commits", search_args.clone(), "tauri://localhost").unwrap();
        assert_eq!(search["hits"][0]["commit"]["oid"], oid);
        assert_eq!(search["partial"], false);
        let investigation_args = json!({"request": {"selection":search_args["request"],"mode":"history","path":"file.txt","needle":"","source":"main","target":"main"}});
        assert!(invoke(
            "investigate_history",
            investigation_args.clone(),
            "https://example.invalid"
        )
        .is_err());
        let evidence = invoke(
            "investigate_history",
            investigation_args,
            "tauri://localhost",
        )
        .unwrap();
        assert!(evidence["evidence"].is_array());
        let focus_args = json!({"workspaceId":workspace,"membershipId":membership,"oid":oid});
        let focus = invoke("focus_commit", focus_args.clone(), "tauri://localhost").unwrap();
        assert_eq!(focus["page"]["rows"][0]["commit"]["oid"], oid);
        assert_eq!(focus["selectedIndex"], 0);
        assert!(invoke("search_commits", search_args, "https://example.invalid").is_err());
        assert!(invoke("focus_commit", focus_args, "https://example.invalid").is_err());
        assert!(invoke("catalog", json!({}), "https://example.invalid").is_err());
        let fixture_git = |path: &std::path::Path, args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args([
                    "-c",
                    "user.name=IPC Pointer",
                    "-c",
                    "user.email=ipc@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "protocol.file.allow=always",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        fixture_git(
            &clone_path,
            &["submodule", "add", "--", repo.to_str().unwrap(), "module"],
        );
        fixture_git(&clone_path, &["commit", "-am", "Record native module"]);
        let child = clone_path.join("module");
        let recorded = fixture_git(&child, &["rev-parse", "HEAD"]);
        fixture_git(
            &child,
            &["commit", "--allow-empty", "-m", "Native child next"],
        );
        let next = fixture_git(&child, &["rev-parse", "HEAD"]);
        let pointer_membership = cloned["catalog"]["workspaces"][0]["selectedId"]
            .as_str()
            .unwrap();
        let display_args = json!({"membershipId":pointer_membership});
        let display = invoke(
            "submodule_snapshot",
            display_args.clone(),
            "tauri://localhost",
        )
        .unwrap();
        assert_eq!(display["submodules"][0]["indexOid"], recorded);
        assert_eq!(display["submodules"][0]["checkedOutOid"], next);
        assert!(invoke(
            "submodule_snapshot",
            display_args,
            "https://example.invalid"
        )
        .is_err());
        for stage in [true, false] {
            let args = json!({"membershipId":pointer_membership,"path":NativePath::new(std::path::Path::new("module")),"stage":stage});
            let preview = invoke(
                "preview_submodule_pointer",
                args.clone(),
                "tauri://localhost",
            )
            .unwrap();
            assert_eq!(preview["version"], 1);
            assert!(preview["blockedReason"].is_null());
            assert!(invoke(
                "preview_submodule_pointer",
                args.clone(),
                "https://example.invalid"
            )
            .is_err());
            let mut apply = args;
            apply["token"] = preview["token"].clone();
            assert!(invoke(
                "change_submodule_pointer",
                apply.clone(),
                "https://example.invalid"
            )
            .is_err());
            assert_eq!(
                invoke("change_submodule_pointer", apply, "tauri://localhost").unwrap()["status"],
                "succeeded"
            );
            assert_eq!(
                fixture_git(&clone_path, &["rev-parse", ":module"]),
                if stage {
                    next.clone()
                } else {
                    recorded.clone()
                }
            );
            assert_eq!(fixture_git(&child, &["rev-parse", "HEAD"]), next);
        }

        let tagged = fixture_git(&clone_path, &["rev-parse", "HEAD"]);
        let request = json!({"kind":"create","name":"native/v1","oid":tagged,"annotation":null});
        let args = json!({"membershipId":pointer_membership,"request":request});
        assert!(invoke("preview_tag", args.clone(), "https://example.invalid").is_err());
        let preview = invoke("preview_tag", args.clone(), "tauri://localhost").unwrap();
        let mut apply = args;
        apply["token"] = preview["token"].clone();
        assert!(invoke("change_tag", apply.clone(), "https://example.invalid").is_err());
        assert_eq!(
            invoke("change_tag", apply, "tauri://localhost").unwrap()["status"],
            "succeeded"
        );
        assert_eq!(
            fixture_git(&clone_path, &["rev-parse", "native/v1"]),
            tagged
        );
        for command in ["preview_remote_delete", "delete_remote_branch"] {
            assert!(invoke(command,json!({"membershipId":pointer_membership,"request":{"remote":"origin","branch":"unused","oid":tagged},"token":"untrusted"}),"https://example.invalid").is_err());
        }
        let selection = json!({"remote":"origin","branch":fixture_git(&clone_path,&["branch","--show-current"]),"oid":tagged,"provider":"gitlab","origin":null});
        for command in ["hosting_context", "connect_hosting", "disconnect_hosting"] {
            assert!(invoke(
                command,
                json!({"membershipId":pointer_membership,"selection":selection}),
                "https://example.invalid"
            )
            .is_err());
        }
        let request = json!({"selection":selection,"targetProject":"team/project","targetBranch":"main","title":"Native request","body":"","draft":true});
        assert!(invoke(
            "preview_hosting",
            json!({"membershipId":pointer_membership,"request":request}),
            "https://example.invalid"
        )
        .is_err());
        assert!(invoke(
            "create_hosting",
            json!({"membershipId":pointer_membership,"request":request,"token":"never-authorized"}),
            "https://example.invalid"
        )
        .is_err());
        let args = json!({"membershipId":pointer_membership,"oid":tagged});
        assert!(invoke("amend_original", args.clone(), "https://example.invalid").is_err());
        assert_eq!(
            invoke("amend_original", args, "tauri://localhost").unwrap()["commit"]["oid"],
            tagged
        );
        let args = json!({"membershipId":pointer_membership,"request":{"oid":tagged,"message":"Native amend review"}});
        assert!(invoke("preview_amend", args.clone(), "https://example.invalid").is_err());
        let preview = invoke("preview_amend", args.clone(), "tauri://localhost").unwrap();
        let recovery = preview["recoveryRef"].as_str().unwrap().to_owned();
        let mut args = args;
        args["token"] = preview["token"].clone();
        args["acknowledgeRewrite"] = json!(true);
        assert!(invoke("amend_commit", args.clone(), "https://example.invalid").is_err());
        assert_eq!(
            invoke("amend_commit", args, "tauri://localhost").unwrap()["status"],
            "succeeded"
        );
        assert_ne!(fixture_git(&clone_path, &["rev-parse", "HEAD"]), tagged);
        assert_eq!(fixture_git(&clone_path, &["rev-parse", &recovery]), tagged);
        assert_eq!(
            fixture_git(&clone_path, &["rev-parse", "native/v1"]),
            tagged
        );
    }
}
