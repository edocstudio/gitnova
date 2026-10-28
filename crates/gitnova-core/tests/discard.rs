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
fn review(app: &Application, id: &str, paths: &[&str], scope: DiscardScope) -> DiscardPreview {
    app.preview_discard(
        id,
        &DiscardRequest {
            paths: paths
                .iter()
                .map(|p| NativePath::new(Path::new(p)))
                .collect(),
            scope,
            id: String::new(),
        },
    )
    .unwrap()
}
fn apply(app: &Application, id: &str, p: &DiscardPreview) -> OperationReceipt {
    app.discard_changes(id, &p.request, &p.token).unwrap()
}
#[test]
fn unstaged_restores_index_and_preserves_staging_drafts_and_head() {
    let (_dir, app, repo, id) = setup();
    fs::write(repo.join("file.txt"), "staged\n").unwrap();
    git(&repo, &["add", "."]);
    fs::write(repo.join("file.txt"), "working\n").unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let p = review(&app, &id, &["file.txt"], DiscardScope::Unstaged);
    assert!(p.blocked_reason.is_none());
    assert!(!p.recovery_dir.to_path().unwrap().exists());
    let receipt = apply(&app, &id, &p);
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "staged\n"
    );
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    let recovery = p.recovery_dir.to_path().unwrap();
    assert_eq!(
        fs::read_to_string(recovery.join("work-0")).unwrap(),
        "working\n"
    );
    assert_eq!(
        fs::read_to_string(recovery.join("staged-0")).unwrap(),
        "staged\n"
    );
    assert_eq!(fs::read(recovery.join("index")).unwrap(), index);
}
#[test]
fn all_restores_selected_staging_and_working_files_and_saves_untracked_binary() {
    let (_dir, app, repo, id) = setup();
    fs::write(repo.join("keep.txt"), "base keep\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Two files"]);
    fs::write(repo.join("keep.txt"), "staged keep\n").unwrap();
    fs::write(repo.join("file.txt"), "staged\n").unwrap();
    git(&repo, &["add", "."]);
    fs::write(repo.join("file.txt"), "working\n").unwrap();
    fs::write(repo.join("new.bin"), [0, 255, 0, 13, 10]).unwrap();
    let p = review(&app, &id, &["file.txt", "new.bin"], DiscardScope::All);
    let r = apply(&app, &id, &p);
    assert_eq!(r.status, "succeeded", "{}", r.summary);
    assert_eq!(git(&repo, &["show", ":file.txt"]), "base");
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "base\n");
    assert!(!repo.join("new.bin").exists());
    assert_eq!(git(&repo, &["show", ":keep.txt"]), "staged keep");
    assert_eq!(
        fs::read(p.recovery_dir.to_path().unwrap().join("work-1")).unwrap(),
        [0, 255, 0, 13, 10]
    );
}
#[test]
fn added_deleted_unicode_literal_paths_and_unborn_index_are_supported() {
    for unborn in [false, true] {
        let (_dir, app, repo, id) = setup();
        if unborn {
            git(&repo, &["checkout", "--orphan", "unborn"]);
        }
        fs::write(repo.join(":(glob) งาน.txt"), "valuable\n").unwrap();
        git(&repo, &["add", "--", ":(literal):(glob) งาน.txt"]);
        fs::remove_file(repo.join("file.txt")).unwrap();
        let p = review(
            &app,
            &id,
            &[":(glob) งาน.txt", "file.txt"],
            DiscardScope::All,
        );
        let r = apply(&app, &id, &p);
        assert_eq!(r.status, "succeeded", "{}", r.summary);
        assert!(!repo.join(":(glob) งาน.txt").exists());
        assert_eq!(repo.join("file.txt").exists(), !unborn);
        assert!(git(&repo, &["status", "--porcelain"]).is_empty());
    }
}
#[test]
fn stale_same_size_content_index_configuration_and_pending_operations_never_discard() {
    for variant in ["file", "index", "config", "operation", "lock"] {
        let (_dir, app, repo, id) = setup();
        fs::write(repo.join("file.txt"), "edit\n").unwrap();
        let p = review(&app, &id, &["file.txt"], DiscardScope::All);
        match variant {
            "file" => fs::write(repo.join("file.txt"), "late\n").unwrap(),
            "index" => {
                git(&repo, &["add", "."]);
            }
            "config" => {
                git(&repo, &["config", "core.eol", "crlf"]);
            }
            "operation" => fs::write(
                repo.join(".git/MERGE_HEAD"),
                git(&repo, &["rev-parse", "HEAD"]),
            )
            .unwrap(),
            "lock" => fs::write(repo.join(".git/index.lock"), "owner").unwrap(),
            _ => unreachable!(),
        }
        let r = app.discard_changes(&id, &p.request, &p.token);
        assert!(r.is_err() || r.unwrap().status != "succeeded", "{variant}");
        assert_eq!(
            fs::read_to_string(repo.join("file.txt")).unwrap(),
            if variant == "file" {
                "late\n"
            } else {
                "edit\n"
            }
        );
    }
}
#[test]
fn submodule_nested_directories_hidden_flags_and_ignored_paths_cannot_be_discarded() {
    for variant in ["submodule", "nested", "skip", "ignored"] {
        let (_dir, app, repo, id) = setup();
        let path = match variant {
            "submodule" => {
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
                "child"
            }
            "nested" => {
                fs::create_dir(repo.join("child")).unwrap();
                git(&repo.join("child"), &["init", "-b", "main"]);
                fs::write(repo.join("child/file"), "nested work").unwrap();
                "child/"
            }
            "skip" => {
                fs::write(repo.join("file.txt"), "staged\n").unwrap();
                git(&repo, &["add", "."]);
                git(&repo, &["update-index", "--skip-worktree", "file.txt"]);
                "file.txt"
            }
            "ignored" => {
                fs::write(repo.join(".git/info/exclude"), "ignored.txt\n").unwrap();
                fs::write(repo.join("ignored.txt"), "ignored work").unwrap();
                "ignored.txt"
            }
            _ => unreachable!(),
        };
        let r = app.preview_discard(
            &id,
            &DiscardRequest {
                paths: vec![NativePath::new(Path::new(path))],
                scope: DiscardScope::All,
                id: String::new(),
            },
        );
        if let Ok(p) = r {
            assert!(p.blocked_reason.is_some(), "{variant}");
            assert_ne!(apply(&app, &id, &p).status, "succeeded");
        }
        assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
    }
}
#[cfg(unix)]
#[test]
fn symlink_leaf_is_saved_without_touching_target_and_parent_symlinks_are_blocked() {
    use std::os::unix::fs::symlink;
    let (dir, app, repo, id) = setup();
    let outside = dir.path().join("outside");
    fs::write(&outside, "outside").unwrap();
    fs::remove_file(repo.join("file.txt")).unwrap();
    symlink(&outside, repo.join("file.txt")).unwrap();
    let p = review(&app, &id, &["file.txt"], DiscardScope::Unstaged);
    assert_eq!(apply(&app, &id, &p).status, "succeeded");
    assert_eq!(fs::read_to_string(outside).unwrap(), "outside");
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "base\n");
    fs::create_dir(repo.join("dir")).unwrap();
    fs::write(repo.join("dir/file"), "base\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Directory"]);
    fs::remove_dir_all(repo.join("dir")).unwrap();
    symlink(dir.path(), repo.join("dir")).unwrap();
    assert!(app
        .preview_discard(
            &id,
            &DiscardRequest {
                paths: vec![NativePath::new(Path::new("dir/file"))],
                scope: DiscardScope::All,
                id: String::new()
            }
        )
        .is_err());
}
#[cfg(unix)]
#[test]
fn preview_never_runs_filters_and_partial_native_failure_retains_complete_recovery() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, app, repo, id) = setup();
    fs::write(repo.join(".gitattributes"), "file.txt filter=test\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Filter attribute"]);
    let filter = dir.path().join("filter.sh");
    fs::write(&filter, "#!/bin/sh\nprintf hook > filter-ran\nexit 9\n").unwrap();
    fs::set_permissions(&filter, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        &repo,
        &["config", "filter.test.smudge", filter.to_str().unwrap()],
    );
    git(&repo, &["config", "filter.test.required", "true"]);
    fs::write(repo.join("file.txt"), "edited\n").unwrap();
    let p = review(&app, &id, &["file.txt"], DiscardScope::Unstaged);
    assert!(!repo.join("filter-ran").exists());
    let r = apply(&app, &id, &p);
    assert_eq!(r.status, "unknown", "{}", r.summary);
    let recovery = p.recovery_dir.to_path().unwrap();
    assert!(recovery.join("manifest.json").exists());
    assert_eq!(
        fs::read_to_string(recovery.join("work-0")).unwrap(),
        "edited\n"
    );
    assert!(repo.join("filter-ran").exists());
}

