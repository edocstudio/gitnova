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
#[test]
fn local_switch_is_previewed_and_updates_real_worktree() {
    let (_directory, app, repo, id) = setup();
    let index = fs::read(repo.join(".git/index")).unwrap();
    let refs = git(&repo, &["show-ref"]);
    let preview = app.preview_branch_switch(&id, "feature").unwrap();
    assert!(preview.blocked_reason.is_none());
    assert_eq!(preview.files[0].path.display, "file.txt");
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(git(&repo, &["show-ref"]), refs);
    let receipt = app.switch_branch(&id, "feature", &preview.token).unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(receipt.kind, "switch_branch");
    assert_eq!(git(&repo, &["branch", "--show-current"]), "feature");
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "feature\n"
    );
    assert_eq!(git(&repo, &["show-ref"]), refs);
    assert!(app
        .local_branches(&id)
        .unwrap()
        .iter()
        .any(|b| b.name == "feature" && b.current));
}
#[test]
fn dirty_and_stale_previews_do_not_discard_or_stash_work() {
    let (_directory, app, repo, id) = setup();
    let preview = app.preview_branch_switch(&id, "feature").unwrap();
    fs::write(repo.join("file.txt"), "uncommitted\n").unwrap();
    let blocked = app.preview_branch_switch(&id, "feature").unwrap();
    assert!(blocked.blocked_reason.is_some());
    let receipt = app.switch_branch(&id, "feature", &preview.token).unwrap();
    assert_eq!(receipt.status, "failed");
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "uncommitted\n"
    );
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
    assert!(!repo.join(".git/refs/stash").exists());
    git(&repo, &["add", "file.txt"]);
    let blocked = app.preview_branch_switch(&id, "feature").unwrap();
    assert!(blocked.blocked_reason.is_some());
    assert_eq!(
        app.switch_branch(&id, "feature", &preview.token)
            .unwrap()
            .status,
        "failed"
    );
    assert_eq!(git(&repo, &["show", ":file.txt"]), "uncommitted");
}
#[test]
fn moved_target_and_other_worktree_are_rejected() {
    let (directory, app, repo, id) = setup();
    let preview = app.preview_branch_switch(&id, "feature").unwrap();
    let main = git(&repo, &["rev-parse", "main"]);
    git(&repo, &["update-ref", "refs/heads/feature", &main]);
    assert_eq!(
        app.switch_branch(&id, "feature", &preview.token)
            .unwrap()
            .status,
        "failed"
    );
    let linked = directory.path().join("linked");
    git(
        &repo,
        &["worktree", "add", linked.to_str().unwrap(), "feature"],
    );
    let preview = app.preview_branch_switch(&id, "feature").unwrap();
    assert!(preview.blocked_reason.unwrap().contains("another worktree"));
    assert!(app
        .local_branches(&id)
        .unwrap()
        .iter()
        .any(|b| b.name == "feature" && b.checked_out_at.is_some()));
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
}
#[test]
fn native_index_lock_and_ignored_file_overwrite_are_protected() {
    let (_directory, app, repo, id) = setup();
    let preview = app.preview_branch_switch(&id, "feature").unwrap();
    fs::write(repo.join(".git/index.lock"), "external").unwrap();
    assert_eq!(
        app.switch_branch(&id, "feature", &preview.token)
            .unwrap()
            .status,
        "failed"
    );
    assert_eq!(
        fs::read_to_string(repo.join(".git/index.lock")).unwrap(),
        "external"
    );
    fs::remove_file(repo.join(".git/index.lock")).unwrap();
    fs::write(repo.join(".gitignore"), "local-secret.txt\n").unwrap();
    git(&repo, &["add", ".gitignore"]);
    git(&repo, &["commit", "-m", "Ignore local secret"]);
    git(&repo, &["switch", "feature"]);
    fs::write(repo.join("local-secret.txt"), "tracked on feature\n").unwrap();
    git(&repo, &["add", "local-secret.txt"]);
    git(&repo, &["commit", "-m", "Add feature file"]);
    git(&repo, &["switch", "main"]);
    fs::write(repo.join("local-secret.txt"), "private local content\n").unwrap();
    let preview = app.preview_branch_switch(&id, "feature").unwrap();
    let receipt = app.switch_branch(&id, "feature", &preview.token).unwrap();
    assert_eq!(receipt.status, "failed", "{}", receipt.summary);
    assert_eq!(
        fs::read_to_string(repo.join("local-secret.txt")).unwrap(),
        "private local content\n"
    );
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
}
#[test]
fn submodule_pointer_changes_and_revision_expressions_are_not_implicit() {
    let (_directory, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    let oid = git(&repo, &["rev-parse", "HEAD"]);
    git(
        &repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{oid},module"),
        ],
    );
    git(&repo, &["commit", "-m", "Add gitlink"]);
    git(&repo, &["switch", "main"]);
    let preview = app.preview_branch_switch(&id, "feature").unwrap();
    assert!(preview.blocked_reason.unwrap().contains("submodule"));
    assert_eq!(
        app.switch_branch(&id, "feature", &preview.token)
            .unwrap()
            .status,
        "failed"
    );
    assert!(app.preview_branch_switch(&id, "@{-1}").is_err());
    assert!(app.preview_branch_switch(&id, "--discard-changes").is_err());
}
#[cfg(unix)]
#[test]
fn checkout_hook_does_not_run_during_preview() {
    use std::os::unix::fs::PermissionsExt;
    let (_directory, app, repo, id) = setup();
    let hook = repo.join(".git/hooks/post-checkout");
    fs::write(&hook, "#!/bin/sh\nprintf checked > checkout-marker\n").unwrap();
    fs::set_permissions(hook, fs::Permissions::from_mode(0o755)).unwrap();
    let preview = app.preview_branch_switch(&id, "feature").unwrap();
    assert!(!repo.join("checkout-marker").exists());
    let receipt = app.switch_branch(&id, "feature", &preview.token).unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert!(repo.join("checkout-marker").exists());
}

