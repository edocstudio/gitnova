use gitnova_core::Application;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim_end().into()
}
fn setup() -> (tempfile::TempDir, Application, PathBuf, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "GitNova Test"),
        ("user.email", "test@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", ".git/hooks"),
        ("core.attributesFile", ".git/no-global-attributes"),
    ] {
        git(&repo, &["config", key, value]);
    }
    fs::write(repo.join("file.txt"), "main\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Main"]);
    git(&repo, &["switch", "-c", "feature"]);
    fs::write(repo.join("file.txt"), "feature\n").unwrap();
    git(&repo, &["commit", "-am", "Feature"]);
    git(&repo, &["switch", "main"]);
    let app = Application::new(directory.path().join("app/state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    (directory, app, repo, id)
}

use gitnova_domain::*;
fn request(oid: &str) -> MergeRequest {
    MergeRequest {
        rebase_steps: vec![],
        acknowledge_rewrite: false,
        delete_source_branch: false,
        method: MergeMethod::Auto,
        operation: "merge".into(),
        mainline: None,
        source_oid: oid.into(),
        source_ref: Some("refs/heads/feature".into()),
        session_id: String::new(),
    }
}
fn begin(app: &Application, id: &str, oid: &str) -> OperationReceipt {
    let preview = app.preview_merge(id, &request(oid)).unwrap();
    assert!(
        preview.blocked_reason.is_none(),
        "{:?}",
        preview.blocked_reason
    );
    app.begin_merge(id, &preview.request, &preview.token)
        .unwrap()
}
fn action(kind: &str) -> MergeAction {
    MergeAction {
        kind: kind.into(),
        path: None,
        choice: None,
        content: None,
        message: String::new(),
    }
}
fn resolution(choice: &str, text: Option<&str>) -> MergeAction {
    MergeAction {
        kind: "resolve".into(),
        path: Some(NativePath::new(Path::new("file.txt"))),
        choice: Some(choice.into()),
        content: text.map(str::to_owned),
        message: String::new(),
    }
}
fn diverge(repo: &Path, conflict: bool) {
    if conflict {
        fs::write(repo.join("file.txt"), "current\n").unwrap();
    } else {
        fs::write(repo.join("destination.txt"), "destination\n").unwrap();
    }
    fs::write(repo.join("notes.txt"), "original notes\n").unwrap();
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", "Destination work"]);
}
fn object_files(path: &Path) -> Vec<PathBuf> {
    fn walk(path: &Path, result: &mut Vec<PathBuf>) {
        for item in fs::read_dir(path).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                walk(&path, result);
            } else {
                result.push(path);
            }
        }
    }
    let mut files = vec![];
    walk(path, &mut files);
    files.sort();
    files
}
#[test]
fn fast_forward_preview_writes_no_source_objects_index_or_worktree_and_apply_preserves_history() {
    let (_temp, app, repo, id) = setup();
    let source = git(&repo, &["rev-parse", "feature"]);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    let objects = object_files(&repo.join(".git/objects"));
    let preview = app.preview_merge(&id, &request(&source)).unwrap();
    assert_eq!(preview.mode, "fast_forward");
    let mut legacy = serde_json::to_value(&preview.request).unwrap();
    legacy.as_object_mut().unwrap().remove("method");
    assert_eq!(
        serde_json::from_value::<MergeRequest>(legacy)
            .unwrap()
            .method,
        MergeMethod::Auto
    );
    let mut explicit = preview.request.clone();
    explicit.method = MergeMethod::FastForwardOnly;
    let preview = app.preview_merge(&id, &explicit).unwrap();
    assert_eq!(preview.mode, "fast_forward");
    assert!(preview.conflicts.is_empty());
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(object_files(&repo.join(".git/objects")), objects);
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "main\n");
    let result = app
        .begin_merge(&id, &preview.request, &preview.token)
        .unwrap();
    assert_eq!(result.status, "succeeded", "{}", result.summary);
    assert_eq!(
        result.summary,
        "Branch fast-forwarded to the reviewed commit; no merge commit was created"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), source);
    assert_eq!(git(&repo, &["rev-parse", &preview.recovery_ref]), original);
    assert_eq!(
        git(
            &repo,
            &["rev-parse", &format!("{}-incoming", preview.recovery_ref)]
        ),
        source
    );
    assert_eq!(app.merge_state(&id).unwrap().phase, "none");
}

