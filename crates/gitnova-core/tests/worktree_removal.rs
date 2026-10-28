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
fn target(dir: &tempfile::TempDir, repo: &Path) -> PathBuf {
    let target = dir.path().join("linked งาน");
    git(
        repo,
        &["worktree", "add", "-b", "topic", target.to_str().unwrap()],
    );
    target.canonicalize().unwrap()
}
fn review(app: &Application, id: &str, target: &Path) -> WorktreeRemovePreview {
    app.preview_remove_worktree(
        id,
        &WorktreeRemoveRequest {
            path: NativePath::new(target),
            id: String::new(),
        },
    )
    .unwrap()
}
fn apply(app: &Application, id: &str, p: &WorktreeRemovePreview) -> OperationReceipt {
    app.remove_worktree(id, &p.request, &p.token).unwrap()
}
#[test]
fn clean_removal_cleans_all_memberships_preserves_source_and_retires_identity() {
    let (dir, app, repo, id) = setup();
    let target = target(&dir, &repo);
    let first_workspace = app.catalog().unwrap().active_workspace_id;
    let catalog = app.add_repository(&target).unwrap();
    let retired = catalog.workspaces[0].memberships[0]
        .worktree_id
        .clone()
        .unwrap();
    app.select(&id).unwrap();
    app.create_workspace("Another workspace").unwrap();
    app.add_repository(&target).unwrap();
    app.switch_workspace(&first_workspace).unwrap();
    fs::write(repo.join("file.txt"), "staged\n").unwrap();
    git(&repo, &["add", "."]);
    fs::write(repo.join("file.txt"), "working\n").unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    let head = git(&target, &["rev-parse", "HEAD"]);
    let p = review(&app, &id, &target);
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert_eq!(p.memberships.len(), 2);
    assert_eq!(p.worktree_id.as_deref(), Some(retired.as_str()));
    assert!(target.exists());
    let receipt = apply(&app, &id, &p);
    assert_eq!(
        receipt.status, "succeeded",
        "{} {:?}",
        receipt.summary, receipt.recovery
    );
    assert!(!target.exists());
    assert_eq!(git(&repo, &["rev-parse", "topic"]), head);
    assert_eq!(git(&repo, &["rev-parse", &p.recovery_ref]), head);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "working\n"
    );
    let catalog = app.catalog().unwrap();
    assert_eq!(
        catalog.workspaces[0].selected_id.as_deref(),
        Some(id.as_str())
    );
    assert!(catalog.workspaces[1].selected_id.is_none());
    assert!(!catalog.worktrees.iter().any(|w| w.id == retired));
    assert!(catalog.workspaces.iter().all(|w| w
        .memberships
        .iter()
        .all(|m| m.worktree_id.as_deref() != Some(retired.as_str()))));
    git(
        &repo,
        &["worktree", "add", target.to_str().unwrap(), "topic"],
    );
    let catalog = app.add_repository(&target).unwrap();
    assert_ne!(
        catalog.workspaces[0].memberships[0].worktree_id.as_deref(),
        Some(retired.as_str())
    );
}
#[test]
fn detached_unpublished_commit_is_retained() {
    let (dir, app, repo, id) = setup();
    let target = target(&dir, &repo);
    git(&target, &["switch", "--detach"]);
    fs::write(target.join("file.txt"), "unpublished\n").unwrap();
    git(&target, &["add", "."]);
    git(&target, &["commit", "-m", "Detached work"]);
    let head = git(&target, &["rev-parse", "HEAD"]);
    let main = git(&repo, &["rev-parse", "main"]);
    let p = review(&app, &id, &target);
    assert!(p.branch.is_none());
    assert_eq!(apply(&app, &id, &p).status, "succeeded");
    assert_eq!(git(&repo, &["rev-parse", &p.recovery_ref]), head);
    assert_eq!(git(&repo, &["rev-parse", "main"]), main);
    assert_eq!(
        git(&repo, &["show", &format!("{}:file.txt", p.recovery_ref)]),
        "unpublished"
    );
}
#[test]
fn modified_staged_untracked_ignored_and_nested_data_are_never_deleted() {
    for variant in ["modified", "staged", "untracked", "ignored", "nested"] {
        let (dir, app, repo, id) = setup();
        let target = target(&dir, &repo);
        match variant {
            "modified" => {
                fs::write(target.join("file.txt"), "edited\n").unwrap();
            }
            "staged" => {
                fs::write(target.join("file.txt"), "edited\n").unwrap();
                git(&target, &["add", "."]);
            }
            "untracked" => {
                fs::write(target.join("new.txt"), "new\n").unwrap();
            }
            "ignored" => {
                fs::write(repo.join(".git/info/exclude"), "local.tmp\n").unwrap();
                fs::write(target.join("local.tmp"), "ignored but valuable\n").unwrap();
            }
            "nested" => {
                let nested = target.join("nested");
                fs::create_dir(&nested).unwrap();
                git(&nested, &["init", "-b", "main"]);
                fs::write(nested.join("data"), "nested data").unwrap();
            }
            _ => unreachable!(),
        }
        let index_path =
            PathBuf::from(git(&target, &["rev-parse", "--absolute-git-dir"])).join("index");
        let index = fs::read(&index_path).unwrap();
        let p = review(&app, &id, &target);
        assert!(p.blocked_reason.is_some(), "{variant}");
        assert_eq!(apply(&app, &id, &p).status, "failed");
        assert!(target.exists());
        assert_eq!(fs::read(index_path).unwrap(), index);
    }
}
#[test]
fn hidden_flags_submodules_native_locks_and_locked_trees_block_removal() {
    for variant in [
        "assume",
        "skip",
        "submodule",
        "index",
        "head",
        "branch",
        "locked",
    ] {
        let (dir, app, repo, id) = setup();
        let target = target(&dir, &repo);
        let admin = PathBuf::from(git(&target, &["rev-parse", "--absolute-git-dir"]));
        let lock = match variant {
            "assume" => {
                git(&target, &["update-index", "--assume-unchanged", "file.txt"]);
                None
            }
            "skip" => {
                git(&target, &["update-index", "--skip-worktree", "file.txt"]);
                None
            }
            "submodule" => {
                let head = git(&target, &["rev-parse", "HEAD"]);
                git(
                    &target,
                    &[
                        "update-index",
                        "--add",
                        "--cacheinfo",
                        "160000",
                        &head,
                        "child",
                    ],
                );
                git(&target, &["commit", "-m", "Pointer"]);
                None
            }
            "locked" => {
                git(&repo, &["worktree", "lock", target.to_str().unwrap()]);
                None
            }
            "branch" => Some(repo.join(".git/refs/heads/topic.lock")),
            "index" => Some(admin.join("index.lock")),
            "head" => Some(admin.join("HEAD.lock")),
            _ => unreachable!(),
        };
        if let Some(lock) = &lock {
            fs::write(lock, "other owner").unwrap();
        }
        let p = review(&app, &id, &target);
        assert!(p.blocked_reason.is_some(), "{variant}");
        assert_eq!(apply(&app, &id, &p).status, "failed");
        assert!(target.exists());
        if let Some(lock) = lock {
            assert_eq!(fs::read_to_string(lock).unwrap(), "other owner");
        }
    }
}
#[test]
fn active_main_missing_and_replaced_worktree_are_protected() {
    let (dir, app, repo, id) = setup();
    let target = target(&dir, &repo);
    let p = review(&app, &id, &repo.canonicalize().unwrap());
    assert!(p.blocked_reason.is_some());
    let catalog = app.add_repository(&target).unwrap();
    let linked_id = catalog.workspaces[0].selected_id.clone().unwrap();
    assert!(review(&app, &linked_id, &target).blocked_reason.is_some());
    assert!(review(&app, &linked_id, &repo.canonicalize().unwrap())
        .blocked_reason
        .is_some());
    app.select(&id).unwrap();
    fs::rename(&target, dir.path().join("keep-old")).unwrap();
    let p = review(&app, &id, &target);
    assert!(p.blocked_reason.is_some());
    assert_eq!(apply(&app, &id, &p).status, "failed");
    fs::create_dir(&target).unwrap();
    git(&target, &["init", "-b", "independent"]);
    assert!(
        app.preview_remove_worktree(&id, &p.request).is_err()
            || review(&app, &id, &target).blocked_reason.is_some()
    );
    assert!(target.join(".git").exists());
}
#[test]
fn changed_files_refs_and_memberships_invalidate_review() {
    for variant in ["file", "ref", "membership"] {
        let (dir, app, repo, id) = setup();
        let target = target(&dir, &repo);
        let p = review(&app, &id, &target);
        match variant {
            "file" => {
                fs::write(target.join("file.txt"), "changed after preview").unwrap();
            }
            "ref" => {
                git(&repo, &["branch", "new-ref"]);
            }
            "membership" => {
                app.add_repository(&target).unwrap();
                app.select(&id).unwrap();
            }
            _ => unreachable!(),
        }
        let result = app.remove_worktree(&id, &p.request, &p.token);
        assert!(
            result.is_err() || result.unwrap().status != "succeeded",
            "{variant}"
        );
        assert!(target.exists());
        assert!(git(&repo, &["for-each-ref", "refs/gitnova/recovery/"]).is_empty());
    }
}
#[test]
fn catalog_publication_failure_reports_partial_result_and_retains_recovery() {
    let (dir, app, repo, id) = setup();
    let target = target(&dir, &repo);
    app.add_repository(&target).unwrap();
    app.select(&id).unwrap();
    let p = review(&app, &id, &target);
    let state = dir.path().join("app/state.json");
    fs::rename(&state, dir.path().join("old-state.json")).unwrap();
    fs::create_dir(&state).unwrap();
    let receipt = apply(&app, &id, &p);
    assert_eq!(receipt.status, "unknown", "{}", receipt.summary);
    assert!(receipt.summary.contains("workspace cleanup"));
    assert!(!target.exists());
    assert!(receipt.recovery.unwrap().contains("no rollback"));
    assert_eq!(git(&repo, &["rev-parse", &p.recovery_ref]), p.head.unwrap());
    assert_eq!(app.catalog().unwrap().workspaces[0].memberships.len(), 2);
}
#[test]
fn stat_preserved_content_is_rechecked_without_mutating_real_index() {
    use std::fs::FileTimes;
    let (dir, app, repo, id) = setup();
    let target = target(&dir, &repo);
    git(&target, &["config", "core.trustctime", "false"]);
    git(&target, &["config", "core.checkStat", "minimal"]);
    let file = target.join("file.txt");
    fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_times(
            FileTimes::new().set_modified(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000),
            ),
        )
        .unwrap();
    git(&target, &["update-index", "--refresh"]);
    let old = fs::metadata(&file).unwrap();
    fs::write(&file, "evil\n").unwrap();
    fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_times(
            FileTimes::new()
                .set_accessed(old.accessed().unwrap())
                .set_modified(old.modified().unwrap()),
        )
        .unwrap();
    let index_path =
        PathBuf::from(git(&target, &["rev-parse", "--absolute-git-dir"])).join("index");
    let index = fs::read(&index_path).unwrap();
    assert_eq!(
        git(&target, &["status", "--porcelain", "-uall"]),
        "",
        "The fixture must fool the native stat cache"
    );
    let p = review(&app, &id, &target);
    assert!(p.blocked_reason.is_some());
    assert_eq!(apply(&app, &id, &p).status, "failed");
    assert_eq!(fs::read(index_path).unwrap(), index);
    assert_eq!(fs::read_to_string(file).unwrap(), "evil\n");
}
#[cfg(unix)]
#[test]
fn reference_hook_edits_are_detected_after_recovery_is_pinned() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, app, repo, id) = setup();
    let target = target(&dir, &repo);
    let hooks = dir.path().join("hooks");
    fs::create_dir(&hooks).unwrap();
    let hook = hooks.join("reference-transaction");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nif [ \"$1\" = committed ]; then printf edited > '{}' ; fi\n",
            target.join("file.txt").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        &repo,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    let p = review(&app, &id, &target);
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert_eq!(
        fs::read_to_string(target.join("file.txt")).unwrap(),
        "base\n"
    );
    let receipt = apply(&app, &id, &p);
    assert_eq!(receipt.status, "unknown", "{}", receipt.summary);
    assert!(target.exists());
    assert_eq!(git(&repo, &["rev-parse", &p.recovery_ref]), p.head.unwrap());
    assert_eq!(
        fs::read_to_string(target.join("file.txt")).unwrap(),
        "edited"
    );
}

