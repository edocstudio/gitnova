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

fn discard_request(paths: &[&str], scope: DiscardScope) -> DiscardRequest {
    DiscardRequest {
        paths: paths
            .iter()
            .map(|p| NativePath::new(Path::new(p)))
            .collect(),
        scope,
        id: String::new(),
    }
}

#[test]
fn empty_pending_merge_after_discard_blocks_pop_until_reviewed_abort_then_restores_saved_work() {
    let (_dir, app, repo, id) = setup();
    diverge(&repo, false);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let source = git(&repo, &["rev-parse", "feature"]);
    fs::write(repo.join("saved.txt"), "saved before merge\n").unwrap();
    git(
        &repo,
        &["stash", "push", "--include-untracked", "-m", "Saved work"],
    );
    let saved = app.stashes(&id).unwrap().entries[0].clone();
    begin(&app, &id, &source);
    let state = app.merge_state(&id).unwrap();
    let discard = app
        .preview_operation_discard(
            &id,
            &discard_request(&["file.txt"], DiscardScope::All),
            &state.token,
        )
        .unwrap();
    assert!(discard.blocked_reason.is_none());
    assert_eq!(
        app.discard_operation_changes(&id, &discard.request, &discard.token, &state.token)
            .unwrap()
            .status,
        "pending"
    );
    assert!(app.working_changes(&id).unwrap().files.is_empty());
    assert_eq!(app.merge_state(&id).unwrap().phase, "pending");
    let pop = StashRequest {
        kind: StashKind::Pop,
        index: Some(saved.index),
        oid: Some(saved.oid.clone()),
        message: String::new(),
        include_untracked: false,
        restore_index: false,
        id: String::new(),
    };
    let blocked = app.preview_stash(&id, &pop).unwrap();
    assert!(blocked
        .blocked_reason
        .as_deref()
        .is_some_and(|r| r.contains("existing merge")));
    assert_ne!(
        app.apply_stash(&id, &blocked.request, &blocked.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(app.stashes(&id).unwrap().entries[0].oid, saved.oid);
    assert!(!repo.join("saved.txt").exists());
    let abort = app.preview_merge_action(&id, &action("abort")).unwrap();
    assert!(abort.blocked_reason.is_none(), "{:?}", abort.blocked_reason);
    // Preview alone must not clear the operation or enable Pop.
    assert!(repo.join(".git/MERGE_HEAD").exists());
    assert!(app
        .preview_stash(&id, &pop)
        .unwrap()
        .blocked_reason
        .is_some());
    assert_eq!(
        app.apply_merge_action(&id, &abort.action, &abort.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(app.merge_state(&id).unwrap().phase, "none");
    // A preview taken before abort cannot be reused afterwards.
    assert_ne!(
        app.apply_stash(&id, &blocked.request, &blocked.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let ready = app.preview_stash(&id, &pop).unwrap();
    assert!(ready.blocked_reason.is_none(), "{:?}", ready.blocked_reason);
    assert_eq!(
        app.apply_stash(&id, &ready.request, &ready.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        fs::read_to_string(repo.join("saved.txt")).unwrap(),
        "saved before merge\n"
    );
    assert!(app.stashes(&id).unwrap().entries.is_empty());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&repo, &["rev-parse", "feature"]), source);
}
#[test]
fn operation_discard_keeps_pending_metadata_staging_and_unselected_work() {
    let (_dir, app, repo, id) = setup();
    diverge(&repo, false);
    let source = git(&repo, &["rev-parse", "feature"]);
    begin(&app, &id, &source);
    fs::write(repo.join("file.txt"), "staged content\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    fs::write(repo.join("file.txt"), "working content\n").unwrap();
    fs::write(repo.join("loose.txt"), "selected loose\n").unwrap();
    fs::write(repo.join("keep.txt"), "keep this\n").unwrap();
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    let metadata = ["MERGE_HEAD", "ORIG_HEAD", "MERGE_MSG", "gitnova-merge.json"]
        .map(|n| (n, fs::read(repo.join(".git").join(n)).unwrap()));
    let state = app.merge_state(&id).unwrap();
    let request = discard_request(&["file.txt", "loose.txt"], DiscardScope::Unstaged);
    assert!(app.preview_discard(&id, &request).is_err());
    let p = app
        .preview_operation_discard(&id, &request, &state.token)
        .unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert!(repo.join("loose.txt").exists());
    let r = app
        .discard_operation_changes(&id, &p.request, &p.token, &state.token)
        .unwrap();
    assert_eq!(r.status, "pending", "{}", r.summary);
    assert_eq!(r.merge.unwrap().phase, "pending");
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "staged content\n"
    );
    assert!(!repo.join("loose.txt").exists());
    assert_eq!(
        fs::read_to_string(repo.join("keep.txt")).unwrap(),
        "keep this\n"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    for (n, bytes) in metadata {
        assert_eq!(fs::read(repo.join(".git").join(n)).unwrap(), bytes);
    }
    let recovery = p.recovery_dir.to_path().unwrap();
    assert_eq!(
        fs::read_to_string(recovery.join("work-0")).unwrap(),
        "working content\n"
    );
    assert_eq!(
        fs::read_to_string(recovery.join("staged-0")).unwrap(),
        "staged content\n"
    );
    let all = discard_request(&["file.txt"], DiscardScope::All);
    let state = app.merge_state(&id).unwrap();
    let p = app
        .preview_operation_discard(&id, &all, &state.token)
        .unwrap();
    let r = app
        .discard_operation_changes(&id, &p.request, &p.token, &state.token)
        .unwrap();
    assert_eq!(r.status, "pending");
    assert_eq!(git(&repo, &["show", ":file.txt"]), "main");
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "main\n");
    assert!(repo.join(".git/MERGE_HEAD").exists());
}
#[test]
fn operation_discard_streams_large_unstaged_files_after_unstage_and_rejects_stale_context() {
    let (_dir, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    fs::write(repo.join("video.bin"), vec![3u8; 20 * 1024 * 1024]).unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Large incoming file"]);
    let source = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["switch", "main"]);
    diverge(&repo, false);
    begin(&app, &id, &source);
    let stale = app.merge_state(&id).unwrap().token;
    let unstage = action("unstage");
    let p = app.preview_merge_action(&id, &unstage).unwrap();
    app.apply_merge_action(&id, &unstage, &p.token).unwrap();
    let request = discard_request(&["video.bin", "file.txt"], DiscardScope::All);
    assert!(app
        .preview_operation_discard(&id, &request, &stale)
        .is_err());
    let state = app.merge_state(&id).unwrap();
    let p = app
        .preview_operation_discard(&id, &request, &state.token)
        .unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    fs::write(repo.join("file.txt"), "late edit\n").unwrap();
    let r = app.discard_operation_changes(&id, &p.request, &p.token, &state.token);
    assert!(r.is_err());
    assert!(repo.join("video.bin").exists());
    let p = app
        .preview_operation_discard(&id, &request, &state.token)
        .unwrap();
    let r = app
        .discard_operation_changes(&id, &p.request, &p.token, &state.token)
        .unwrap();
    assert_eq!(r.status, "pending", "{}", r.summary);
    assert!(!repo.join("video.bin").exists());
    assert_eq!(
        fs::metadata(p.recovery_dir.to_path().unwrap().join("work-0"))
            .unwrap()
            .len(),
        20 * 1024 * 1024
    );
    assert_eq!(
        fs::read(p.recovery_dir.to_path().unwrap().join("work-0")).unwrap()[20 * 1024 * 1024 - 1],
        3
    );
    assert!(repo.join(".git/MERGE_HEAD").exists());
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "main\n");
}
#[test]
fn operation_discard_refuses_conflicts_and_external_operations() {
    let (_dir, app, repo, id) = setup();
    diverge(&repo, true);
    let source = git(&repo, &["rev-parse", "feature"]);
    begin(&app, &id, &source);
    let request = discard_request(&["file.txt"], DiscardScope::All);
    let state = app.merge_state(&id).unwrap();
    assert!(app
        .preview_operation_discard(&id, &request, &state.token)
        .is_err());
    let bytes = fs::read(repo.join(".git/index")).unwrap();
    fs::remove_file(repo.join(".git/gitnova-merge.json")).unwrap();
    let state = app.merge_state(&id).unwrap();
    assert!(app
        .preview_operation_discard(&id, &request, &state.token)
        .is_err());
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), bytes);
}

#[test]
fn progress_reports_real_phases_and_native_metadata_changes_stop_the_fast_path() {
    use std::sync::{Arc, Mutex};
    let (_dir, app, repo, id) = setup();
    diverge(&repo, false);
    let source = git(&repo, &["rev-parse", "feature"]);
    begin(&app, &id, &source);
    fs::write(repo.join("one.txt"), "one original\n").unwrap();
    fs::write(repo.join("two.txt"), "two original\n").unwrap();
    let request = discard_request(&["one.txt", "two.txt"], DiscardScope::All);
    let state = app.merge_state(&id).unwrap();
    let p = app
        .preview_operation_discard(&id, &request, &state.token)
        .unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let collected = events.clone();
    let moved = repo.clone();
    let r = gitnova_core::with_progress(
        move |event| {
            if event.phase == "discarding" && event.completed == 1 {
                fs::write(moved.join(".git/MERGE_MSG"), "external metadata change\n").unwrap();
            }
            collected.lock().unwrap().push(event);
        },
        || app.discard_operation_changes(&id, &p.request, &p.token, &state.token),
    )
    .unwrap();
    assert_eq!(r.status, "unknown");
    assert!(!repo.join("one.txt").exists());
    assert_eq!(
        fs::read_to_string(repo.join("two.txt")).unwrap(),
        "two original\n"
    );
    assert!(repo.join(".git/MERGE_HEAD").exists());
    let events = events.lock().unwrap();
    for phase in ["checking", "backing-up", "discarding"] {
        assert!(events.iter().any(|p| p.phase == phase));
    }
    assert!(events.iter().all(|p| p.completed <= p.total));
    assert!(events
        .iter()
        .any(|p| p.phase == "backing-up" && p.completed == 2 && p.total == 2));
    assert_eq!(
        fs::read_to_string(p.recovery_dir.to_path().unwrap().join("work-1")).unwrap(),
        "two original\n"
    );
}