#[test]
fn explicit_merge_commit_on_linear_history_has_two_real_parents_and_a_graph_junction() {
    let (_temp, app, repo, id) = setup();
    let source = git(&repo, &["rev-parse", "feature"]);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let mut requested = request(&source);
    requested.method = MergeMethod::MergeCommit;
    let preview = app.preview_merge(&id, &requested).unwrap();
    assert!(
        preview.blocked_reason.is_none(),
        "{:?}",
        preview.blocked_reason
    );
    assert_ne!(preview.mode, "fast_forward");
    let mut altered = preview.request.clone();
    altered.method = MergeMethod::FastForwardOnly;
    assert_eq!(
        app.begin_merge(&id, &altered, &preview.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    let started = app
        .begin_merge(&id, &preview.request, &preview.token)
        .unwrap();
    assert_eq!(started.status, "pending", "{}", started.summary);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(git(&repo, &["rev-parse", "MERGE_HEAD"]), source);
    let finish = MergeAction {
        kind: "finish".into(),
        path: None,
        choice: None,
        content: None,
        message: "Merge feature into main".into(),
    };
    let reviewed = app.preview_merge_action(&id, &finish).unwrap();
    assert!(
        reviewed.blocked_reason.is_none(),
        "{:?}",
        reviewed.blocked_reason
    );
    assert_eq!(
        app.apply_merge_action(&id, &reviewed.action, &reviewed.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let merged = git(&repo, &["rev-parse", "HEAD"]);
    assert_ne!(merged, source);
    assert_eq!(
        git(&repo, &["show", "-s", "--format=%P", "HEAD"]),
        format!("{original} {source}")
    );
    assert_eq!(git(&repo, &["rev-parse", "feature"]), source);
    let graph = app.graph(&id, None).unwrap();
    assert_eq!(graph.rows[0].commit.oid, merged);
    assert_eq!(
        graph.rows[0].commit.parents,
        vec![original.clone(), source.clone()]
    );
    assert_eq!(graph.rows[0].edges.len(), 2);
    assert_ne!(
        graph
            .rows
            .iter()
            .find(|r| r.commit.oid == source)
            .unwrap()
            .lane,
        graph.rows[0].lane
    );
    assert_eq!(app.merge_state(&id).unwrap().phase, "none");
}

#[test]
fn explicit_fast_forward_refuses_divergence_and_method_does_not_apply_to_other_operations() {
    let (_temp, app, repo, id) = setup();
    fs::write(repo.join("destination.txt"), "separate progress\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Main progress"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let mut requested = request(&git(&repo, &["rev-parse", "feature"]));
    requested.method = MergeMethod::FastForwardOnly;
    let preview = app.preview_merge(&id, &requested).unwrap();
    assert!(preview
        .blocked_reason
        .as_deref()
        .is_some_and(|s| s.contains("diverged")));
    assert_ne!(
        app.begin_merge(&id, &preview.request, &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert!(!repo.join(".git/MERGE_HEAD").exists());
    requested.operation = "cherryPick".into();
    assert_eq!(
        app.preview_merge(&id, &requested).unwrap_err().code,
        "merge_method"
    );
    requested.operation = "merge".into();
    requested.method = MergeMethod::MergeCommit;
    requested.source_oid = head;
    requested.source_ref = Some("refs/heads/main".into());
    assert_eq!(
        app.preview_merge(&id, &requested).unwrap().mode,
        "up_to_date"
    );
}
#[test]
fn clean_three_way_merge_waits_for_review_and_finish_uses_exact_tree_and_two_parents() {
    let (_temp, app, repo, id) = setup();
    diverge(&repo, false);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let source = git(&repo, &["rev-parse", "feature"]);
    let result = begin(&app, &id, &source);
    assert_eq!(result.status, "pending", "{}", result.summary);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    let mut finish = action("finish");
    finish.message = "Merge feature after reviewing its staged result".into();
    let objects = object_files(&repo.join(".git/objects"));
    let preview = app.preview_merge_action(&id, &finish).unwrap();
    assert!(preview.diff.contains("+feature"));
    assert_eq!(object_files(&repo.join(".git/objects")), objects);
    let result = app
        .apply_merge_action(&id, &finish, &preview.token)
        .unwrap();
    assert_eq!(result.status, "succeeded", "{}", result.summary);
    assert_eq!(
        git(&repo, &["show", "-s", "--format=%P", "HEAD"]),
        format!("{original} {source}")
    );
    assert_eq!(
        git(&repo, &["rev-parse", "HEAD^{tree}"]),
        preview.expected_tree.unwrap()
    );
    assert_eq!(app.merge_state(&id).unwrap().phase, "none");
}
#[test]
fn conflict_choices_capture_prior_contents_and_leave_a_reviewable_staged_result() {
    let (_temp, app, repo, id) = setup();
    diverge(&repo, true);
    let source = git(&repo, &["rev-parse", "feature"]);
    let preview = app.preview_merge(&id, &request(&source)).unwrap();
    assert_eq!(preview.conflicts.len(), 1);
    let result = app
        .begin_merge(&id, &preview.request, &preview.token)
        .unwrap();
    assert_eq!(result.status, "conflicted", "{}", result.summary);
    let before = fs::read(repo.join("file.txt")).unwrap();
    let index_before = fs::read(repo.join(".git/index")).unwrap();
    let file = app
        .merge_file(&id, &NativePath::new(Path::new("file.txt")))
        .unwrap();
    assert_eq!(file.ours_text.as_deref(), Some("current\n"));
    assert_eq!(file.theirs_text.as_deref(), Some("feature\n"));
    assert!(file.working_text.as_deref().unwrap().contains("<<<<<<<"));
    assert!(file.editable);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index_before);
    assert_eq!(fs::read(repo.join("file.txt")).unwrap(), before);
    let select = resolution("theirs", None);
    let review = app.preview_merge_action(&id, &select).unwrap();
    assert!(review.diff.contains("feature"));
    assert_eq!(
        review.expected_tree.as_deref(),
        Some(git(&repo, &["rev-parse", "feature^{tree}"]).as_str())
    );
    assert_eq!(fs::read(repo.join("file.txt")).unwrap(), before);
    let result = app.apply_merge_action(&id, &select, &review.token).unwrap();
    assert_eq!(result.status, "pending", "{}", result.summary);
    assert_eq!(git(&repo, &["show", ":file.txt"]), "feature");
    let dir = review.recovery_dir.unwrap().to_path().unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    let name = manifest["entries"][0]["file"].as_str().unwrap();
    assert_eq!(fs::read(dir.join(name)).unwrap(), before);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(dir.join(name)).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert!(app.merge_state(&id).unwrap().conflicts.is_empty());
}
#[test]
fn edited_result_is_bound_to_actual_working_contents_and_conflict_markers_cannot_be_staged() {
    let (_temp, app, repo, id) = setup();
    diverge(&repo, true);
    let source = git(&repo, &["rev-parse", "feature"]);
    begin(&app, &id, &source);
    assert!(app
        .preview_merge_action(&id, &resolution("working", None))
        .is_err());
    for marker in [
        "<<<<<<< HEAD\n",
        "<<<<<<<<< HEAD\n",
        ">>>>>>>>> incoming\n",
        "||||||||| base\n",
    ] {
        assert!(app
            .preview_merge_action(&id, &resolution("edited", Some(marker)))
            .is_err());
    }
    let edit = resolution("edited", Some("combined current and feature\n"));
    let preview = app.preview_merge_action(&id, &edit).unwrap();
    fs::write(repo.join("file.txt"), "external manual edit\n").unwrap();
    assert!(app.apply_merge_action(&id, &edit, &preview.token).is_err());
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "external manual edit\n"
    );
    let review = app.preview_merge_action(&id, &edit).unwrap();
    let result = app.apply_merge_action(&id, &edit, &review.token).unwrap();
    assert_eq!(result.status, "pending", "{}", result.summary);
    assert_eq!(
        git(&repo, &["show", ":file.txt"]),
        "combined current and feature"
    );
}
#[test]
fn abort_saves_manual_working_and_staged_versions_before_restoring_original_head() {
    let (_temp, app, repo, id) = setup();
    diverge(&repo, true);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let source = git(&repo, &["rev-parse", "feature"]);
    begin(&app, &id, &source);
    fs::write(repo.join("file.txt"), "manual conflict work\n").unwrap();
    fs::write(repo.join("notes.txt"), "staged notes\n").unwrap();
    git(&repo, &["add", "notes.txt"]);
    fs::write(repo.join("notes.txt"), "unstaged notes\n").unwrap();
    let abort = action("abort");
    let preview = app.preview_merge_action(&id, &abort).unwrap();
    let result = app.apply_merge_action(&id, &abort, &preview.token).unwrap();
    assert!(
        matches!(result.status.as_str(), "succeeded" | "unknown"),
        "{}",
        result.summary
    );
    let dir = preview.recovery_dir.unwrap().to_path().unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    let mut contents = vec![];
    for entry in manifest["entries"].as_array().unwrap() {
        if let Some(name) = entry["file"].as_str() {
            contents.push(fs::read_to_string(dir.join(name)).unwrap());
        }
    }
    assert!(contents.contains(&"manual conflict work\n".to_owned()));
    assert!(contents.contains(&"unstaged notes\n".to_owned()));
    let mut blobs = vec![];
    for blob in manifest["stage_blobs"].as_array().unwrap() {
        blobs.push(
            fs::read_to_string(dir.join(format!("blob-{}", blob.as_str().unwrap()))).unwrap(),
        );
    }
    assert!(blobs.contains(&"staged notes\n".to_owned()));
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    if result.status == "succeeded" {
        assert_eq!(app.merge_state(&id).unwrap().phase, "none");
    }
}
#[test]
fn simple_abort_returns_to_original_checkout_and_keeps_merge_recovery_refs() {
    let (_temp, app, repo, id) = setup();
    diverge(&repo, true);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let source = git(&repo, &["rev-parse", "feature"]);
    begin(&app, &id, &source);
    let abort = action("abort");
    let preview = app.preview_merge_action(&id, &abort).unwrap();
    let result = app.apply_merge_action(&id, &abort, &preview.token).unwrap();
    assert_eq!(result.status, "succeeded", "{}", result.summary);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "current\n"
    );
    assert!(preview
        .recovery_dir
        .unwrap()
        .to_path()
        .unwrap()
        .join("manifest.json")
        .exists());
}
#[test]
fn dirty_hidden_locked_or_moved_sources_do_not_begin_a_merge() {
    let (_temp, app, repo, id) = setup();
    let source = git(&repo, &["rev-parse", "feature"]);
    fs::write(repo.join("new.txt"), "keep\n").unwrap();
    assert!(app
        .preview_merge(&id, &request(&source))
        .unwrap()
        .blocked_reason
        .is_some());
    fs::remove_file(repo.join("new.txt")).unwrap();
    git(&repo, &["update-index", "--assume-unchanged", "file.txt"]);
    assert!(app
        .preview_merge(&id, &request(&source))
        .unwrap()
        .blocked_reason
        .is_some());
    git(
        &repo,
        &["update-index", "--no-assume-unchanged", "file.txt"],
    );
    let preview = app.preview_merge(&id, &request(&source)).unwrap();
    git(&repo, &["branch", "-f", "feature", "main"]);
    assert!(app
        .begin_merge(&id, &preview.request, &preview.token)
        .is_err());
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "main\n");
}
#[cfg(unix)]
#[test]
fn preview_never_executes_configured_custom_merge_driver() {
    let (_temp, app, repo, id) = setup();
    diverge(&repo, true);
    let source = git(&repo, &["rev-parse", "feature"]);
    git(
        &repo,
        &["config", "merge.surprise.driver", "touch .git/driver-ran"],
    );
    let preview = app.preview_merge(&id, &request(&source)).unwrap();
    assert!(preview.blocked_reason.is_some());
    assert!(!repo.join(".git/driver-ran").exists());
    assert_eq!(app.merge_state(&id).unwrap().phase, "none");
}
#[test]
fn externally_started_merge_is_inspectable_but_not_silently_adopted() {
    let (_temp, app, repo, id) = setup();
    diverge(&repo, true);
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["merge", "--no-commit", "feature"])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap();
    assert!(!output.status.success());
    let state = app.merge_state(&id).unwrap();
    assert_eq!(state.phase, "external");
    assert!(!state.conflicts.is_empty());
    assert!(app.preview_merge_action(&id, &action("abort")).is_err());
}

#[test]
fn modify_delete_conflict_can_explicitly_choose_deletion() {
    let (_temp, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    git(&repo, &["rm", "file.txt"]);
    git(&repo, &["commit", "-m", "Remove file"]);
    git(&repo, &["switch", "main"]);
    diverge(&repo, true);
    let source = git(&repo, &["rev-parse", "feature"]);
    assert_eq!(begin(&app, &id, &source).status, "conflicted");
    let file = app
        .merge_file(&id, &NativePath::new(Path::new("file.txt")))
        .unwrap();
    assert_eq!(file.theirs_text.as_deref(), Some("")); // Missing stage 3 means deletion.
    assert_eq!(file.ours_text.as_deref(), Some("current\n"));
    let select = resolution("theirs", None);
    let preview = app.preview_merge_action(&id, &select).unwrap();
    assert!(preview.diff.contains("deleted file mode"));
    let receipt = app
        .apply_merge_action(&id, &select, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "pending", "{}", receipt.summary);
    assert!(!repo.join("file.txt").exists());
    assert!(app.merge_state(&id).unwrap().conflicts.is_empty());
}

#[test]
fn binary_conflict_is_not_editable_and_can_choose_exact_incoming_bytes() {
    let (_temp, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    fs::write(repo.join("file.txt"), b"\0incoming\xff\n").unwrap();
    git(&repo, &["commit", "-am", "Incoming binary"]);
    git(&repo, &["switch", "main"]);
    fs::write(repo.join("file.txt"), b"\0current\xfe\n").unwrap();
    git(&repo, &["commit", "-am", "Current binary"]);
    let source = git(&repo, &["rev-parse", "feature"]);
    assert_eq!(begin(&app, &id, &source).status, "conflicted");
    let file = app
        .merge_file(&id, &NativePath::new(Path::new("file.txt")))
        .unwrap();
    assert!(!file.editable);
    assert!(file.ours_text.is_none());
    assert!(file.theirs_text.is_none());
    let select = resolution("theirs", None);
    let preview = app.preview_merge_action(&id, &select).unwrap();
    let receipt = app
        .apply_merge_action(&id, &select, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "pending", "{}", receipt.summary);
    assert_eq!(
        fs::read(repo.join("file.txt")).unwrap(),
        b"\0incoming\xff\n"
    );
}

#[test]
fn finish_rejects_stale_index_and_merge_metadata_is_not_silently_adopted() {
    let (_temp, app, repo, id) = setup();
    diverge(&repo, false);
    let source = git(&repo, &["rev-parse", "feature"]);
    begin(&app, &id, &source);
    let mut finish = action("finish");
    finish.message = "Reviewed merge".into();
    let preview = app.preview_merge_action(&id, &finish).unwrap();
    fs::write(repo.join("notes.txt"), "new staged contents\n").unwrap();
    git(&repo, &["add", "notes.txt"]);
    assert!(app
        .apply_merge_action(&id, &finish, &preview.token)
        .is_err());
    assert_eq!(app.merge_state(&id).unwrap().phase, "pending");
    fs::write(repo.join(".git/ORIG_HEAD"), format!("{source}\n")).unwrap();
    assert_eq!(app.merge_state(&id).unwrap().phase, "incomplete");
    assert_eq!(
        app.preview_merge_action(&id, &action("abort"))
            .unwrap_err()
            .code,
        "merge_blocked"
    );
}

#[test]
fn ignored_files_are_not_overwritten_when_beginning_a_merge() {
    let (_temp, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    fs::write(repo.join("collision.txt"), "incoming tracked file\n").unwrap();
    git(&repo, &["add", "collision.txt"]);
    git(&repo, &["commit", "-m", "Add collision path"]);
    let source = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["switch", "main"]);
    fs::write(repo.join(".git/info/exclude"), "collision.txt\n").unwrap();
    fs::write(repo.join("collision.txt"), "ignored local data\n").unwrap();
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let preview = app.preview_merge(&id, &request(&source)).unwrap();
    let receipt = app
        .begin_merge(&id, &preview.request, &preview.token)
        .unwrap();
    assert_ne!(receipt.status, "succeeded");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(
        fs::read_to_string(repo.join("collision.txt")).unwrap(),
        "ignored local data\n"
    );
}

#[test]
fn linked_worktree_merge_session_is_bound_to_its_own_gitdir() {
    let (_temp, app, repo, _id) = setup();
    let linked = repo.parent().unwrap().join("linked");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            "linked-branch",
            linked.to_str().unwrap(),
            "main",
        ],
    );
    let catalog = app.add_repository(&linked).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    diverge(&linked, true);
    let source = git(&repo, &["rev-parse", "feature"]);
    let receipt = begin(&app, &id, &source);
    assert_eq!(receipt.status, "conflicted", "{}", receipt.summary);
    assert!(!repo.join(".git/gitnova-merge.json").exists());
    assert_eq!(git(&repo, &["symbolic-ref", "--short", "HEAD"]), "main");
    let preview = app.preview_merge_action(&id, &action("abort")).unwrap();
    let receipt = app
        .apply_merge_action(&id, &action("abort"), &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
}

#[test]
fn relative_attributes_and_quoted_alternate_paths_match_native_merge_results() {
    let (temp, _old_app, repo, _old_id) = setup();
    #[cfg(unix)]
    let moved = temp.path().join("repo with spaces:อักษร");
    #[cfg(windows)]
    let moved = temp.path().join("repo with spaces อักษร");
    fs::rename(repo, &moved).unwrap();
    git(
        &moved,
        &["config", "core.attributesFile", ".git/merge-attributes"],
    );
    fs::write(
        moved.join(".git/merge-attributes"),
        "file.txt merge=union\n",
    )
    .unwrap();
    diverge(&moved, true);
    let app = Application::new(temp.path().join("attributes-app/state.json")).unwrap();
    let catalog = app.add_repository(&moved).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    let source = git(&moved, &["rev-parse", "feature"]);
    let preview = app.preview_merge(&id, &request(&source)).unwrap();
    assert!(preview.conflicts.is_empty());
    let receipt = app
        .begin_merge(&id, &preview.request, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "pending", "{}", receipt.summary);
    assert!(app.merge_state(&id).unwrap().conflicts.is_empty());
    assert_eq!(git(&moved, &["write-tree"]), preview.result_tree.unwrap());
    assert!(fs::read_to_string(moved.join("file.txt"))
        .unwrap()
        .contains("feature"));
}

#[test]
fn large_staged_binary_and_partial_code_can_finish_without_loading_recovery_content() {
    let (_dir, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    let mut binary = vec![0u8; 17 * 1024 * 1024];
    binary[100] = 1;
    fs::write(repo.join("large.bin"), &binary).unwrap();
    fs::write(
        repo.join("large.txt"),
        "large review line with an explicit stable fixture value\n".repeat(20000),
    )
    .unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Large artifacts"]);
    let source = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["switch", "main"]);
    fs::write(repo.join("local.txt"), "local\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Independent main work"]);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let p = app.preview_merge(&id, &request(&source)).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let receipt = app.begin_merge(&id, &p.request, &p.token).unwrap();
    assert_eq!(receipt.status, "pending", "{}", receipt.summary);
    let action = MergeAction {
        kind: "finish".into(),
        path: None,
        choice: None,
        content: None,
        message: "Reviewed large staged result".into(),
    };
    let p = app.preview_merge_action(&id, &action).unwrap();
    assert!(p.truncated);
    assert!(p.blocked_reason.is_none());
    assert!(p.paths.iter().any(|p| p.display == "large.bin"));
    let file = app
        .merge_file(&id, &NativePath::new(Path::new("large.bin")))
        .unwrap();
    assert!(!file.editable);
    assert!(file.working_text.is_none());
    let file = app
        .merge_file(&id, &NativePath::new(Path::new("large.txt")))
        .unwrap();
    assert!(file.truncated);
    assert!(!file.editable);
    let abort = app
        .preview_merge_action(
            &id,
            &MergeAction {
                kind: "abort".into(),
                message: String::new(),
                ..action.clone()
            },
        )
        .unwrap();
    assert!(abort
        .blocked_reason
        .unwrap()
        .contains("Preserve these files externally"));
    binary[100] = 2;
    fs::write(repo.join("large.bin"), &binary).unwrap();
    assert_eq!(
        app.apply_merge_action(&id, &action, &p.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    binary[100] = 1;
    fs::write(repo.join("large.bin"), &binary).unwrap();
    let p = app.preview_merge_action(&id, &action).unwrap();
    let receipt = app.apply_merge_action(&id, &action, &p.token).unwrap();
    assert_eq!(
        receipt.status, "succeeded",
        "{} {:?}",
        receipt.summary, receipt.recovery
    );
    assert_eq!(
        git(&repo, &["show", "-s", "--format=%P", "HEAD"]),
        format!("{original} {source}")
    );
    assert_eq!(app.merge_state(&id).unwrap().phase, "none");
    assert_eq!(
        git(&repo, &["rev-parse", "HEAD^{tree}"]),
        p.expected_tree.unwrap()
    );
}

#[test]
fn interrupted_session_cleanup_archives_only_owned_metadata_and_preserves_staged_and_working_data()
{
    let (_dir, app, repo, id) = setup();
    diverge(&repo, false);
    let source = git(&repo, &["rev-parse", "feature"]);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let receipt = begin(&app, &id, &source);
    assert_eq!(receipt.status, "pending");
    let release = action("release");
    assert!(app
        .preview_merge_action(&id, &release)
        .unwrap()
        .blocked_reason
        .is_some());
    // Native Git recovery cannot remove the app-owned record; this reproduces the reported stale session.
    git(&repo, &["merge", "--abort"]);
    assert_eq!(app.merge_state(&id).unwrap().phase, "incomplete");
    fs::write(repo.join("file.txt"), "new staged work\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    fs::write(repo.join("file.txt"), "new unstaged work\n").unwrap();
    fs::write(repo.join("keep.txt"), "untracked work\n").unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    let marker = fs::read(repo.join(".git/gitnova-merge.json")).unwrap();
    let p = app.preview_merge_action(&id, &release).unwrap();
    assert!(p.blocked_reason.is_none());
    let archive = p.recovery_dir.unwrap().to_path().unwrap();
    assert!(!archive.exists());
    let done = app.apply_merge_action(&id, &release, &p.token).unwrap();
    assert_eq!(done.status, "succeeded", "{}", done.summary);
    assert_eq!(fs::read(archive).unwrap(), marker);
    assert!(!repo.join(".git/gitnova-merge.json").exists());
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "new unstaged work\n"
    );
    assert_eq!(
        fs::read_to_string(repo.join("keep.txt")).unwrap(),
        "untracked work\n"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(app.merge_state(&id).unwrap().phase, "none");
    let recovery = if let Some(PlannedOperation::Merge(plan)) = done.planned {
        plan.recovery_ref.unwrap()
    } else {
        panic!("Missing reviewed cleanup receipt")
    };
    assert_eq!(git(&repo, &["rev-parse", &recovery]), original);
    assert_eq!(
        git(&repo, &["rev-parse", &format!("{recovery}-incoming")]),
        source
    );
}

#[test]
fn unstage_operation_one_and_all_preserve_files_metadata_and_restaging() {
    let (_dir, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    fs::write(repo.join("new.txt"), "new incoming\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Add incoming file"]);
    let source = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["switch", "main"]);
    diverge(&repo, false);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    begin(&app, &id, &source);
    let metadata = ["MERGE_HEAD", "ORIG_HEAD", "MERGE_MSG", "gitnova-merge.json"]
        .map(|name| (name, fs::read(repo.join(".git").join(name)).unwrap()));
    fs::write(repo.join("file.txt"), "extra staged version\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    let old = git(&repo, &["rev-parse", ":file.txt"]);
    fs::write(repo.join("file.txt"), "working draft stays\n").unwrap();
    let mut one = action("unstage");
    one.path = Some(NativePath::new(Path::new("file.txt")));
    let p = app.preview_merge_action(&id, &one).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert_eq!(p.paths.len(), 1);
    let r = app.apply_merge_action(&id, &one, &p.token).unwrap();
    assert_eq!(r.status, "pending", "{}", r.summary);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "working draft stays\n"
    );
    assert_eq!(git(&repo, &["show", ":file.txt"]), "main");
    let reference = fs::read_to_string(
        p.recovery_dir
            .unwrap()
            .to_path()
            .unwrap()
            .join("recovery-ref"),
    )
    .unwrap();
    assert_eq!(
        git(&repo, &["rev-parse", &format!("{reference}:file.txt")]),
        old
    );
    let all = action("unstage");
    let p = app.preview_merge_action(&id, &all).unwrap();
    assert_eq!(p.paths.len(), 1);
    app.apply_merge_action(&id, &all, &p.token).unwrap();
    assert!(git(&repo, &["diff", "--cached", "--name-only"]).is_empty());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    for (name, bytes) in metadata {
        assert_eq!(fs::read(repo.join(".git").join(name)).unwrap(), bytes);
    }
    let patch = app
        .merge_file_scope(&id, &NativePath::new(Path::new("new.txt")), false)
        .unwrap();
    assert!(patch.diff.contains("+new incoming"));
    let p = app
        .preview_merge_action(&id, &resolution("working", None))
        .unwrap();
    assert!(p.blocked_reason.is_none());
    app.apply_merge_action(&id, &p.action, &p.token).unwrap();
    assert_eq!(git(&repo, &["show", ":file.txt"]), "working draft stays");
}
#[test]
fn unstage_operation_large_binary_stale_review_and_native_lock() {
    let (_dir, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    fs::write(repo.join("large.bin"), vec![0u8; 17 * 1024 * 1024]).unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Large binary"]);
    let source = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["switch", "main"]);
    diverge(&repo, false);
    begin(&app, &id, &source);
    let a = action("unstage");
    let p = app.preview_merge_action(&id, &a).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    fs::write(repo.join("other.txt"), "external staged\n").unwrap();
    git(&repo, &["add", "other.txt"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    assert!(app.apply_merge_action(&id, &a, &p.token).is_err());
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    fs::write(repo.join(".git/index.lock"), "external owner").unwrap();
    assert!(app
        .preview_merge_action(&id, &a)
        .unwrap()
        .blocked_reason
        .is_some());
    assert_eq!(
        fs::read_to_string(repo.join(".git/index.lock")).unwrap(),
        "external owner"
    );
    fs::remove_file(repo.join(".git/index.lock")).unwrap();
    let p = app.preview_merge_action(&id, &a).unwrap();
    let r = app.apply_merge_action(&id, &a, &p.token).unwrap();
    assert_eq!(r.status, "pending", "{}", r.summary);
    assert_eq!(
        fs::metadata(repo.join("large.bin")).unwrap().len(),
        17 * 1024 * 1024
    );
    assert!(repo.join(".git/MERGE_HEAD").exists());
    let reference = fs::read_to_string(
        p.recovery_dir
            .unwrap()
            .to_path()
            .unwrap()
            .join("recovery-ref"),
    )
    .unwrap();
    git(&repo, &["prune", "--expire", "now"]);
    assert_eq!(
        git(
            &repo,
            &["cat-file", "-s", &format!("{reference}:large.bin")]
        ),
        (17 * 1024 * 1024).to_string()
    );
}
#[test]
fn unstage_operation_refuses_unresolved_or_external_sessions() {
    let (_dir, app, repo, id) = setup();
    diverge(&repo, true);
    let source = git(&repo, &["rev-parse", "feature"]);
    begin(&app, &id, &source);
    let original = fs::read(repo.join(".git/index")).unwrap();
    let a = action("unstage");
    // A conflict-only index has no eligible staged selection, so no write is offered.
    let p = app.preview_merge_action(&id, &a).unwrap();
    assert!(p.blocked_reason.is_some());
    let receipt = app.apply_merge_action(&id, &a, &p.token).unwrap();
    assert_eq!(receipt.status, "failed");
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), original);
    fs::remove_file(repo.join(".git/gitnova-merge.json")).unwrap();
    assert!(app.preview_merge_action(&id, &a).is_err());
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), original);
}

#[test]
fn session_cleanup_after_external_completion_keeps_current_checkout_and_all_work() {
    for kind in ["merge", "cherryPick", "revert"] {
        let (_dir, app, repo, id) = setup();
        diverge(&repo, false);
        let source = git(
            &repo,
            &[
                "rev-parse",
                if kind == "revert" { "HEAD" } else { "feature" },
            ],
        );
        let mut r = request(&source);
        r.operation = kind.into();
        r.source_ref = None;
        let p = app.preview_merge(&id, &r).unwrap();
        assert!(p.blocked_reason.is_none(), "{kind}: {:?}", p.blocked_reason);
        let start = app.begin_merge(&id, &p.request, &p.token).unwrap();
        assert_eq!(start.status, "pending", "{kind}: {}", start.summary);
        git(&repo, &["commit", "-m", "Completed with external Git"]);
        let completed = git(&repo, &["rev-parse", "HEAD"]);
        git(&repo, &["switch", "-c", "continued-work"]);
        fs::write(repo.join("file.txt"), "new staged work\n").unwrap();
        git(&repo, &["add", "file.txt"]);
        fs::write(repo.join("file.txt"), "new unstaged work\n").unwrap();
        fs::write(repo.join("untracked.txt"), "private untracked work\n").unwrap();
        let index = fs::read(repo.join(".git/index")).unwrap();
        let refs = git(&repo, &["show-ref"]);
        let marker = fs::read(repo.join(".git/gitnova-merge.json")).unwrap();
        let state = app.merge_state(&id).unwrap();
        assert_eq!(state.phase, "incomplete");
        let release = action("release");
        let p = app.preview_merge_action(&id, &release).unwrap();
        assert!(p.blocked_reason.is_none(), "{kind}: {:?}", p.blocked_reason);
        assert!(p.paths.is_empty());
        assert!(repo.join(".git/gitnova-merge.json").exists());
        let archive = p.recovery_dir.unwrap().to_path().unwrap();
        let result = app.apply_merge_action(&id, &release, &p.token).unwrap();
        assert_eq!(result.status, "succeeded", "{kind}: {}", result.summary);
        assert_eq!(fs::read(archive).unwrap(), marker);
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]), completed);
        assert_eq!(git(&repo, &["branch", "--show-current"]), "continued-work");
        assert_eq!(git(&repo, &["show-ref"]), refs);
        assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
        assert_eq!(
            fs::read_to_string(repo.join("file.txt")).unwrap(),
            "new unstaged work\n"
        );
        assert_eq!(
            fs::read_to_string(repo.join("untracked.txt")).unwrap(),
            "private untracked work\n"
        );
        assert_eq!(app.merge_state(&id).unwrap().phase, "none");
    }
}