#[test]
fn submodule_pointer_has_a_real_diff_and_uses_recorded_restore_without_discarding_child_work() {
    let (dir, app, repo, id) = setup();
    let child = dir.path().join("child-source");
    fs::create_dir(&child).unwrap();
    git(&child, &["init", "-b", "main"]);
    fs::write(child.join("file.txt"), "child base\n").unwrap();
    git(&child, &["add", "."]);
    git(&child, &["commit", "-m", "Child base"]);
    git(
        &repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child.to_str().unwrap(),
            "_dags",
        ],
    );
    git(&repo, &["commit", "-am", "Record child"]);
    let checkout = repo.join("_dags");
    let recorded = git(&checkout, &["rev-parse", "HEAD"]);
    fs::write(checkout.join("file.txt"), "child next\n").unwrap();
    git(&checkout, &["commit", "-am", "Child next"]);
    let next = git(&checkout, &["rev-parse", "HEAD"]);
    let path = NativePath::new(Path::new("_dags"));
    let p = app.file_preview(&id, &path, false).unwrap();
    assert!(p.diff.contains(&recorded));
    assert!(p.diff.contains(&next));
    let discard = review(&app, &id, &["_dags"], DiscardScope::All);
    assert!(discard.blocked_reason.is_some());
    assert_eq!(apply(&app, &id, &discard).status, "failed");
    fs::write(checkout.join("file.txt"), "child unsaved\n").unwrap();
    let restore = app.preview_submodule_update(&id, &path, "restore").unwrap();
    assert!(restore.blocked_reason.is_some());
    assert_eq!(
        fs::read_to_string(checkout.join("file.txt")).unwrap(),
        "child unsaved\n"
    );
}
#[test]
fn failed_recovery_storage_never_changes_files_or_index() {
    let (_dir, app, repo, id) = setup();
    fs::write(repo.join("file.txt"), "edited\n").unwrap();
    let p = review(&app, &id, &["file.txt"], DiscardScope::All);
    let original = fs::read(repo.join(".git/index")).unwrap();
    fs::write(
        repo.join(".git/gitnova-discard-recovery"),
        "not a directory",
    )
    .unwrap();
    assert!(app.discard_changes(&id, &p.request, &p.token).is_err());
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "edited\n"
    );
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), original);
}

