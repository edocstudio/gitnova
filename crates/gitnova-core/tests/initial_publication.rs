use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(root: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .args(args)
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim_end().into()
}
struct F {
    _dir: tempfile::TempDir,
    app: Application,
    root: PathBuf,
    id: String,
}
fn fixture() -> F {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    for (k, v) in [
        ("user.name", "GitNova Fixture"),
        ("user.email", "fixture@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", ".git/hooks"),
        ("core.attributesFile", ".git/no-attributes"),
    ] {
        git(&root, &["config", k, v]);
    }
    let app = Application::new(dir.path().join("state/catalog.json")).unwrap();
    let id = app.add_repository(&root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    F {
        _dir: dir,
        app,
        root,
        id,
    }
}
fn stage(f: &F) -> OperationReceipt {
    let p = f.app.preview_stage_all(&f.id).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    f.app.stage_all(&f.id, &p.token).unwrap()
}
#[test]
fn empty_project_with_383_files_reviews_large_commit_then_adds_remote_and_publishes() {
    let f = fixture();
    fs::create_dir(f.root.join("src")).unwrap();
    fs::write(f.root.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(f.root.join("ignored.txt"), "keep out\n").unwrap();
    for i in 0..382 {
        fs::write(
            f.root.join(format!("src/module-{i:03}.txt")),
            format!("module {i}\n{}\n", "fixture line\n".repeat(600)),
        )
        .unwrap();
    }
    let p = f.app.preview_stage_all(&f.id).unwrap();
    assert_eq!(p.files.len(), 383);
    assert!(p.excluded.is_empty());
    assert!(!f.root.join(".git/index").exists());
    assert_eq!(
        f.app.stage_all(&f.id, &p.token).unwrap().status,
        "succeeded"
    );
    let p = f.app.commit_preview(&f.id).unwrap();
    assert!(p.complete_manifest);
    assert!(p.truncated);
    assert_eq!(p.files.len(), 383);
    let file = NativePath::new(Path::new("src/module-381.txt"));
    let patch = f.app.commit_file_preview(&f.id, &file, &p.token).unwrap();
    assert_eq!(patch.path, file);
    assert!(patch.diff.contains("+module 381"));
    assert!(!patch.diff.contains("+module 0"));
    let r = f
        .app
        .create_commit(&f.id, "Initial commit", &p.token)
        .unwrap();
    assert_eq!(r.status, "succeeded", "{}", r.summary);
    let head = git(&f.root, &["rev-parse", "HEAD"]);
    assert!(git(&f.root, &["show", "-s", "--format=%P", "HEAD"]).is_empty());
    assert_eq!(
        git(&f.root, &["ls-tree", "-r", "--name-only", "HEAD"])
            .lines()
            .count(),
        383
    );
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%ae", "HEAD"]),
        "fixture@example.invalid"
    );
    let bare = f._dir.path().join("remote.git");
    fs::create_dir(&bare).unwrap();
    git(&bare, &["init", "--bare"]);
    let request = RemoteAddRequest {
        name: "origin".into(),
        url: bare.to_str().unwrap().into(),
    };
    let original = fs::read(f.root.join(".git/config")).unwrap();
    let p = f.app.preview_remote_add(&f.id, &request).unwrap();
    assert!(git(&f.root, &["remote"]).is_empty());
    assert_eq!(
        f.app.add_remote(&f.id, &request, &p.token).unwrap().status,
        "succeeded"
    );
    assert_eq!(
        fs::read(p.recovery_path.to_path().unwrap()).unwrap(),
        original
    );
    let push = PushRequest {
        branch: "main".into(),
        oid: head.clone(),
        remote: "origin".into(),
        remote_branch: "main".into(),
        set_upstream: true,
        buffered_https: false,
    };
    let p = f.app.preview_push(&f.id, &push).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert_eq!(p.outgoing_count, 1);
    let r = f.app.push_branch(&f.id, &push, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{}", r.summary);
    assert_eq!(git(&bare, &["rev-parse", "main"]), head);
    assert_eq!(
        git(&f.root, &["rev-parse", "--abbrev-ref", "main@{upstream}"]),
        "origin/main"
    );
    assert!(f.root.join("ignored.txt").exists());
}
#[test]
fn stage_all_rejects_changed_contents_and_preserves_preexisting_native_locks() {
    let f = fixture();
    fs::write(f.root.join("a"), "first\n").unwrap();
    let p = f.app.preview_stage_all(&f.id).unwrap();
    fs::write(f.root.join("a"), "changed\n").unwrap();
    assert!(f.app.stage_all(&f.id, &p.token).is_err());
    assert!(!f.root.join(".git/index").exists());
    let p = f.app.preview_stage_all(&f.id).unwrap();
    fs::write(f.root.join(".git/index.lock"), "other operation").unwrap();
    assert_ne!(
        f.app.stage_all(&f.id, &p.token).unwrap().status,
        "succeeded"
    );
    assert_eq!(
        fs::read_to_string(f.root.join(".git/index.lock")).unwrap(),
        "other operation"
    );
    assert!(!f.root.join(".git/index").exists());
}
#[test]
fn stage_all_preserves_working_bytes_and_handles_partial_deletes_and_literal_names() {
    let f = fixture();
    fs::write(f.root.join("delete me"), "old\n").unwrap();
    fs::write(f.root.join("partial"), "base\n").unwrap();
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-m", "Base"]);
    fs::remove_file(f.root.join("delete me")).unwrap();
    fs::write(f.root.join("partial"), "staged\n").unwrap();
    git(&f.root, &["add", "partial"]);
    fs::write(f.root.join("partial"), "working\n").unwrap();
    fs::write(f.root.join("--literal [file] name"), "literal\n").unwrap();
    assert_eq!(stage(&f).status, "succeeded");
    assert_eq!(git(&f.root, &["show", ":partial"]), "working");
    assert_eq!(git(&f.root, &["show", ":--literal [file] name"]), "literal");
    assert!(!f.root.join("delete me").exists());
    assert_eq!(
        fs::read_to_string(f.root.join("partial")).unwrap(),
        "working\n"
    );
}
#[test]
fn stage_all_excludes_nested_repository_without_staging_its_files_or_pointer() {
    let f = fixture();
    let nested = f.root.join("nested");
    fs::create_dir(&nested).unwrap();
    git(&nested, &["init", "-b", "main"]);
    fs::write(nested.join("private.txt"), "child work\n").unwrap();
    fs::write(f.root.join("ordinary.txt"), "parent work\n").unwrap();
    let p = f.app.preview_stage_all(&f.id).unwrap();
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.excluded.len(), 1);
    assert_eq!(
        f.app.stage_all(&f.id, &p.token).unwrap().status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["ls-files"]), "ordinary.txt");
    assert_eq!(
        fs::read_to_string(nested.join("private.txt")).unwrap(),
        "child work\n"
    );
}
#[test]
fn add_remote_rejects_unsafe_duplicate_and_rewritten_transports_without_mutating_config() {
    let f = fixture();
    let original = fs::read(f.root.join(".git/config")).unwrap();
    for url in [
        "ext::touch sentinel",
        "https://user:secret@example.invalid/repo",
        "https://example.invalid/repo?token=secret",
        "relative/path",
    ] {
        assert!(f
            .app
            .preview_remote_add(
                &f.id,
                &RemoteAddRequest {
                    name: "origin".into(),
                    url: url.into()
                }
            )
            .is_err());
    }
    assert_eq!(fs::read(f.root.join(".git/config")).unwrap(), original);
    git(
        &f.root,
        &[
            "config",
            "url.ext::touch sentinel.insteadOf",
            "https://fixture.invalid/",
        ],
    );
    assert!(f
        .app
        .preview_remote_add(
            &f.id,
            &RemoteAddRequest {
                name: "origin".into(),
                url: "https://fixture.invalid/project".into()
            }
        )
        .is_err());
    assert!(!f.root.join("sentinel").exists());
    git(
        &f.root,
        &[
            "remote",
            "add",
            "existing",
            "https://example.invalid/repo.git",
        ],
    );
    assert!(f
        .app
        .preview_remote_add(
            &f.id,
            &RemoteAddRequest {
                name: "existing".into(),
                url: "https://example.invalid/other.git".into()
            }
        )
        .is_err());
    assert_eq!(git(&f.root, &["remote"]), "existing");
}
#[test]
fn add_remote_binds_config_and_leaves_other_writers_locks_intact() {
    let f = fixture();
    let request = RemoteAddRequest {
        name: "origin".into(),
        url: "https://example.invalid/project.git".into(),
    };
    let p = f.app.preview_remote_add(&f.id, &request).unwrap();
    git(&f.root, &["config", "fixture.changed", "true"]);
    assert!(f.app.add_remote(&f.id, &request, &p.token).is_err());
    let p = f.app.preview_remote_add(&f.id, &request).unwrap();
    fs::write(f.root.join(".git/config.lock"), "other writer").unwrap();
    assert_ne!(
        f.app.add_remote(&f.id, &request, &p.token).unwrap().status,
        "succeeded"
    );
    assert_eq!(
        fs::read_to_string(f.root.join(".git/config.lock")).unwrap(),
        "other writer"
    );
    assert!(git(&f.root, &["remote"]).is_empty());
}
#[test]
fn commit_file_review_and_commit_refuse_a_moved_index() {
    let f = fixture();
    fs::write(f.root.join("a"), "first\n").unwrap();
    stage(&f);
    let p = f.app.commit_preview(&f.id).unwrap();
    fs::write(f.root.join("b"), "later\n").unwrap();
    git(&f.root, &["add", "b"]);
    assert!(f
        .app
        .commit_file_preview(&f.id, &NativePath::new(Path::new("a")), &p.token)
        .is_err());
    assert_ne!(
        f.app
            .create_commit(&f.id, "Stale", &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert!(f.app.working_changes(&f.id).unwrap().status.head.is_none());
}
#[test]
fn remote_settings_are_shared_by_linked_worktrees_without_changing_either_checkout() {
    let f = fixture();
    fs::write(f.root.join("a"), "base\n").unwrap();
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-m", "Base"]);
    let head = git(&f.root, &["rev-parse", "HEAD"]);
    let linked = f._dir.path().join("linked");
    git(
        &f.root,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    let id = f.app.add_repository(&linked).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    let request = RemoteAddRequest {
        name: "origin".into(),
        url: "https://example.invalid/project.git".into(),
    };
    let p = f.app.preview_remote_add(&id, &request).unwrap();
    assert_eq!(
        f.app.add_remote(&id, &request, &p.token).unwrap().status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["remote"]), "origin");
    assert_eq!(git(&linked, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        git(&f.root, &["config", "user.email"]),
        "fixture@example.invalid"
    );
}

#[cfg(unix)]
#[test]
fn stage_all_preview_never_runs_filters_and_filter_failure_keeps_original_index() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    fs::write(f.root.join("base.txt"), "base\n").unwrap();
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-m", "Base"]);
    let original = fs::read(f.root.join(".git/index")).unwrap();
    let script = f.root.join(".git/fail-filter");
    let sentinel = f.root.join("filter-ran");
    fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\nexit 1\n", sentinel.display()),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        &f.root,
        &[
            "config",
            "filter.broken.clean",
            &format!("'{}'", script.display()),
        ],
    );
    git(&f.root, &["config", "filter.broken.required", "true"]);
    fs::write(f.root.join(".gitattributes"), "*.txt filter=broken\n").unwrap();
    fs::write(f.root.join("new.txt"), "new work\n").unwrap();
    let p = f.app.preview_stage_all(&f.id).unwrap();
    assert!(!sentinel.exists());
    let r = f.app.stage_all(&f.id, &p.token).unwrap();
    assert_ne!(r.status, "succeeded");
    assert!(sentinel.exists());
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), original);
    assert_eq!(
        fs::read_to_string(f.root.join("new.txt")).unwrap(),
        "new work\n"
    );
    assert!(!f.root.join(".git/index.lock").exists());
}

#[test]
fn remote_display_is_config_only_complete_and_never_exposes_embedded_credentials() {
    let f = fixture();
    git(
        &f.root,
        &[
            "remote",
            "add",
            "origin",
            "https://git.example/group/project.git",
        ],
    );
    git(
        &f.root,
        &[
            "remote",
            "add",
            "upstream",
            "git@other.example:team/project.git",
        ],
    );
    git(
        &f.root,
        &[
            "remote",
            "add",
            "private",
            "https://user:synthetic-secret@git.example/private.git",
        ],
    );
    let before = fs::read(f.root.join(".git/config")).unwrap();
    let value = f.app.remote_branches(&f.id).unwrap();
    assert_eq!(value.remotes.len(), 3);
    assert_eq!(
        value.urls["origin"],
        vec!["https://git.example/group/project.git"]
    );
    assert_eq!(
        value.urls["upstream"],
        vec!["git@other.example:team/project.git"]
    );
    assert!(!serde_json::to_string(&value)
        .unwrap()
        .contains("synthetic-secret"));
    assert_eq!(fs::read(f.root.join(".git/config")).unwrap(), before);
    assert!(!f.root.join(".git/FETCH_HEAD").exists());
    assert!(!f.root.join(".git/index").exists());
}
