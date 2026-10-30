//! Test-only transport for browser E2E against the real Rust core. Not shipped in the app.
use gitnova_core::Application;
use gitnova_domain::{
    AppError, GraphCursor, NativePath, Result, SubmoduleLifecycleRequest,
    WorktreeMaintenanceRequest,
};
use serde_json::{json, Value};
use std::{io::Read, path::PathBuf};
fn dispatch(app: &Application, request: &Value) -> Result<Value> {
    let command = request["command"]
        .as_str()
        .ok_or_else(|| AppError::new("test", "Missing command"))?;
    let args = &request["args"];
    let s = |key: &str| {
        args[key]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| AppError::new("test", format!("Missing {key}")))
    };
    let value = match command {
        "preview_stale_lock" => json!(app.preview_stale_lock(&s("membershipId")?)?),
        "recover_stale_lock" => json!(app.recover_stale_lock(&s("membershipId")?, &s("token")?)?),
        "recorded_activity" => json!(app.recorded_activity(&s("membershipId")?)?),
        "preview_operation_discard" | "discard_operation_changes" => {
            let request = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("discard", e.to_string()))?;
            if command == "preview_operation_discard" {
                json!(app.preview_operation_discard(
                    &s("membershipId")?,
                    &request,
                    &s("operationToken")?
                )?)
            } else {
                json!(app.discard_operation_changes(
                    &s("membershipId")?,
                    &request,
                    &s("token")?,
                    &s("operationToken")?
                )?)
            }
        }
        "preview_discard" | "discard_changes" => {
            let r = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("discard", e.to_string()))?;
            if command == "preview_discard" {
                json!(app.preview_discard(&s("membershipId")?, &r)?)
            } else {
                json!(app.discard_changes(&s("membershipId")?, &r, &s("token")?)?)
            }
        }
        "preview_remove_worktree" | "remove_worktree" => {
            let request = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("worktree_request", e.to_string()))?;
            if command == "preview_remove_worktree" {
                json!(app.preview_remove_worktree(&s("membershipId")?, &request)?)
            } else {
                json!(app.remove_worktree(&s("membershipId")?, &request, &s("token")?)?)
            }
        }
        "preview_create_worktree" | "create_worktree" => {
            let r = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("worktree_request", e.to_string()))?;
            if command == "preview_create_worktree" {
                json!(app.preview_create_worktree(&s("membershipId")?, &r)?)
            } else {
                json!(app.create_worktree(&s("membershipId")?, &r, &s("token")?)?)
            }
        }
        "stashes" => json!(app.stashes(&s("membershipId")?)?),
        "preview_stash" | "apply_stash" => {
            let r = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("stash_request", e.to_string()))?;
            if command == "preview_stash" {
                json!(app.preview_stash(&s("membershipId")?, &r)?)
            } else {
                json!(app.apply_stash(&s("membershipId")?, &r, &s("token")?)?)
            }
        }
        "stash_patch" => json!(app.stash_patch(
            &s("membershipId")?,
            &serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("stash_request", e.to_string()))?,
            &s("token")?,
            &serde_json::from_value(args["path"].clone())
                .map_err(|e| AppError::new("path", e.to_string()))?,
            args["index"].as_bool().unwrap_or(false)
        )?),
        "historical_patch" => json!(app.historical_patch(
            &s("membershipId")?,
            &s("oid")?,
            &serde_json::from_value(args["path"].clone())
                .map_err(|e| AppError::new("path", e.to_string()))?
        )?),
        "search_commits" => {
            let request = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("search", format!("{e}")))?;
            json!(app.search_commits(&request)?)
        }
        "investigate_history" => {
            let request = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("investigation", format!("{e}")))?;
            json!(app.investigate_history(&request)?)
        }
        "focus_commit" => {
            json!(app.focus_commit(&s("workspaceId")?, &s("membershipId")?, &s("oid")?)?)
        }
        "preview_graph_action" | "apply_graph_action" => {
            let action = serde_json::from_value(args["action"].clone())
                .map_err(|e| AppError::new("graph_action", format!("{e}")))?;
            if command == "preview_graph_action" {
                json!(app.preview_graph_action(&s("membershipId")?, &action)?)
            } else {
                json!(app.apply_graph_action(&s("membershipId")?, &action, &s("token")?)?)
            }
        }
        "preview_remote_delete" | "delete_remote_branch" => {
            let request = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("remote_delete_blocked", format!("{e}")))?;
            if command == "preview_remote_delete" {
                json!(app.preview_remote_delete(&s("membershipId")?, &request)?)
            } else {
                json!(app.delete_remote_branch(&s("membershipId")?, &request, &s("token")?)?)
            }
        }
        "amend_original" => json!(app.amend_original(&s("membershipId")?, &s("oid")?)?),
        "preview_amend" | "amend_commit" => {
            let request = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("amend_blocked", format!("{e}")))?;
            if command == "preview_amend" {
                json!(app.preview_amend(&s("membershipId")?, &request)?)
            } else {
                json!(app.amend_commit(
                    &s("membershipId")?,
                    &request,
                    &s("token")?,
                    args["acknowledgeRewrite"].as_bool().unwrap_or(false)
                )?)
            }
        }
        "preview_ref_operation" | "apply_ref_operation" => {
            let action = serde_json::from_value(args["action"].clone())
                .map_err(|e| AppError::new("ref_operation", format!("{e}")))?;
            if command == "preview_ref_operation" {
                json!(app.preview_ref_operation(&s("membershipId")?, &action)?)
            } else {
                json!(app.apply_ref_operation(&s("membershipId")?, &action, &s("token")?)?)
            }
        }
        "commit_link" => {
            let selection = serde_json::from_value(args["selection"].clone())
                .map_err(|e| AppError::new("hosting", e.to_string()))?;
            json!(app.commit_link(&s("membershipId")?, &selection)?)
        }
        "jenkins_builds" => {
            let selection = serde_json::from_value(args["selection"].clone())
                .map_err(|e| AppError::new("hosting", e.to_string()))?;
            json!(app.jenkins_builds(
                &s("membershipId")?,
                &selection,
                args["refresh"].as_bool().unwrap_or(false)
            )?)
        }
        "connect_jenkins" => {
            return Err(AppError::new(
                "test",
                "Credential prompts are disabled in the test transport.",
            ))
        }
        "repository_create_options" => json!(app.repository_create_options(
            &serde_json::from_value(args["profile"].clone())
                .map_err(|e| AppError::new("json", e.to_string()))?
        )?),
        "preview_repository_create" => json!(app.preview_repository_create(
            &s("membershipId")?,
            &serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("json", e.to_string()))?
        )?),
        "create_repository" => json!(app.create_repository(
            &s("membershipId")?,
            &serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("json", e.to_string()))?,
            &s("token")?
        )?),
        "connection_commit_identity" => json!(app.connection_commit_identity(
            &serde_json::from_value(args["profile"].clone())
                .map_err(|e| AppError::new("json", e.to_string()))?
        )?),
        "set_connection_commit_identity" => json!(app.set_connection_commit_identity(
            &serde_json::from_value(args["profile"].clone())
                .map_err(|e| AppError::new("json", e.to_string()))?,
            serde_json::from_value(args["identity"].clone())
                .map_err(|e| AppError::new("json", e.to_string()))?
        )?),
        "commit_identity" => json!(app.commit_identity(&s("membershipId")?)?),
        "host_account" => json!(app.host_account(
            &serde_json::from_value(args["profile"].clone())
                .map_err(|e| AppError::new("hosting", e.to_string()))?
        )?),
        "ci_connection" => json!(app.ci_connection(
            &s("membershipId")?,
            &s("oid")?,
            &serde_json::from_value::<Vec<gitnova_domain::HostProfile>>(args["profiles"].clone())
                .map_err(|e| AppError::new("hosting", e.to_string()))?
        )?),
        "jenkins_account" => json!(app.jenkins_account(&s("controller")?, &s("username")?)?),
        "connect_host"
        | "disconnect_host"
        | "connect_jenkins_account"
        | "disconnect_jenkins_account" => {
            return Err(AppError::new(
                "test",
                "Credential writes are disabled in the test transport.",
            ))
        }
        "pipeline_context" | "connect_pipeline" | "pipelines" | "ci_summary" => {
            let selection: gitnova_domain::HostingSelection =
                serde_json::from_value(args["selection"].clone())
                    .map_err(|e| AppError::new("hosting", e.to_string()))?;
            match command {
                "pipeline_context" => json!(app.pipeline_context(&s("membershipId")?, &selection)?),
                "connect_pipeline" => {
                    return Err(AppError::new(
                        "test",
                        "Credential prompts are disabled in the test transport.",
                    ))
                }
                "ci_summary" => json!(app.ci_summary(&s("membershipId")?, &selection)?),
                _ => json!(app.pipelines(
                    &s("membershipId")?,
                    &selection,
                    args["refresh"].as_bool().unwrap_or(false)
                )?),
            }
        }
        "hosting_context" | "connect_hosting" | "disconnect_hosting" => {
            let selection = serde_json::from_value(args["selection"].clone())
                .map_err(|e| AppError::new("hosting", e.to_string()))?;
            if command == "hosting_context" {
                json!(app.hosting_context(&selection, &s("membershipId")?)?)
            } else {
                return Err(AppError::new(
                    "test",
                    "Native credential entry is not available in the browser test bridge.",
                ));
            }
        }
        "preview_hosting" | "create_hosting" => {
            let request = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("hosting", e.to_string()))?;
            if command == "preview_hosting" {
                json!(app.preview_hosting(&s("membershipId")?, &request)?)
            } else {
                json!(app.create_hosting(&s("membershipId")?, &request, &s("token")?)?)
            }
        }
        "preview_tag" | "change_tag" => {
            let request: gitnova_domain::TagRequest =
                serde_json::from_value(args["request"].clone())
                    .map_err(|e| AppError::new("tag_request", e.to_string()))?;
            if command == "preview_tag" {
                json!(app.preview_tag(&s("membershipId")?, &request)?)
            } else {
                json!(app.change_tag(&s("membershipId")?, &request, &s("token")?)?)
            }
        }
        "preview_push" | "push_branch" => {
            let request = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("push", format!("{e}")))?;
            if command == "preview_push" {
                json!(app.preview_push(&s("membershipId")?, &request)?)
            } else {
                json!(app.push_branch(&s("membershipId")?, &request, &s("token")?)?)
            }
        }
        "merge_state" => json!(app.merge_state(&s("membershipId")?)?),
        "preview_merge" | "begin_merge" => {
            let request = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("merge", format!("{e}")))?;
            if command == "preview_merge" {
                json!(app.preview_merge(&s("membershipId")?, &request)?)
            } else {
                json!(app.begin_merge(&s("membershipId")?, &request, &s("token")?)?)
            }
        }
        "merge_file" => json!(app.merge_file_scope(
            &s("membershipId")?,
            &serde_json::from_value(args["path"].clone())
                .map_err(|e| AppError::new("merge", format!("{e}")))?,
            args.get("staged")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true)
        )?),
        "preview_merge_action" | "apply_merge_action" => {
            let action = serde_json::from_value(args["action"].clone())
                .map_err(|e| AppError::new("merge", format!("{e}")))?;
            if command == "preview_merge_action" {
                json!(app.preview_merge_action(&s("membershipId")?, &action)?)
            } else {
                json!(app.apply_merge_action(&s("membershipId")?, &action, &s("token")?)?)
            }
        }
        "remote_branches" => json!(app.remote_branches(&s("membershipId")?)?),
        "auto_fetch" => json!(app.auto_fetch(&s("membershipId")?)?),
        "fetch_remote" => {
            json!(app.fetch_remote(&s("membershipId")?, &s("remote")?, args["branch"].as_str())?)
        }
        "pull_upstream" => json!(app.pull_upstream(&s("membershipId")?)?),
        "preview_pull_current" => json!(app.preview_pull_current(&s("membershipId")?)?),
        "pull_current" => json!(app.pull_current(&s("membershipId")?, &s("token")?)?),
        "preview_remote_switch" => {
            json!(app.preview_remote_switch(&s("membershipId")?, &s("remote")?, &s("branch")?)?)
        }
        "switch_remote_branch" => json!(app.switch_remote_branch(
            &s("membershipId")?,
            &s("remote")?,
            &s("branch")?,
            &s("token")?
        )?),
        "local_branches" => json!(app.local_branches(&s("membershipId")?)?),
        "preview_branch_switch" => {
            json!(app.preview_branch_switch(&s("membershipId")?, &s("branch")?)?)
        }
        "switch_branch" => {
            json!(app.switch_branch(&s("membershipId")?, &s("branch")?, &s("token")?)?)
        }
        "repository_snapshot" => json!(app.snapshot(&s("membershipId")?)?),
        "working_changes" => json!(app.working_changes(&s("membershipId")?)?),
        "file_preview" | "change_index" => {
            let path: NativePath = serde_json::from_value(args["path"].clone())
                .map_err(|e| AppError::new("test", e.to_string()))?;
            if command == "file_preview" {
                json!(app.file_preview(
                    &s("membershipId")?,
                    &path,
                    args["staged"].as_bool().unwrap_or(false)
                )?)
            } else {
                json!(app.change_index(
                    &s("membershipId")?,
                    &path,
                    args["stage"].as_bool().unwrap_or(false),
                    &s("token")?
                )?)
            }
        }
        "preview_worktree_maintenance" | "maintain_worktree" => {
            let r: WorktreeMaintenanceRequest = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("test", e.to_string()))?;
            if command == "preview_worktree_maintenance" {
                json!(app.preview_worktree_maintenance(&s("membershipId")?, &r)?)
            } else {
                json!(app.maintain_worktree(&s("membershipId")?, &r, &s("token")?)?)
            }
        }
        "preview_submodule_lifecycle" | "change_submodule_lifecycle" => {
            let r: SubmoduleLifecycleRequest = serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("test", e.to_string()))?;
            if command == "preview_submodule_lifecycle" {
                json!(app.preview_submodule_lifecycle(&s("membershipId")?, &r)?)
            } else {
                json!(app.change_submodule_lifecycle(&s("membershipId")?, &r, &s("token")?)?)
            }
        }
        "preview_submodule_sync" | "sync_submodule" => {
            let p: NativePath = serde_json::from_value(args["path"].clone())
                .map_err(|e| AppError::new("test", e.to_string()))?;
            if command == "preview_submodule_sync" {
                json!(app.preview_submodule_sync(&s("membershipId")?, &p)?)
            } else {
                json!(app.sync_submodule(&s("membershipId")?, &p, &s("token")?)?)
            }
        }
        "preview_hunks" | "change_hunk" => {
            let path: NativePath = serde_json::from_value(args["path"].clone())
                .map_err(|e| AppError::new("test", e.to_string()))?;
            let staged = args["staged"].as_bool().unwrap_or(false);
            if command == "preview_hunks" {
                json!(app.preview_hunks(&s("membershipId")?, &path, staged)?)
            } else {
                json!(app.change_hunk(
                    &s("membershipId")?,
                    &path,
                    staged,
                    args["hunk"]
                        .as_u64()
                        .ok_or_else(|| AppError::new("test", "Missing hunk"))?
                        as usize,
                    &s("token")?
                )?)
            }
        }
        "preview_stage_all" => json!(app.preview_stage_all(&s("membershipId")?)?),
        "stage_all" => json!(app.stage_all(&s("membershipId")?, &s("token")?)?),
        "preview_commit_file" => json!(app.commit_file_preview(
            &s("membershipId")?,
            &serde_json::from_value(args["path"].clone())
                .map_err(|e| AppError::new("test", e.to_string()))?,
            &s("token")?
        )?),
        "preview_remote_add" => json!(app.preview_remote_add(
            &s("membershipId")?,
            &serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("test", e.to_string()))?
        )?),
        "add_remote" => json!(app.add_remote(
            &s("membershipId")?,
            &serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("test", e.to_string()))?,
            &s("token")?
        )?),
        "preview_commit" => json!(app.commit_preview(&s("membershipId")?)?),
        "create_commit" => {
            json!(app.create_commit(&s("membershipId")?, &s("message")?, &s("token")?)?)
        }
        "catalog" => json!(app.catalog()?),
        "preview_init" => json!(app.preview_init(
            &serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("init", e.to_string()))?
        )?),
        "init_repository" => json!(app.init_repository(
            &serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("init", e.to_string()))?,
            &s("token")?
        )?),
        "preview_clone" => json!(app.preview_clone(
            &serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("clone", e.to_string()))?
        )?),
        "clone_repository" => json!(app.clone_repository(
            &serde_json::from_value(args["request"].clone())
                .map_err(|e| AppError::new("clone", e.to_string()))?,
            &s("token")?
        )?),
        "recent_clones" => json!(app.recent_clones(&s("workspaceId")?)?),
        "add_repository" => json!(app.add_repository(&PathBuf::from(s("path")?))?),
        "create_workspace" => json!(app.create_workspace(&s("name")?)?),
        "switch_workspace" => json!(app.switch_workspace(&s("workspaceId")?)?),
        "select_repository" => json!(app.select(&s("membershipId")?)?),
        "pin_repository" => json!(app.pin(&s("membershipId")?)?),
        "repository_status" => json!(app.status(&s("membershipId")?)?),
        "graph_page" => {
            let cursor: Option<GraphCursor> = serde_json::from_value(args["cursor"].clone())
                .map_err(|e| AppError::new("test", e.to_string()))?;
            json!(app.graph(&s("membershipId")?, cursor)?)
        }
        "commit_detail" => json!(app.detail(&s("membershipId")?, &s("oid")?)?),
        "preview_submodule_pointer" | "change_submodule_pointer" => {
            let path = serde_json::from_value(args["path"].clone())
                .map_err(|error| AppError::new("path", error.to_string()))?;
            let stage = args["stage"]
                .as_bool()
                .ok_or_else(|| AppError::new("stage", "Missing stage choice"))?;
            if command == "preview_submodule_pointer" {
                json!(app.preview_submodule_pointer(&s("membershipId")?, &path, stage)?)
            } else {
                json!(app.change_submodule_pointer(
                    &s("membershipId")?,
                    &path,
                    stage,
                    &s("token")?
                )?)
            }
        }
        "preview_submodule_update" => json!(app.preview_submodule_update(
            &s("membershipId")?,
            &serde_json::from_value(args["path"].clone())
                .map_err(|e| AppError::new("path", e.to_string()))?,
            &s("mode")?
        )?),
        "update_submodule" => json!(app.update_submodule(
            &s("membershipId")?,
            &serde_json::from_value(args["path"].clone())
                .map_err(|e| AppError::new("path", e.to_string()))?,
            &s("mode")?,
            &s("token")?
        )?),
        "submodules" => json!(app.submodules(&s("membershipId")?)?),
        "submodule_snapshot" => json!(app.submodule_snapshot(&s("membershipId")?)?),
        "worktrees" => json!(app.worktrees(&s("membershipId")?)?),
        "open_related" => {
            let path: NativePath = serde_json::from_value(args["path"].clone())
                .map_err(|e| AppError::new("test", e.to_string()))?;
            json!(app.open_related(&s("membershipId")?, &path)?)
        }
        _ => return Err(AppError::new("test", "Unsupported test command")),
    };
    Ok(value)
}
fn main() {
    let result = (|| -> Result<Value> {
        let path = std::env::args_os()
            .nth(1)
            .ok_or_else(|| AppError::new("test", "Missing state path"))?;
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        let request: Value =
            serde_json::from_str(&input).map_err(|e| AppError::new("test", e.to_string()))?;
        let app = Application::new(path.into())?;
        if request["args"]["onProgress"].is_string() {
            gitnova_core::with_progress(
                |event| {
                    if let Ok(value) = serde_json::to_string(&event) {
                        eprintln!("GITNOVA_PROGRESS:{value}");
                    }
                },
                || dispatch(&app, &request),
            )
        } else {
            dispatch(&app, &request)
        }
    })();
    println!(
        "{}",
        match result {
            Ok(value) => json!({"ok":value}),
            Err(error) => json!({"error":error}),
        }
    );
}
