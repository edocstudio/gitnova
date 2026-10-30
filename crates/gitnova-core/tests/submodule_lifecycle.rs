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
    String::from_utf8(out.stdout).unwrap().trim_end().into()
}
fn init(path: &Path) {
    fs::create_dir(path).unwrap();
    git(path, &["init", "-b", "main"]);
    git(path, &["config", "user.name", "Fixture"]);
    git(path, &["config", "user.email", "fixture@example.invalid"]);
    git(path, &["config", "commit.gpgsign", "false"]);
    fs::write(path.join("file"), "base\n").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-m", "base"]);
}
struct F {
    _dir: tempfile::TempDir,
    root: PathBuf,
    source: PathBuf,
    app: Application,
    id: String,
}
fn fixture() -> F {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("parent");
    let source = dir.path().join("source");
    init(&root);
    init(&source);
    let app = Application::new(dir.path().join("catalog.json")).unwrap();
    let id = app.add_repository(&root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    F {
        _dir: dir,
        root,
        source,
        app,
        id,
    }
}
fn request(f: &F, kind: &str) -> SubmoduleLifecycleRequest {
    SubmoduleLifecycleRequest {
        kind: kind.into(),
        path: "child".into(),
        source: if kind == "add" {
            f.source.to_str().unwrap().into()
        } else {
            String::new()
        },
        id: String::new(),
    }
}
#[test]
fn add_download_and_remove_registration_keep_unrelated_staging_child_work_and_object_store() {
    let f = fixture();
    fs::write(f.root.join("prepared"), "staged unrelated\n").unwrap();
    git(&f.root, &["add", "prepared"]);
    let head = git(&f.root, &["rev-parse", "HEAD"]);
    let p = f
        .app
        .preview_submodule_lifecycle(&f.id, &request(&f, "add"))
        .unwrap();
    assert!(p.download);
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert!(!f.root.join("child").exists());
    let r = f
        .app
        .change_submodule_lifecycle(&f.id, &p.request, &p.token)
        .unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(git(&f.root, &["show", ":prepared"]), "staged unrelated");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), head);
    assert!(git(&f.root, &["ls-files", "--stage", "child"]).starts_with("160000 "));
    assert!(f.root.join("child/file").exists());
    let commit = f.app.commit_preview(&f.id).unwrap();
    assert!(commit.files.iter().any(|f| f.path.display == "child"));
    assert_eq!(
        f.app
            .create_commit(&f.id, "record child", &commit.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let child = f.root.join("child");
    let oid = git(&child, &["rev-parse", "HEAD"]);
    let admin = git(&child, &["rev-parse", "--absolute-git-dir"]);
    fs::write(child.join("file"), "retain dirty child\n").unwrap();
    let p = f
        .app
        .preview_submodule_lifecycle(&f.id, &request(&f, "remove"))
        .unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let r = f
        .app
        .change_submodule_lifecycle(&f.id, &p.request, &p.token)
        .unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(git(&f.root, &["ls-files", "--", "child"]), "");
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), oid);
    assert_eq!(
        fs::read_to_string(child.join("file")).unwrap(),
        "retain dirty child\n"
    );
    assert!(Path::new(&admin).exists());
    let commit = f.app.commit_preview(&f.id).unwrap();
    assert_eq!(
        f.app
            .create_commit(&f.id, "remove registration", &commit.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert!(p.recovery_dir.to_path().unwrap().join("index").exists());
}
#[test]
fn existing_independent_child_is_attached_without_checkout_or_branch_change() {
    let f = fixture();
    let child = f.root.join("child");
    init(&child);
    let before = git(&child, &["rev-parse", "HEAD"]);
    let p = f
        .app
        .preview_submodule_lifecycle(&f.id, &request(&f, "add"))
        .unwrap();
    assert!(!p.download);
    assert_eq!(p.child_head.as_deref(), Some(before.as_str()));
    let r = f
        .app
        .change_submodule_lifecycle(&f.id, &p.request, &p.token)
        .unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(git(&child, &["symbolic-ref", "--short", "HEAD"]), "main");
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), before);
}
#[test]
fn stale_parent_or_existing_foreign_lock_never_changes_registration() {
    let f = fixture();
    let p = f
        .app
        .preview_submodule_lifecycle(&f.id, &request(&f, "add"))
        .unwrap();
    git(&f.root, &["config", "test.changed", "yes"]);
    let r = f
        .app
        .change_submodule_lifecycle(&f.id, &p.request, &p.token)
        .unwrap();
    assert_ne!(r.status, "succeeded");
    assert!(!f.root.join("child").exists());
    let p = f
        .app
        .preview_submodule_lifecycle(&f.id, &request(&f, "add"))
        .unwrap();
    fs::write(f.root.join(".git/index.lock"), "foreign").unwrap();
    let r = f
        .app
        .change_submodule_lifecycle(&f.id, &p.request, &p.token)
        .unwrap();
    assert_ne!(r.status, "succeeded");
    assert_eq!(
        fs::read_to_string(f.root.join(".git/index.lock")).unwrap(),
        "foreign"
    );
    assert!(!f.root.join("child").exists());
}
#[test]
fn tracked_or_edited_manifest_destinations_and_path_escape_are_refused() {
    let f = fixture();
    let mut r = request(&f, "add");
    r.path = "../escape".into();
    assert!(f.app.preview_submodule_lifecycle(&f.id, &r).is_err());
    r.path = "file".into();
    let p = f.app.preview_submodule_lifecycle(&f.id, &r).unwrap();
    assert!(p.blocked_reason.is_some());
    assert_ne!(
        f.app
            .change_submodule_lifecycle(&f.id, &p.request, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(fs::read_to_string(f.root.join("file")).unwrap(), "base\n");
    fs::write(f.root.join(".gitmodules"), "# uncommitted manifest\n").unwrap();
    let p = f
        .app
        .preview_submodule_lifecycle(&f.id, &request(&f, "add"))
        .unwrap();
    assert!(p.blocked_reason.is_some());
}
