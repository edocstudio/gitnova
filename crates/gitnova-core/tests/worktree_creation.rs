use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Worktree Test",
            "-c",
            "user.email=worktree@example.invalid",
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
    String::from_utf8_lossy(&out.stdout).trim().into()
}
fn setup() -> (tempfile::TempDir, Application, PathBuf, String) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("source");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    fs::write(repo.join("file.txt"), "base\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Base"]);
    // Isolate native confirmed hooks from the developer's global configuration too.
    git(&repo, &["config", "core.hooksPath", "/dev/null"]);
    let app = Application::new(dir.path().join("app/state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    (dir, app, repo, id)
}
fn request(
    repo: &Path,
    path: &Path,
    mode: WorktreeCreateMode,
    branch: &str,
) -> WorktreeCreateRequest {
    WorktreeCreateRequest {
        oid: git(repo, &["rev-parse", "HEAD"]),
        destination: path.to_string_lossy().into(),
        mode,
        branch: branch.into(),
    }
}
fn apply(app: &Application, id: &str, r: &WorktreeCreateRequest) -> OperationReceipt {
    let p = app.preview_create_worktree(id, r).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let result = app.create_worktree(id, r, &p.token).unwrap();
    assert_eq!(
        result.status, "succeeded",
        "{} {:?}",
        result.summary, result.recovery
    );
    result
}
#[test]
fn new_branch_unicode_worktree_preserves_dirty_source_and_has_shared_identity() {
    let (dir, app, repo, id) = setup();
    let target = dir.path().join("งาน แยก");
    fs::write(repo.join("file.txt"), "staged\n").unwrap();
    git(&repo, &["add", "."]);
    fs::write(repo.join("file.txt"), "working\n").unwrap();
    fs::write(repo.join("new.txt"), "draft\n").unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let r = request(&repo, &target, WorktreeCreateMode::NewBranch, "feature/แยก");
    let preview = app.preview_create_worktree(&id, &r).unwrap();
    assert_eq!(preview.files, 1);
    assert!(!target.exists());
    let receipt = apply(&app, &id, &r);
    assert!(matches!(
        receipt.planned,
        Some(PlannedOperation::WorktreeCreate(_))
    ));
    assert_eq!(git(&target, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&target, &["branch", "--show-current"]), "feature/แยก");
    assert_eq!(
        fs::read_to_string(target.join("file.txt")).unwrap(),
        "base\n"
    );
    assert!(!target.join("new.txt").exists());
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "working\n"
    );
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
    let catalog = app.open_related(&id, &preview.destination).unwrap();
    let memberships = &catalog.workspaces[0].memberships;
    assert_eq!(memberships.len(), 2);
    assert_eq!(memberships[0].repository_id, memberships[1].repository_id);
    assert_ne!(memberships[0].worktree_id, memberships[1].worktree_id);
}
#[test]
fn existing_branch_and_detached_modes_never_infer_or_reset_branches() {
    let (dir, app, repo, id) = setup();
    git(&repo, &["branch", "available"]);
    apply(
        &app,
        &id,
        &request(
            &repo,
            &dir.path().join("existing"),
            WorktreeCreateMode::ExistingBranch,
            "available",
        ),
    );
    let blocked = app
        .preview_create_worktree(
            &id,
            &request(
                &repo,
                &dir.path().join("occupied"),
                WorktreeCreateMode::ExistingBranch,
                "available",
            ),
        )
        .unwrap();
    assert!(blocked
        .blocked_reason
        .unwrap()
        .contains("already checked out"));
    apply(
        &app,
        &id,
        &request(
            &repo,
            &dir.path().join("detached"),
            WorktreeCreateMode::Detached,
            "",
        ),
    );
    assert!(git(&dir.path().join("detached"), &["branch", "--show-current"]).is_empty());
    assert_eq!(
        git(&repo, &["branch", "--format=%(refname:short)"]),
        "available\nmain"
    );
}
#[test]
fn destination_and_branch_changes_invalidate_confirmation_without_cleanup() {
    let (dir, app, repo, id) = setup();
    let target = dir.path().join("new");
    let r = request(&repo, &target, WorktreeCreateMode::NewBranch, "topic");
    let p = app.preview_create_worktree(&id, &r).unwrap();
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep"), "do not delete").unwrap();
    let receipt = app.create_worktree(&id, &r, &p.token).unwrap();
    assert_eq!(receipt.status, "failed");
    assert!(receipt.summary.contains("changed"));
    assert_eq!(
        fs::read_to_string(target.join("keep")).unwrap(),
        "do not delete"
    );
    let r = request(
        &repo,
        &dir.path().join("other"),
        WorktreeCreateMode::NewBranch,
        "topic",
    );
    let p = app.preview_create_worktree(&id, &r).unwrap();
    git(&repo, &["branch", "topic"]);
    assert_eq!(
        app.create_worktree(&id, &r, &p.token).unwrap().status,
        "failed"
    );
    assert!(!dir.path().join("other").exists());
}
#[test]
fn existing_paths_nested_repositories_and_native_locks_are_refused() {
    let (dir, app, repo, id) = setup();
    for target in [repo.join("nested"), repo.join(".git/unsafe"), repo.clone()] {
        let r = request(&repo, &target, WorktreeCreateMode::Detached, "");
        let p = app.preview_create_worktree(&id, &r).unwrap();
        assert!(p.blocked_reason.is_some());
        assert_eq!(
            app.create_worktree(&id, &r, &p.token).unwrap().status,
            "failed"
        );
    }
    fs::write(repo.join(".git/index.lock"), "external").unwrap();
    let r = request(
        &repo,
        &dir.path().join("locked"),
        WorktreeCreateMode::Detached,
        "",
    );
    let p = app.preview_create_worktree(&id, &r).unwrap();
    assert!(p.blocked_reason.is_some());
    assert_eq!(
        app.create_worktree(&id, &r, &p.token).unwrap().status,
        "failed"
    );
    assert_eq!(
        fs::read_to_string(repo.join(".git/index.lock")).unwrap(),
        "external"
    );
}
#[test]
fn branch_move_and_configuration_changes_are_stale() {
    let (dir, app, repo, id) = setup();
    git(&repo, &["branch", "available"]);
    let r = request(
        &repo,
        &dir.path().join("new"),
        WorktreeCreateMode::ExistingBranch,
        "available",
    );
    let p = app.preview_create_worktree(&id, &r).unwrap();
    git(
        &repo,
        &[
            "config",
            "branch.available.description",
            "changed after review",
        ],
    );
    assert_eq!(
        app.create_worktree(&id, &r, &p.token).unwrap().status,
        "failed"
    );
    assert!(!dir.path().join("new").exists());
    let p = app.preview_create_worktree(&id, &r).unwrap();
    let moved = git(
        &repo,
        &[
            "commit-tree",
            "HEAD^{tree}",
            "-p",
            "HEAD",
            "-m",
            "Moved elsewhere",
        ],
    );
    git(&repo, &["update-ref", "refs/heads/available", &moved]);
    assert_eq!(
        app.create_worktree(&id, &r, &p.token).unwrap().status,
        "failed"
    );
    assert!(!dir.path().join("new").exists());
    assert!(app
        .preview_create_worktree(
            &id,
            &WorktreeCreateRequest {
                branch: "HEAD".into(),
                ..r
            }
        )
        .is_err());
}
#[cfg(unix)]
#[test]
fn symlink_destination_and_replaced_parent_are_protected() {
    use std::os::unix::fs::symlink;
    let (dir, app, repo, id) = setup();
    let alias = dir.path().join("alias");
    symlink(&repo, &alias).unwrap();
    let r = request(&repo, &alias, WorktreeCreateMode::Detached, "");
    assert!(app
        .preview_create_worktree(&id, &r)
        .unwrap()
        .blocked_reason
        .is_some());
    let parent = dir.path().join("parent");
    fs::create_dir(&parent).unwrap();
    let r = request(&repo, &parent.join("new"), WorktreeCreateMode::Detached, "");
    let p = app.preview_create_worktree(&id, &r).unwrap();
    fs::rename(&parent, dir.path().join("old-parent")).unwrap();
    fs::create_dir(&parent).unwrap();
    assert_eq!(
        app.create_worktree(&id, &r, &p.token).unwrap().status,
        "failed"
    );
    assert!(!parent.join("new").exists());
}
#[cfg(unix)]
#[test]
fn preview_never_runs_hooks_and_confirmed_hook_failure_retains_partial_result() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, app, repo, id) = setup();
    let hooks = dir.path().join("hooks");
    fs::create_dir(&hooks).unwrap();
    let hook = hooks.join("post-checkout");
    fs::write(&hook, "#!/bin/sh\nprintf hook > hook-ran\nexit 9\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        &repo,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    let target = dir.path().join("partial");
    let r = request(&repo, &target, WorktreeCreateMode::NewBranch, "partial");
    let p = app.preview_create_worktree(&id, &r).unwrap();
    assert!(!target.exists());
    assert!(!repo.join("hook-ran").exists());
    let receipt = app.create_worktree(&id, &r, &p.token).unwrap();
    assert_eq!(receipt.status, "unknown");
    assert!(receipt.recovery.unwrap().contains("No directory deletion"));
    assert!(target.join("hook-ran").exists());
    assert_eq!(git(&target, &["branch", "--show-current"]), "partial");
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
}

