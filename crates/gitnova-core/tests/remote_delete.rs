use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
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
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim_end().to_owned()
}
fn exists(root: &Path, reference: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["show-ref", "--verify", "--quiet", reference])
        .status()
        .unwrap()
        .success()
}
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    remote: PathBuf,
    app: Application,
    id: String,
    oid: String,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let remote = dir.path().join("remote.git");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Delete Test"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    git(&root, &["config", "core.hooksPath", "/dev/null"]);
    fs::write(root.join("file.txt"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Base"]);
    let oid = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["branch", "feature/cleanup"]);
    git(dir.path(), &["init", "--bare", remote.to_str().unwrap()]);
    git(&remote, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    git(
        &root,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&root, &["push", "origin", "main", "feature/cleanup"]);
    git(&root, &["fetch", "origin"]);
    let app = Application::new(dir.path().join("catalog.json")).unwrap();
    let catalog = app.add_repository(&root).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    Fixture {
        _dir: dir,
        root,
        remote,
        app,
        id,
        oid,
    }
}
fn request(f: &Fixture) -> RemoteDeleteRequest {
    RemoteDeleteRequest {
        remote: "origin".into(),
        branch: "feature/cleanup".into(),
        oid: f.oid.clone(),
    }
}
#[test]
fn remote_delete_preserves_local_branches_dirty_files_index_and_recovery() {
    let f = fixture();
    fs::write(f.root.join("file.txt"), "staged\n").unwrap();
    git(&f.root, &["add", "file.txt"]);
    fs::write(f.root.join("file.txt"), "unstaged\n").unwrap();
    fs::write(f.root.join("untracked.txt"), "keep\n").unwrap();
    let index = fs::read(f.root.join(".git/index")).unwrap();
    git(&f.root, &["tag", "keep-tag"]);
    git(&f.root, &["push", "origin", "refs/tags/keep-tag"]);
    let r = request(&f);
    let p = f.app.preview_remote_delete(&f.id, &r).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert!(exists(&f.remote, "refs/heads/feature/cleanup"));
    assert!(!exists(&f.root, &p.recovery_ref));
    let out = f.app.delete_remote_branch(&f.id, &r, &p.token).unwrap();
    assert_eq!(out.status, "succeeded", "{out:?}");
    assert!(!exists(&f.remote, "refs/heads/feature/cleanup"));
    assert!(!exists(&f.root, "refs/remotes/origin/feature/cleanup"));
    assert_eq!(git(&f.root, &["rev-parse", "feature/cleanup"]), f.oid);
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.oid);
    assert!(exists(&f.remote, "refs/heads/main"));
    assert!(exists(&f.remote, "refs/tags/keep-tag"));
    assert_eq!(git(&f.root, &["branch", "--show-current"]), "main");
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(f.root.join("file.txt")).unwrap(),
        "unstaged\n"
    );
    assert_eq!(
        fs::read_to_string(f.root.join("untracked.txt")).unwrap(),
        "keep\n"
    );
    assert!(f
        .app
        .recorded_activity(&f.id)
        .unwrap()
        .entries
        .iter()
        .any(|e| e.recovery_ref.as_deref() == Some(&p.recovery_ref)));
}
#[test]
fn default_branch_and_stale_remote_review_cannot_delete() {
    let f = fixture();
    let mut r = request(&f);
    r.branch = "main".into();
    let p = f.app.preview_remote_delete(&f.id, &r).unwrap();
    assert!(p
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("default branch"));
    let out = f.app.delete_remote_branch(&f.id, &r, &p.token).unwrap();
    assert_ne!(out.status, "succeeded");
    assert!(exists(&f.remote, "refs/heads/main"));
    let r = request(&f);
    let p = f.app.preview_remote_delete(&f.id, &r).unwrap();
    git(
        &f.root,
        &["commit", "--allow-empty", "-m", "Concurrent remote change"],
    );
    let newer = git(&f.root, &["rev-parse", "HEAD"]);
    git(&f.root, &["push", "origin", "HEAD:refs/heads/concurrent"]);
    git(
        &f.remote,
        &["update-ref", "refs/heads/feature/cleanup", &newer],
    );
    let e = f.app.delete_remote_branch(&f.id, &r, &p.token).unwrap_err();
    assert_eq!(e.code, "stale_preview");
    assert_eq!(git(&f.remote, &["rev-parse", "feature/cleanup"]), newer);
}
#[cfg(unix)]
#[test]
fn exact_remote_lease_refuses_a_change_from_the_pre_push_hook() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    git(
        &f.root,
        &["commit", "--allow-empty", "-m", "Concurrent new tip"],
    );
    let newer = git(&f.root, &["rev-parse", "HEAD"]);
    git(&f.root, &["push", "origin", "HEAD:refs/heads/concurrent"]);
    let hooks = f.root.join(".git/test-hooks");
    fs::create_dir(&hooks).unwrap();
    let marker = f.root.join("hook-ran");
    let hook = hooks.join("pre-push");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\ngit -C '{}' update-ref refs/heads/feature/cleanup '{}'\ntouch '{}'\n",
            f.remote.display(),
            newer,
            marker.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        &f.root,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    let r = request(&f);
    let p = f.app.preview_remote_delete(&f.id, &r).unwrap();
    assert!(!marker.exists());
    let out = f.app.delete_remote_branch(&f.id, &r, &p.token).unwrap();
    assert_eq!(out.status, "unknown", "{out:?}");
    assert!(marker.exists());
    assert_eq!(git(&f.remote, &["rev-parse", "feature/cleanup"]), newer);
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.oid);
    assert!(exists(&f.root, "refs/remotes/origin/feature/cleanup"));
}
#[test]
fn differing_push_destination_keeps_fetch_cache_and_rejects_mirror() {
    let f = fixture();
    let other = f._dir.path().join("other.git");
    git(
        f._dir.path(),
        &[
            "clone",
            "--bare",
            f.remote.to_str().unwrap(),
            other.to_str().unwrap(),
        ],
    );
    git(
        &f.root,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            other.to_str().unwrap(),
        ],
    );
    // Keep hook execution bound to the exact reviewed destination even though
    // native automatic tracking is isolated behind a process-local remote.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let hooks = f.root.join(".git/destination-hooks");
        fs::create_dir(&hooks).unwrap();
        let hook = hooks.join("pre-push");
        fs::write(
            &hook,
            "#!/bin/sh\nprintf '%s\\n%s\\n' \"$1\" \"$2\" > .git/push-destination\n",
        )
        .unwrap();
        fs::set_permissions(hook, fs::Permissions::from_mode(0o755)).unwrap();
        git(
            &f.root,
            &["config", "core.hooksPath", hooks.to_str().unwrap()],
        );
    }
    let config = fs::read(f.root.join(".git/config")).unwrap();
    let r = request(&f);
    let p = f.app.preview_remote_delete(&f.id, &r).unwrap();
    assert!(p.tracking_ref.is_none());
    let out = f.app.delete_remote_branch(&f.id, &r, &p.token).unwrap();
    assert_eq!(out.status, "succeeded", "{out:?}");
    assert!(exists(&f.remote, "refs/heads/feature/cleanup"));
    assert!(exists(&f.root, "refs/remotes/origin/feature/cleanup"));
    assert!(!exists(&other, "refs/heads/feature/cleanup"));
    assert_eq!(fs::read(f.root.join(".git/config")).unwrap(), config);
    assert!(!git(&f.root, &["remote"]).contains("gitnova-reviewed-"));
    #[cfg(unix)]
    {
        let hook_args = fs::read_to_string(f.root.join(".git/push-destination")).unwrap();
        let lines: Vec<_> = hook_args.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("gitnova-reviewed-"));
        assert_eq!(lines[1], other.to_str().unwrap());
    }
    git(
        &f.root,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            f.remote.to_str().unwrap(),
        ],
    );
    git(&f.root, &["config", "remote.origin.mirror", "true"]);
    let p = f.app.preview_remote_delete(&f.id, &r).unwrap();
    assert!(p.blocked_reason.as_deref().unwrap().contains("Mirror"));
}

