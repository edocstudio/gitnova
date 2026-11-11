use gitnova_core::Application;
use std::{fs, path::Path, process::Command};
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Undo Test",
            "-c",
            "user.email=undo@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn setup() -> (tempfile::TempDir, Application, String) {
    let t = tempfile::tempdir().unwrap();
    git(t.path(), &["init", "-b", "main"]);
    fs::write(t.path().join("file.txt"), "base\n").unwrap();
    git(t.path(), &["add", "."]);
    git(t.path(), &["commit", "-m", "Base"]);
    let app = Application::new(t.path().join("app/state.json")).unwrap();
    // Exclude fixture app metadata so it never becomes working content.
    fs::write(t.path().join(".git/info/exclude"), "app/\n").unwrap();
    let c = app.add_repository(t.path()).unwrap();
    let id = c.workspaces[0].selected_id.clone().unwrap();
    (t, app, id)
}
#[test]
fn undo_and_redo_tip_preserve_index_files_and_recovery() {
    let (t, app, id) = setup();
    let root = t.path();
    let base = git(root, &["rev-parse", "HEAD"]);
    fs::write(root.join("file.txt"), "updated\n").unwrap();
    git(root, &["commit", "-am", "Edit"]);
    let tip = git(root, &["rev-parse", "HEAD"]);
    let index = fs::read(root.join(".git/index")).unwrap();
    let p = app.preview_commit_history(&id, false).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    let r = app.apply_commit_history(&id, false, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(git(root, &["rev-parse", "HEAD"]), base);
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(root.join("file.txt")).unwrap(),
        "updated\n"
    );
    assert_eq!(git(root, &["status", "--porcelain"]), "M  file.txt");
    assert_eq!(git(root, &["rev-parse", &p.recovery_ref]), tip);
    let p = app.preview_commit_history(&id, true).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    let r = app.apply_commit_history(&id, true, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(git(root, &["rev-parse", "HEAD"]), tip);
    assert_eq!(git(root, &["status", "--porcelain"]), "");
    assert!(app.preview_commit_history(&id, true).is_err());
}
#[test]
fn undo_root_and_redo_restore_original_commit() {
    let (t, app, id) = setup();
    let tip = git(t.path(), &["rev-parse", "HEAD"]);
    let p = app.preview_commit_history(&id, false).unwrap();
    assert!(p.parent_oid.is_none());
    let r = app.apply_commit_history(&id, false, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(git(t.path(), &["symbolic-ref", "HEAD"]), "refs/heads/main");
    assert_eq!(git(t.path(), &["status", "--porcelain"]), "A  file.txt");
    let p = app.preview_commit_history(&id, true).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    assert_eq!(
        app.apply_commit_history(&id, true, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(t.path(), &["rev-parse", "HEAD"]), tip);
}
#[test]
fn edits_stale_reviews_and_published_tips_are_blocked() {
    let (t, app, id) = setup();
    let root = t.path();
    git(root, &["commit", "--allow-empty", "-m", "Tip"]);
    let p = app.preview_commit_history(&id, false).unwrap();
    fs::write(root.join("file.txt"), "dirty").unwrap();
    assert!(app
        .preview_commit_history(&id, false)
        .unwrap()
        .blocked_reason
        .is_some());
    assert_ne!(
        app.apply_commit_history(&id, false, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    git(root, &["restore", "file.txt"]);
    let p = app.preview_commit_history(&id, false).unwrap();
    assert_eq!(
        app.apply_commit_history(&id, false, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    fs::write(root.join("file.txt"), "new edit").unwrap();
    let p = app.preview_commit_history(&id, true).unwrap();
    assert!(p.blocked_reason.is_some());
    assert_ne!(
        app.apply_commit_history(&id, true, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    git(root, &["restore", "file.txt"]);
    let p = app.preview_commit_history(&id, true).unwrap();
    assert_eq!(
        app.apply_commit_history(&id, true, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    git(root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    assert!(app
        .preview_commit_history(&id, false)
        .unwrap()
        .blocked_reason
        .unwrap()
        .contains("remote"));
    git(root, &["update-ref", "-d", "refs/remotes/origin/main"]);
    git(root, &["tag", "v1"]);
    assert!(app
        .preview_commit_history(&id, false)
        .unwrap()
        .blocked_reason
        .is_some());
}
#[test]
fn merge_undo_uses_first_parent_and_restores_both_parents_on_redo() {
    let (t, app, id) = setup();
    let root = t.path();
    let base = git(root, &["rev-parse", "HEAD"]);
    git(root, &["switch", "-c", "feature"]);
    fs::write(root.join("feature.txt"), "feature").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "Feature"]);
    git(root, &["switch", "main"]);
    git(root, &["merge", "--no-ff", "-m", "Merge", "feature"]);
    let tip = git(root, &["rev-parse", "HEAD"]);
    let parents = git(root, &["show", "-s", "--format=%P", "HEAD"]);
    let p = app.preview_commit_history(&id, false).unwrap();
    assert_eq!(p.parent_oid.as_deref(), Some(base.as_str()));
    assert_eq!(
        app.apply_commit_history(&id, false, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert!(root.join("feature.txt").exists());
    let p = app.preview_commit_history(&id, true).unwrap();
    assert_eq!(
        app.apply_commit_history(&id, true, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(root, &["rev-parse", "HEAD"]), tip);
    assert_eq!(git(root, &["show", "-s", "--format=%P", "HEAD"]), parents);
}
#[test]
fn native_locks_pending_operations_and_staged_edits_never_move_history() {
    let (t, app, id) = setup();
    let root = t.path();
    git(root, &["commit", "--allow-empty", "-m", "Tip"]);
    let tip = git(root, &["rev-parse", "HEAD"]);
    let p = app.preview_commit_history(&id, false).unwrap();
    fs::write(root.join(".git/index.lock"), "external owner").unwrap();
    assert_ne!(
        app.apply_commit_history(&id, false, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        fs::read_to_string(root.join(".git/index.lock")).unwrap(),
        "external owner"
    );
    fs::remove_file(root.join(".git/index.lock")).unwrap();
    fs::write(root.join(".git/MERGE_HEAD"), &tip).unwrap();
    assert!(app.preview_commit_history(&id, false).is_err());
    fs::remove_file(root.join(".git/MERGE_HEAD")).unwrap();
    fs::write(root.join("file.txt"), "staged edit").unwrap();
    git(root, &["add", "file.txt"]);
    assert!(app
        .preview_commit_history(&id, false)
        .unwrap()
        .blocked_reason
        .is_some());
    assert_ne!(
        app.apply_commit_history(&id, false, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(root, &["rev-parse", "HEAD"]), tip);
}
#[test]
fn redo_survives_application_restart_but_not_index_edits_or_a_new_commit() {
    let (t, app, id) = setup();
    let root = t.path();
    git(root, &["commit", "--allow-empty", "-m", "Tip"]);
    let p = app.preview_commit_history(&id, false).unwrap();
    assert_eq!(
        app.apply_commit_history(&id, false, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    drop(app);
    let app = Application::new(root.join("app/state.json")).unwrap();
    let p = app.preview_commit_history(&id, true).unwrap();
    assert!(p.blocked_reason.is_none());
    fs::write(root.join("file.txt"), "staged edit").unwrap();
    git(root, &["add", "file.txt"]);
    assert!(app
        .preview_commit_history(&id, true)
        .unwrap()
        .blocked_reason
        .is_some());
    assert_ne!(
        app.apply_commit_history(&id, true, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    git(root, &["commit", "-m", "New commit"]);
    assert!(app
        .preview_commit_history(&id, true)
        .unwrap()
        .blocked_reason
        .is_some());
}
#[cfg(unix)]
#[test]
fn undo_does_not_execute_repository_ref_hooks() {
    use std::os::unix::fs::PermissionsExt;
    let (t, app, id) = setup();
    let root = t.path();
    let hook = root.join(".git/hooks/reference-transaction");
    fs::write(&hook, "#!/bin/sh\nexit 99\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let p = app.preview_commit_history(&id, false).unwrap();
    assert_eq!(
        app.apply_commit_history(&id, false, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let p = app.preview_commit_history(&id, true).unwrap();
    assert_eq!(
        app.apply_commit_history(&id, true, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
}
#[test]
fn linked_checkout_redo_is_isolated_from_the_main_checkout() {
    let (t, app, id) = setup();
    let root = t.path();
    let main = git(root, &["rev-parse", "HEAD"]);
    let main_index = fs::read(root.join(".git/index")).unwrap();
    // Keep the linked checkout under a separately owned temporary directory.
    let linked_temp = tempfile::tempdir().unwrap();
    let linked = linked_temp.path().join("checkout");
    git(
        root,
        &["worktree", "add", "-b", "other", linked.to_str().unwrap()],
    );
    fs::write(linked.join("file.txt"), "linked edit\n").unwrap();
    git(&linked, &["commit", "-am", "Linked tip"]);
    let c = app.add_repository(&linked).unwrap();
    let linked_id = c.workspaces[0].selected_id.as_ref().unwrap();
    let p = app.preview_commit_history(linked_id, false).unwrap();
    assert_eq!(
        app.apply_commit_history(linked_id, false, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert!(app.preview_commit_history(&id, true).is_err());
    let p = app.preview_commit_history(linked_id, true).unwrap();
    assert_eq!(
        app.apply_commit_history(linked_id, true, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(root, &["rev-parse", "HEAD"]), main);
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), main_index);
    assert_eq!(fs::read_to_string(root.join("file.txt")).unwrap(), "base\n");
}