#[test]
fn session_cleanup_rejects_new_head_even_with_identical_index_and_rechecks_native_operations() {
    let (_dir, app, repo, id) = setup();
    diverge(&repo, false);
    let source = git(&repo, &["rev-parse", "feature"]);
    begin(&app, &id, &source);
    git(&repo, &["merge", "--abort"]);
    let release = action("release");
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let p = app.preview_merge_action(&id, &release).unwrap();
    assert!(p.blocked_reason.is_none());
    let index = fs::read(repo.join(".git/index")).unwrap();
    let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);
    let moved = git(
        &repo,
        &[
            "commit-tree",
            &tree,
            "-p",
            &original,
            "-m",
            "Same tree new HEAD",
        ],
    );
    git(&repo, &["update-ref", "refs/heads/main", &moved, &original]);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        app.apply_merge_action(&id, &release, &p.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    assert!(!p.recovery_dir.unwrap().to_path().unwrap().exists());
    let fresh = app.preview_merge_action(&id, &release).unwrap();
    assert!(fresh.blocked_reason.is_none());
    // A newly started native operation must not be hidden by closing an old journal.
    git(&repo, &["merge", "--no-ff", "--no-commit", "feature"]);
    assert!(app
        .preview_merge_action(&id, &release)
        .unwrap()
        .blocked_reason
        .is_some());
    assert!(app.apply_merge_action(&id, &release, &fresh.token).is_err());
    assert!(repo.join(".git/MERGE_HEAD").exists());
    assert!(repo.join(".git/gitnova-merge.json").exists());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), moved);
}