#[test]
fn custom_mapping_same_repository_and_changed_configuration_are_not_silent_deletions() {
    let f = fixture();
    let r = request(&f);
    let p = f.app.preview_remote_delete(&f.id, &r).unwrap();
    git(&f.root, &["config", "push.followTags", "true"]);
    assert_eq!(
        f.app
            .delete_remote_branch(&f.id, &r, &p.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    git(
        &f.root,
        &[
            "config",
            "--replace-all",
            "remote.origin.fetch",
            "+refs/heads/main:refs/remotes/origin/main",
        ],
    );
    let p = f.app.preview_remote_delete(&f.id, &r).unwrap();
    assert!(p
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("standard fetch mapping"));
    assert!(exists(&f.remote, "refs/heads/feature/cleanup"));
    git(
        &f.root,
        &[
            "config",
            "--replace-all",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
    );
    git(
        &f.root,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            f.root.to_str().unwrap(),
        ],
    );
    let p = f.app.preview_remote_delete(&f.id, &r).unwrap();
    assert!(p
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("same repository"));
    assert!(exists(&f.root, "refs/heads/feature/cleanup"));
}

#[cfg(unix)]
#[test]
fn remote_delete_accepts_tracking_ref_already_removed_by_native_git() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let hooks = f.root.join(".git/test-hooks");
    fs::create_dir(&hooks).unwrap();
    let hook = hooks.join("pre-push");
    fs::write(
        &hook,
        "#!/bin/sh\ngit update-ref -d refs/remotes/origin/feature/cleanup\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        &f.root,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    fs::write(f.root.join("file.txt"), "keep draft\n").unwrap();
    let r = request(&f);
    let p = f.app.preview_remote_delete(&f.id, &r).unwrap();
    let out = f.app.delete_remote_branch(&f.id, &r, &p.token).unwrap();
    assert_eq!(out.status, "succeeded", "{out:?}");
    assert!(!exists(&f.remote, "refs/heads/feature/cleanup"));
    assert!(!exists(&f.root, "refs/remotes/origin/feature/cleanup"));
    assert_eq!(git(&f.root, &["rev-parse", "feature/cleanup"]), f.oid);
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.oid);
    assert_eq!(
        fs::read_to_string(f.root.join("file.txt")).unwrap(),
        "keep draft\n"
    );
}
