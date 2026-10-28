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
use gitnova_domain::{GraphAction, GraphActionKind};
fn action(kind: GraphActionKind, oid: &str, name: &str) -> GraphAction {
    GraphAction {
        kind,
        oid: oid.into(),
        name: name.into(),
    }
}
#[test]
fn expanded_status_counts_files_in_untracked_directories_and_counts_renames_consistently() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("unborn");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "user.name", "GitNova Test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    git(&repo, &["config", "core.hooksPath", ".git/hooks"]);
    git(&repo, &["config", "status.showUntrackedFiles", "no"]);
    for i in 0..127 {
        let folder = repo.join(format!("bucket-{}", i % 12));
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(format!("file{i}.txt")), "first\n").unwrap();
    }
    let app = Application::new(temp.path().join("app/state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    let snapshot = app.snapshot(&id).unwrap();
    assert_eq!(snapshot.changes.status.changed_entries, 127);
    assert_eq!(snapshot.changes.files.len(), 127);
    assert_eq!(app.graph(&id, None).unwrap().status.changed_entries, 127);
    assert_eq!(app.status(&id).unwrap().changed_entries, 127);
    git(&repo, &["add", "bucket-0/file0.txt"]);
    assert_eq!(app.working_changes(&id).unwrap().files.len(), 127);
    assert_eq!(app.status(&id).unwrap().changed_entries, 127);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "All files"]);
    git(&repo, &["mv", "bucket-0/file0.txt", "renamed file.txt"]);
    fs::write(repo.join("renamed file.txt"), "first\nsecond\n").unwrap();
    assert_eq!(app.status(&id).unwrap().changed_entries, 2);
    let changed = app.working_changes(&id).unwrap();
    assert_eq!(changed.files.len(), 2);
    let renamed = changed
        .files
        .iter()
        .find(|f| f.path.display == "renamed file.txt")
        .unwrap();
    assert!(renamed.staged && renamed.unstaged);
}
#[test]
fn checkout_commit_then_create_branch_retains_detached_work_and_receipt_scope() {
    let (_temp, app, repo, id) = setup();
    let first = git(&repo, &["rev-parse", "main"]);
    let feature = git(&repo, &["rev-parse", "feature"]);
    let checkout = action(GraphActionKind::CheckoutCommit, &feature, "");
    let index = fs::read(repo.join(".git/index")).unwrap();
    let refs = git(&repo, &["show-ref"]);
    let preview = app.preview_graph_action(&id, &checkout).unwrap();
    assert!(preview.blocked_reason.is_none());
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(git(&repo, &["show-ref"]), refs);
    let receipt = app
        .apply_graph_action(&id, &checkout, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert!(app.status(&id).unwrap().branch.is_none());
    assert_eq!(receipt.graph_action.unwrap().oid, feature);
    fs::write(repo.join("file.txt"), "detached edit\n").unwrap();
    git(&repo, &["commit", "-am", "Detached work"]);
    let detached = git(&repo, &["rev-parse", "HEAD"]);
    let leave = action(GraphActionKind::CheckoutCommit, &first, "");
    assert!(app
        .preview_graph_action(&id, &leave)
        .unwrap()
        .blocked_reason
        .is_some());
    let branch = action(GraphActionKind::CreateBranch, &detached, "saved/detached");
    let review = app.preview_graph_action(&id, &branch).unwrap();
    assert!(review.blocked_reason.is_none());
    let receipt = app.apply_graph_action(&id, &branch, &review.token).unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(git(&repo, &["branch", "--show-current"]), "saved/detached");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), detached);
    assert_eq!(git(&repo, &["rev-parse", "main"]), first);
}
#[test]
fn graph_actions_reject_revision_expressions_existing_names_and_changed_review() {
    let (_temp, app, repo, id) = setup();
    let oid = git(&repo, &["rev-parse", "feature"]);
    for name in ["-force", "HEAD", "bad..name", "a@{x}", "refs/../escape"] {
        assert!(
            app.preview_graph_action(&id, &action(GraphActionKind::CreateBranch, &oid, name))
                .is_err(),
            "{name}"
        );
    }
    assert!(app
        .preview_graph_action(&id, &action(GraphActionKind::CheckoutCommit, "HEAD~1", ""))
        .is_err());
    let blob = git(&repo, &["rev-parse", "HEAD:file.txt"]);
    assert!(app
        .preview_graph_action(&id, &action(GraphActionKind::CheckoutCommit, &blob, ""))
        .is_err());
    let existing = action(GraphActionKind::CreateBranch, &oid, "main");
    let review = app.preview_graph_action(&id, &existing).unwrap();
    assert!(review.blocked_reason.is_some());
    assert_eq!(
        app.apply_graph_action(&id, &existing, &review.token)
            .unwrap()
            .status,
        "failed"
    );
    let create = action(GraphActionKind::CreateBranch, &oid, "fresh");
    let review = app.preview_graph_action(&id, &create).unwrap();
    git(&repo, &["branch", "racing-ref"]);
    let receipt = app.apply_graph_action(&id, &create, &review.token).unwrap();
    assert_eq!(receipt.status, "failed");
    assert!(receipt.summary.contains("Review"));
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
    assert!(!git(&repo, &["branch", "--list", "fresh"]).contains("fresh"));
    let changed_target = action(GraphActionKind::CheckoutCommit, &oid, "");
    assert_eq!(
        app.apply_graph_action(&id, &changed_target, &review.token)
            .unwrap()
            .status,
        "failed"
    );
}
#[test]
fn graph_checkout_respects_dirty_hidden_locked_and_ignored_work() {
    let (_temp, app, repo, id) = setup();
    let oid = git(&repo, &["rev-parse", "feature"]);
    let checkout = action(GraphActionKind::CheckoutCommit, &oid, "");
    fs::write(repo.join("file.txt"), "keep me\n").unwrap();
    assert!(app
        .preview_graph_action(&id, &checkout)
        .unwrap()
        .blocked_reason
        .is_some());
    git(&repo, &["update-index", "--assume-unchanged", "file.txt"]);
    assert!(app
        .preview_graph_action(&id, &checkout)
        .unwrap()
        .blocked_reason
        .is_some());
    git(
        &repo,
        &["update-index", "--no-assume-unchanged", "file.txt"],
    );
    fs::write(repo.join("file.txt"), "main\n").unwrap();
    fs::write(repo.join(".git/index.lock"), "held").unwrap();
    assert!(app
        .preview_graph_action(&id, &checkout)
        .unwrap()
        .blocked_reason
        .is_some());
    assert_eq!(
        fs::read_to_string(repo.join(".git/index.lock")).unwrap(),
        "held"
    );
    fs::remove_file(repo.join(".git/index.lock")).unwrap();
    git(&repo, &["switch", "feature"]);
    fs::write(repo.join("ignored.txt"), "tracked target\n").unwrap();
    git(&repo, &["add", "ignored.txt"]);
    git(&repo, &["commit", "-m", "Target contains ignored file"]);
    let target = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["switch", "main"]);
    fs::write(repo.join(".git/info/exclude"), "ignored.txt\n").unwrap();
    fs::write(repo.join("ignored.txt"), "keep ignored\n").unwrap();
    let checkout = action(GraphActionKind::CheckoutCommit, &target, "");
    let review = app.preview_graph_action(&id, &checkout).unwrap();
    assert!(review.blocked_reason.is_none());
    let receipt = app
        .apply_graph_action(&id, &checkout, &review.token)
        .unwrap();
    assert_ne!(receipt.status, "succeeded");
    assert_eq!(
        fs::read_to_string(repo.join("ignored.txt")).unwrap(),
        "keep ignored\n"
    );
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
}
#[test]
fn lightweight_tag_preserves_dirty_checkout_index_and_existing_tags() {
    let (_temp, app, repo, id) = setup();
    let oid = git(&repo, &["rev-parse", "feature"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(repo.join("file.txt"), "staged\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    fs::write(repo.join("file.txt"), "unstaged\n").unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    let tag = action(GraphActionKind::CreateTag, &oid, "v1.0");
    let review = app.preview_graph_action(&id, &tag).unwrap();
    assert!(review.blocked_reason.is_none());
    assert!(review.switch.is_none());
    assert!(git(&repo, &["tag", "--list"]).is_empty());
    let receipt = app.apply_graph_action(&id, &tag, &review.token).unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(git(&repo, &["rev-parse", "refs/tags/v1.0"]), oid);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "unstaged\n"
    );
    let overwrite = action(GraphActionKind::CreateTag, &head, "v1.0");
    assert!(app
        .preview_graph_action(&id, &overwrite)
        .unwrap()
        .blocked_reason
        .is_some());
    let race = action(GraphActionKind::CreateTag, &oid, "race");
    let review = app.preview_graph_action(&id, &race).unwrap();
    git(&repo, &["tag", "race"]);
    assert_eq!(
        app.apply_graph_action(&id, &race, &review.token)
            .unwrap()
            .status,
        "failed"
    );
    assert_eq!(git(&repo, &["rev-parse", "refs/tags/race"]), head);
}
#[cfg(unix)]
#[test]
fn checkout_preview_does_not_run_hooks_and_hook_movement_is_reported() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, app, repo, id) = setup();
    let oid = git(&repo, &["rev-parse", "feature"]);
    let hook = repo.join(".git/hooks/post-checkout");
    let marker = repo.join(".git/hook-marker");
    fs::write(
        &hook,
        "#!/bin/sh\nprintf ran > .git/hook-marker\ngit symbolic-ref HEAD refs/heads/main\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let checkout = action(GraphActionKind::CheckoutCommit, &oid, "");
    let review = app.preview_graph_action(&id, &checkout).unwrap();
    assert!(!marker.exists());
    let receipt = app
        .apply_graph_action(&id, &checkout, &review.token)
        .unwrap();
    assert!(marker.exists());
    assert_eq!(receipt.status, "unknown");
    assert!(receipt.recovery.is_some());
}

#[test]
fn detached_ancestor_retained_by_a_local_branch_can_be_left_without_losing_commits() {
    let (_temp, app, repo, id) = setup();
    let ancestor = git(&repo, &["rev-parse", "feature"]);
    git(&repo, &["switch", "feature"]);
    fs::write(repo.join("file.txt"), "descendant\n").unwrap();
    git(&repo, &["commit", "-am", "Retain ancestor"]);
    git(&repo, &["switch", "--detach", &ancestor]);
    let review = app.preview_branch_switch(&id, "main").unwrap();
    assert!(review.blocked_reason.is_none());
    let receipt = app.switch_branch(&id, "main", &review.token).unwrap();
    assert_eq!(receipt.status, "succeeded");
    assert_eq!(git(&repo, &["show", "feature:file.txt"]), "descendant");
}
