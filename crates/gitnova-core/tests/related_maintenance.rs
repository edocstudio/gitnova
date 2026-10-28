use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(root: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("-c")
        .arg("protocol.file.allow=always")
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap().trim_end().into()
}
fn repo(path: &Path) {
    fs::create_dir(path).unwrap();
    git(path, &["init", "-b", "main"]);
    git(path, &["config", "user.name", "Fixture"]);
    git(path, &["config", "user.email", "test@example.invalid"]);
    git(path, &["config", "commit.gpgsign", "false"]);
    fs::write(path.join("file"), "base\n").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-m", "base"]);
}
fn app(root: &Path, dir: &Path) -> (Application, String) {
    let a = Application::new(dir.join("catalog.json")).unwrap();
    let id = a.add_repository(root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    (a, id)
}
#[test]
fn lock_unlock_and_repair_preserve_dirty_checkout_and_refs() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(&root);
    let linked = d.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    fs::write(linked.join("file"), "keep dirty\n").unwrap();
    let head = git(&root, &["rev-parse", "side"]);
    let (a, id) = app(&root, d.path());
    for kind in ["lock", "unlock", "repair"] {
        let request = WorktreeMaintenanceRequest {
            path: NativePath::new(&linked),
            kind: kind.into(),
            reason: if kind == "lock" {
                "offline checkout"
            } else {
                ""
            }
            .into(),
        };
        let p = a.preview_worktree_maintenance(&id, &request).unwrap();
        assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
        assert_eq!(
            a.maintain_worktree(&id, &request, &p.token).unwrap().status,
            "succeeded"
        );
        assert_eq!(
            fs::read_to_string(linked.join("file")).unwrap(),
            "keep dirty\n"
        );
        assert_eq!(git(&root, &["rev-parse", "side"]), head);
    }
    assert!(linked.exists());
}
#[test]
fn stale_lock_state_and_main_checkout_maintenance_are_refused() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(&root);
    let linked = d.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    let (a, id) = app(&root, d.path());
    let mut r = WorktreeMaintenanceRequest {
        path: NativePath::new(&linked),
        kind: "lock".into(),
        reason: String::new(),
    };
    let p = a.preview_worktree_maintenance(&id, &r).unwrap();
    git(&root, &["worktree", "lock", linked.to_str().unwrap()]);
    assert_ne!(
        a.maintain_worktree(&id, &r, &p.token).unwrap().status,
        "succeeded"
    );
    r.path = NativePath::new(&root);
    assert!(a
        .preview_worktree_maintenance(&id, &r)
        .unwrap()
        .blocked_reason
        .is_some());
}
struct S {
    _dir: tempfile::TempDir,
    root: PathBuf,
    new: PathBuf,
    app: Application,
    id: String,
}
fn sub() -> S {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("parent");
    let old = d.path().join("source-old");
    let new = d.path().join("source-new");
    repo(&root);
    repo(&old);
    repo(&new);
    git(&root, &["submodule", "add", old.to_str().unwrap(), "child"]);
    git(&root, &["commit", "-am", "record child"]);
    git(
        &root,
        &[
            "config",
            "--file",
            ".gitmodules",
            "submodule.child.url",
            new.to_str().unwrap(),
        ],
    );
    let (a, id) = app(&root, d.path());
    S {
        _dir: d,
        root,
        new,
        app: a,
        id,
    }
}
#[test]
fn sync_urls_is_selected_only_and_keeps_dirty_files_head_and_index() {
    let f = sub();
    let child = f.root.join("child");
    let head = git(&child, &["rev-parse", "HEAD"]);
    let index = fs::read(f.root.join(".git/index")).unwrap();
    fs::write(child.join("file"), "private work\n").unwrap();
    let p = f
        .app
        .preview_submodule_sync(&f.id, &NativePath::new(Path::new("child")))
        .unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let backup = p.recovery_dir.to_path().unwrap();
    let result = f
        .app
        .sync_submodule(&f.id, &p.submodule.path, &p.token)
        .unwrap();
    assert_eq!(result.status, "succeeded", "{result:?}");
    assert_eq!(
        git(&f.root, &["config", "submodule.child.url"]),
        f.new.to_str().unwrap()
    );
    assert_eq!(
        git(&child, &["config", "remote.origin.url"]),
        f.new.to_str().unwrap()
    );
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(child.join("file")).unwrap(),
        "private work\n"
    );
    assert!(backup.join("child-config").exists());
}
#[test]
fn stale_submodule_url_preview_is_refused_before_config_changes() {
    let f = sub();
    let p = f
        .app
        .preview_submodule_sync(&f.id, &NativePath::new(Path::new("child")))
        .unwrap();
    let previous = git(&f.root, &["config", "submodule.child.url"]);
    git(
        &f.root,
        &[
            "config",
            "--file",
            ".gitmodules",
            "submodule.child.url",
            "https://example.invalid/replacement.git",
        ],
    );
    assert_ne!(
        f.app
            .sync_submodule(&f.id, &p.submodule.path, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["config", "submodule.child.url"]), previous);
}

#[test]
fn repair_restores_a_broken_linked_locator_without_deleting_dirty_work() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(&root);
    let linked = d.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    fs::write(linked.join("file"), "keep dirty repair\n").unwrap();
    let (a, id) = app(&root, d.path());
    fs::write(
        linked.join(".git"),
        "gitdir: /missing-gitnova-fixture-only\n",
    )
    .unwrap();
    let r = WorktreeMaintenanceRequest {
        path: NativePath::new(&linked),
        kind: "repair".into(),
        reason: String::new(),
    };
    let p = a.preview_worktree_maintenance(&id, &r).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let result = a.maintain_worktree(&id, &r, &p.token).unwrap();
    assert_eq!(result.status, "succeeded", "{result:?}");
    assert_eq!(git(&linked, &["branch", "--show-current"]), "side");
    assert_eq!(
        fs::read_to_string(linked.join("file")).unwrap(),
        "keep dirty repair\n"
    );
}

#[test]
fn repair_refuses_a_registered_locator_redirected_to_another_repository() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    let foreign = d.path().join("foreign");
    repo(&root);
    repo(&foreign);
    let linked = d.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    let (a, id) = app(&root, d.path());
    let locator = format!("gitdir: {}\n", foreign.join(".git").display());
    fs::write(linked.join(".git"), &locator).unwrap();
    let r = WorktreeMaintenanceRequest {
        path: NativePath::new(&linked),
        kind: "repair".into(),
        reason: String::new(),
    };
    let p = a.preview_worktree_maintenance(&id, &r).unwrap();
    assert!(p.blocked_reason.is_some());
    assert_ne!(
        a.maintain_worktree(&id, &r, &p.token).unwrap().status,
        "succeeded"
    );
    assert_eq!(fs::read_to_string(linked.join(".git")).unwrap(), locator);
    assert_eq!(git(&foreign, &["status", "--porcelain"]), "");
}
