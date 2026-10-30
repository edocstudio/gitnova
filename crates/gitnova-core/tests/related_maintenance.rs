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
            destination: String::new(),
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
        destination: String::new(),
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
        destination: String::new(),
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
        destination: String::new(),
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

#[test]
fn sync_relative_urls_matches_native_git_and_revalidates_parent_remote() {
    for (base, relative, expected) in [
        (
            "https://example.invalid/team/parent.git",
            "../child.git",
            "https://example.invalid/team/child.git",
        ),
        (
            "ssh://git@example.invalid:2222/team/parent.git",
            "../child.git",
            "ssh://git@example.invalid:2222/team/child.git",
        ),
        (
            "git@example.invalid:team/parent.git",
            "../../other/child.git",
            "git@example.invalid:other/child.git",
        ),
    ] {
        let f = sub();
        let child = f.root.join("child");
        git(&f.root, &["remote", "add", "origin", base]);
        git(
            &f.root,
            &[
                "config",
                "--file",
                ".gitmodules",
                "submodule.child.url",
                relative,
            ],
        );
        fs::write(child.join("file"), "keep dirty relative sync\n").unwrap();
        let head = git(&child, &["rev-parse", "HEAD"]);
        let before = fs::read(f.root.join(".git/index")).unwrap();
        let p = f
            .app
            .preview_submodule_sync(&f.id, &NativePath::new(Path::new("child")))
            .unwrap();
        assert!(p.blocked_reason.is_none(), "{p:?}");
        let outcome = f
            .app
            .sync_submodule(&f.id, &p.submodule.path, &p.token)
            .unwrap();
        assert_eq!(outcome.status, "succeeded", "{outcome:?}");
        assert_eq!(git(&f.root, &["config", "submodule.child.url"]), expected);
        assert_eq!(git(&child, &["config", "remote.origin.url"]), expected);
        assert_eq!(git(&child, &["rev-parse", "HEAD"]), head);
        assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), before);
        assert_eq!(
            fs::read_to_string(child.join("file")).unwrap(),
            "keep dirty relative sync\n"
        );
        git(
            &f.root,
            &[
                "config",
                "--file",
                ".gitmodules",
                "submodule.child.url",
                "../another.git",
            ],
        );
        let p = f
            .app
            .preview_submodule_sync(&f.id, &p.submodule.path)
            .unwrap();
        git(
            &f.root,
            &[
                "remote",
                "set-url",
                "origin",
                "https://example.invalid/changed/parent.git",
            ],
        );
        assert_ne!(
            f.app
                .sync_submodule(&f.id, &p.submodule.path, &p.token)
                .unwrap()
                .status,
            "succeeded"
        );
        assert_eq!(git(&child, &["config", "remote.origin.url"]), expected);
    }
}