#[test]
fn attribute_changes_after_review_are_stale_without_running_helpers() {
    let (_dir, app, repo, id) = setup();
    fs::write(repo.join("file.txt"), "edited\n").unwrap();
    let p = review(&app, &id, &["file.txt"], DiscardScope::Unstaged);
    fs::write(repo.join(".gitattributes"), "file.txt text eol=crlf\n").unwrap();
    assert_eq!(
        app.discard_changes(&id, &p.request, &p.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "edited\n"
    );
}
#[test]
fn built_in_crlf_restore_uses_git_checkout_semantics() {
    let (_dir, app, repo, id) = setup();
    fs::write(repo.join(".gitattributes"), "file.txt text eol=crlf\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "EOL rule"]);
    fs::write(repo.join("file.txt"), "edited\n").unwrap();
    let p = review(&app, &id, &["file.txt"], DiscardScope::Unstaged);
    assert_eq!(apply(&app, &id, &p).status, "succeeded");
    assert_eq!(fs::read(repo.join("file.txt")).unwrap(), b"base\r\n");
}

#[test]
fn initial_untracked_files_can_be_discarded_without_creating_an_index_or_commit() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("empty");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "core.hooksPath", "/dev/null"]);
    fs::write(repo.join("new.txt"), "unsaved\n").unwrap();
    let app = Application::new(dir.path().join("state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    let p = review(&app, &id, &["new.txt"], DiscardScope::All);
    assert!(p.head.is_none());
    assert!(!repo.join(".git/index").exists());
    assert_eq!(apply(&app, &id, &p).status, "succeeded");
    assert!(!repo.join("new.txt").exists());
    assert!(!repo.join(".git/index").exists());
    assert_eq!(
        fs::read_to_string(p.recovery_dir.to_path().unwrap().join("work-0")).unwrap(),
        "unsaved\n"
    );
}

#[test]
fn oversized_working_content_is_refused_before_recovery_or_discard() {
    let (_dir, app, repo, id) = setup();
    let original = fs::read(repo.join(".git/index")).unwrap();
    let bytes = vec![b'x'; 17 * 1024 * 1024];
    fs::write(repo.join("large.bin"), &bytes).unwrap();
    drop(bytes);
    let r = app.preview_discard(
        &id,
        &DiscardRequest {
            paths: vec![NativePath::new(Path::new("large.bin"))],
            scope: DiscardScope::All,
            id: String::new(),
        },
    );
    assert!(r.is_err());
    assert_eq!(
        fs::metadata(repo.join("large.bin")).unwrap().len(),
        17 * 1024 * 1024
    );
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), original);
    assert!(!repo.join(".git/gitnova-discard-recovery").exists());
}

#[test]
fn recovery_survives_linked_worktree_removal_and_identifies_its_original_checkout() {
    let (dir, app, repo, _id) = setup();
    let linked = dir.path().join("linked");
    git(
        &repo,
        &["worktree", "add", "-b", "topic", linked.to_str().unwrap()],
    );
    let c = app.add_repository(&linked).unwrap();
    let id = c.workspaces[0].selected_id.clone().unwrap();
    fs::write(linked.join("file.txt"), "linked saved work\n").unwrap();
    let p = review(&app, &id, &["file.txt"], DiscardScope::Unstaged);
    assert_eq!(p.source, NativePath::new(&linked.canonicalize().unwrap()));
    assert!(p
        .recovery_dir
        .to_path()
        .unwrap()
        .starts_with(repo.canonicalize().unwrap().join(".git")));
    assert_eq!(apply(&app, &id, &p).status, "succeeded");
    git(&repo, &["worktree", "remove", linked.to_str().unwrap()]);
    assert_eq!(
        fs::read_to_string(p.recovery_dir.to_path().unwrap().join("work-0")).unwrap(),
        "linked saved work\n"
    );
}