#[test]
fn assume_unchanged_does_not_hide_uncommitted_work_from_switch_safety() {
    let (_directory, app, repo, id) = setup();
    git(&repo, &["update-index", "--assume-unchanged", "file.txt"]);
    fs::write(repo.join("file.txt"), "hidden uncommitted edit\n").unwrap();
    let preview = app.preview_branch_switch(&id, "feature").unwrap();
    assert!(preview
        .blocked_reason
        .as_ref()
        .unwrap()
        .contains("assume-unchanged"));
    assert_eq!(
        app.switch_branch(&id, "feature", &preview.token)
            .unwrap()
            .status,
        "failed"
    );
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "hidden uncommitted edit\n"
    );
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
}

#[test]
fn parent_switch_reviews_gitlink_changes_and_preserves_child_checkout_and_unpushed_commits() {
    let (directory, app, repo, id) = setup();
    let source = directory.path().join("module-source");
    fs::create_dir(&source).unwrap();
    git(&source, &["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Fixture"),
        ("user.email", "fixture@example.invalid"),
        ("commit.gpgsign", "false"),
    ] {
        git(&source, &["config", key, value]);
    }
    fs::write(source.join("child.txt"), "first\n").unwrap();
    git(&source, &["add", "."]);
    git(&source, &["commit", "-m", "First"]);
    let first = git(&source, &["rev-parse", "HEAD"]);
    fs::write(source.join("child.txt"), "second\n").unwrap();
    git(&source, &["commit", "-am", "Second"]);
    let second = git(&source, &["rev-parse", "HEAD"]);
    git(
        &repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--",
            source.to_str().unwrap(),
            "module",
        ],
    );
    let child = repo.join("module");
    git(&child, &["switch", "--detach", &first]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Record first module version"]);
    git(&repo, &["switch", "-c", "with-module"]);
    git(&child, &["switch", "--detach", &second]);
    git(&repo, &["add", "module"]);
    git(&repo, &["commit", "-m", "Record second module version"]);
    git(&repo, &["switch", "--no-recurse-submodules", "main"]);
    git(&child, &["switch", "--detach", &first]);
    git(&repo, &["config", "submodule.recurse", "true"]);
    git(
        &repo,
        &[
            "config",
            "submodule.module.update",
            "!touch update-must-not-run",
        ],
    );
    let preview = app.preview_branch_switch(&id, "with-module").unwrap();
    assert!(
        preview.blocked_reason.is_none(),
        "{:?}",
        preview.blocked_reason
    );
    assert_eq!(preview.submodules.len(), 1);
    assert_eq!(preview.submodules[0].source_oid, first);
    assert_eq!(preview.submodules[0].target_oid, second);
    assert_eq!(
        preview.submodules[0].checked_out_oid.as_deref(),
        Some(first.as_str())
    );
    assert!(!repo.join("update-must-not-run").exists());
    assert!(!child.join("update-must-not-run").exists());
    let receipt = app
        .switch_branch(&id, "with-module", &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), first);
    let sub = app.submodules(&id).unwrap().remove(0);
    assert_eq!(sub.index_oid, second);
    assert_eq!(sub.checked_out_oid.as_deref(), Some(first.as_str()));
    assert!(app.status(&id).unwrap().dirty);
    let back = app.preview_branch_switch(&id, "main").unwrap();
    assert!(back.blocked_reason.is_none(), "{:?}", back.blocked_reason);
    assert_eq!(
        app.switch_branch(&id, "main", &back.token).unwrap().status,
        "succeeded"
    );
    for (key, value) in [
        ("user.name", "Fixture"),
        ("user.email", "fixture@example.invalid"),
        ("commit.gpgsign", "false"),
    ] {
        git(&child, &["config", key, value]);
    }
    fs::write(child.join("unpublished.txt"), "local child commit\n").unwrap();
    git(&child, &["add", "."]);
    git(&child, &["commit", "-m", "Unpublished child work"]);
    let unpublished = git(&child, &["rev-parse", "HEAD"]);
    let preview = app.preview_branch_switch(&id, "with-module").unwrap();
    assert!(preview.blocked_reason.is_none());
    assert_eq!(
        app.switch_branch(&id, "with-module", &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), unpublished);
    fs::write(child.join("child.txt"), "dirty child\n").unwrap();
    assert!(app
        .preview_branch_switch(&id, "main")
        .unwrap()
        .blocked_reason
        .is_some());
    assert_eq!(
        fs::read_to_string(child.join("child.txt")).unwrap(),
        "dirty child\n"
    );
}