#[test]
fn ssh_display_redaction_never_changes_sync_postconditions() {
    let f = sub();
    let url = "ssh://git@example.invalid:2222/team/child.git";
    git(
        &f.root,
        &[
            "config",
            "--file",
            ".gitmodules",
            "submodule.child.url",
            url,
        ],
    );
    let p = f
        .app
        .preview_submodule_sync(&f.id, &NativePath::new(Path::new("child")))
        .unwrap();
    assert!(p.url.contains("[credentials]"));
    let receipt = f
        .app
        .sync_submodule(&f.id, &p.submodule.path, &p.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{receipt:?}");
    assert_eq!(
        git(&f.root.join("child"), &["config", "remote.origin.url"]),
        url
    );
}

#[test]
fn relative_local_sync_without_a_parent_remote_matches_native_git() {
    let f = sub();
    git(
        &f.root,
        &[
            "config",
            "--file",
            ".gitmodules",
            "submodule.child.url",
            "../source-new",
        ],
    );
    let p = f
        .app
        .preview_submodule_sync(&f.id, &NativePath::new(Path::new("child")))
        .unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    let result = f
        .app
        .sync_submodule(&f.id, &p.submodule.path, &p.token)
        .unwrap();
    assert_eq!(result.status, "succeeded", "{result:?}");
    assert_eq!(
        git(&f.root.join("child"), &["config", "remote.origin.url"]),
        p.url
    );
}

#[test]
fn moved_worktree_reattach_preserves_dirty_index_branch_and_workspace_identity() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("parent");
    repo(&root);
    let linked = d.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    let (a, id) = app(&root, d.path());
    let catalog = a.add_repository(&linked).unwrap();
    let linked_id = catalog.workspaces[0].selected_id.clone().unwrap();
    let worktree_id = catalog.workspaces[0]
        .memberships
        .iter()
        .find(|m| m.id == linked_id)
        .unwrap()
        .worktree_id
        .clone();
    fs::write(linked.join("file"), "staged child\n").unwrap();
    git(&linked, &["add", "file"]);
    fs::write(linked.join("file"), "working child\n").unwrap();
    let before = a
        .worktrees(&id)
        .unwrap()
        .into_iter()
        .find(|t| t.branch.as_deref() == Some("side"))
        .unwrap();
    let old = before.path.to_path().unwrap();
    let moved = old.with_file_name("moved");
    fs::rename(&old, &moved).unwrap();
    let moved = moved.canonicalize().unwrap();
    let admin = PathBuf::from(git(&moved, &["rev-parse", "--absolute-git-dir"]));
    let index = fs::read(admin.join("index")).unwrap();
    let head = fs::read(admin.join("HEAD")).unwrap();
    let r = WorktreeMaintenanceRequest {
        path: before.path.clone(),
        kind: "reattach".into(),
        destination: moved.to_str().unwrap().into(),
        reason: String::new(),
    };
    let p = a.preview_worktree_maintenance(&id, &r).unwrap();
    assert!(p.tree.missing);
    assert!(p.blocked_reason.is_none());
    assert!(!old.exists());
    assert_eq!(
        a.maintain_worktree(&id, &r, &p.token).unwrap().status,
        "succeeded"
    );
    assert_eq!(
        a.status(&linked_id).unwrap().branch.as_deref(),
        Some("side")
    );
    assert_eq!(fs::read(admin.join("index")).unwrap(), index);
    assert_eq!(fs::read(admin.join("HEAD")).unwrap(), head);
    assert_eq!(
        fs::read_to_string(moved.join("file")).unwrap(),
        "working child\n"
    );
    let c = a.catalog().unwrap();
    assert_eq!(
        c.workspaces[0]
            .memberships
            .iter()
            .find(|m| m.id == linked_id)
            .unwrap()
            .worktree_id,
        worktree_id
    );
    assert!(c
        .worktrees
        .iter()
        .any(|w| Some(&w.id) == worktree_id.as_ref() && w.root.to_path().unwrap() == moved));
    assert!(p
        .recovery_dir
        .unwrap()
        .to_path()
        .unwrap()
        .join("index")
        .exists());
}
#[test]
fn moved_worktree_repair_rejects_wrong_repository_and_stale_locator() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("parent");
    repo(&root);
    let linked = d.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    let (a, id) = app(&root, d.path());
    let tree = a
        .worktrees(&id)
        .unwrap()
        .into_iter()
        .find(|t| t.branch.as_deref() == Some("side"))
        .unwrap();
    let old = tree.path.to_path().unwrap();
    let moved = old.with_file_name("moved");
    fs::rename(&old, &moved).unwrap();
    let moved = moved.canonicalize().unwrap();
    let mut r = WorktreeMaintenanceRequest {
        path: tree.path,
        kind: "reattach".into(),
        destination: moved.to_str().unwrap().into(),
        reason: String::new(),
    };
    let p = a.preview_worktree_maintenance(&id, &r).unwrap();
    let gitfile = fs::read(moved.join(".git")).unwrap();
    fs::write(moved.join(".git"), b"gitdir: /nonexistent\n").unwrap();
    assert!(a.maintain_worktree(&id, &r, &p.token).is_err());
    assert!(!old.exists());
    fs::write(moved.join(".git"), gitfile).unwrap();
    let foreign = d.path().join("foreign");
    repo(&foreign);
    r.destination = foreign.canonicalize().unwrap().to_str().unwrap().into();
    assert!(a.preview_worktree_maintenance(&id, &r).is_err());
}