#[test]
fn submodule_pointers_are_disclosed_without_initialization_or_source_changes() {
    let (dir, app, repo, id) = setup();
    let oid = git(&repo, &["rev-parse", "HEAD"]);
    git(
        &repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            &oid,
            "child",
        ],
    );
    git(&repo, &["commit", "-m", "Recorded child pointer"]);
    let target = dir.path().join("with-pointer");
    let r = request(&repo, &target, WorktreeCreateMode::Detached, "");
    let p = app.preview_create_worktree(&id, &r).unwrap();
    assert_eq!(p.submodules, 1);
    assert_eq!(p.files, 2);
    apply(&app, &id, &r);
    assert!(!target.join("child/.git").exists());
    assert_eq!(
        git(&target, &["ls-files", "--stage", "child"]),
        format!("160000 {oid} 0\tchild")
    );
}
#[test]
fn sha256_worktree_uses_exact_full_commit_identity() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("sha256");
    fs::create_dir(&repo).unwrap();
    let probe = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["init", "--object-format=sha256", "-b", "main"])
        .output()
        .unwrap();
    if !probe.status.success() {
        return;
    }
    git(&repo, &["config", "core.hooksPath", "/dev/null"]);
    fs::write(repo.join("file"), "SHA256 worktree\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "SHA256 root"]);
    let app = Application::new(dir.path().join("app/state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    let target = dir.path().join("sha256-linked");
    let r = request(
        &repo,
        &target,
        WorktreeCreateMode::NewBranch,
        "sha256-topic",
    );
    assert_eq!(r.oid.len(), 64);
    apply(&app, &id, &r);
    assert_eq!(git(&target, &["rev-parse", "HEAD"]), r.oid);
}

#[test]
fn destination_requires_explicit_absolute_path_without_dot_components() {
    let (dir, app, repo, id) = setup();
    for destination in [
        "relative-worktree".to_owned(),
        format!("{}/./new", dir.path().display()),
        format!("{}/source/../new", dir.path().display()),
    ] {
        let r = WorktreeCreateRequest {
            destination,
            ..request(
                &repo,
                &dir.path().join("unused"),
                WorktreeCreateMode::Detached,
                "",
            )
        };
        assert_eq!(
            app.preview_create_worktree(&id, &r).unwrap_err().code,
            "worktree_request"
        );
    }
}

#[test]
fn broken_parent_repository_metadata_is_not_treated_as_an_available_location() {
    let (dir, app, repo, id) = setup();
    let parent = dir.path().join("broken-parent");
    fs::create_dir(&parent).unwrap();
    fs::write(parent.join(".git"), "broken Git metadata\n").unwrap();
    let r = request(&repo, &parent.join("new"), WorktreeCreateMode::Detached, "");
    assert!(app.preview_create_worktree(&id, &r).is_err());
    assert!(!parent.join("new").exists());
}