#[cfg(unix)]
#[test]
fn symlink_replacement_preserves_both_folders() {
    use std::os::unix::fs::symlink;
    let (dir, app, repo, id) = setup();
    let target = target(&dir, &repo);
    let p = review(&app, &id, &target);
    let original = dir.path().join("original");
    fs::rename(&target, &original).unwrap();
    symlink(&original, &target).unwrap();
    assert!(app.remove_worktree(&id, &p.request, &p.token).is_err());
    assert!(target.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(original.join("file.txt").exists());
}
#[test]
fn native_target_operation_is_never_removed() {
    let (dir, app, repo, id) = setup();
    let target = target(&dir, &repo);
    let p = review(&app, &id, &target);
    let admin = PathBuf::from(git(&target, &["rev-parse", "--absolute-git-dir"]));
    fs::write(admin.join("MERGE_HEAD"), format!("{}\n", p.head.unwrap())).unwrap();
    assert!(app.remove_worktree(&id, &p.request, &p.token).is_err());
    assert!(target.exists());
    assert!(admin.join("MERGE_HEAD").exists());
}
#[test]
fn sha256_detached_removal_retains_full_identity() {
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
    fs::write(repo.join("file.txt"), "sha256\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "SHA256"]);
    let app = Application::new(dir.path().join("app/state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    let target = dir.path().join("detached");
    git(
        &repo,
        &["worktree", "add", "--detach", target.to_str().unwrap()],
    );
    let p = review(&app, &id, &target.canonicalize().unwrap());
    assert_eq!(p.head.as_ref().unwrap().len(), 64);
    assert_eq!(apply(&app, &id, &p).status, "succeeded");
    assert_eq!(git(&repo, &["rev-parse", &p.recovery_ref]), p.head.unwrap());
}

#[test]
fn same_gitdir_and_content_in_a_replaced_folder_still_invalidates_removal() {
    let (dir, app, repo, id) = setup();
    let target = target(&dir, &repo);
    let p = review(&app, &id, &target);
    let original = dir.path().join("kept-original");
    fs::rename(&target, &original).unwrap();
    fs::create_dir(&target).unwrap();
    fs::copy(original.join(".git"), target.join(".git")).unwrap();
    fs::copy(original.join("file.txt"), target.join("file.txt")).unwrap();
    assert_eq!(
        app.remove_worktree(&id, &p.request, &p.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    assert!(original.join("file.txt").exists());
    assert!(target.join("file.txt").exists());
}
