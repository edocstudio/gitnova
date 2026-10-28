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
            "user.name=Merge Pointer Test",
            "-c",
            "user.email=pointer@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "submodule.recurse=false",
            "-c",
            "protocol.file.allow=always",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
struct F {
    _dir: tempfile::TempDir,
    app: Application,
    root: PathBuf,
    child: PathBuf,
    id: String,
    a: String,
    b: String,
    c: String,
    source: String,
}
fn fixture() -> F {
    let dir = tempfile::tempdir().unwrap();
    let origin = dir.path().join("child-source");
    let root = dir.path().join("parent");
    fs::create_dir(&origin).unwrap();
    fs::create_dir(&root).unwrap();
    git(&origin, &["init", "-b", "main"]);
    let mut ids = vec![];
    for name in ["a", "b", "c"] {
        fs::write(origin.join("child.txt"), format!("{name}\n")).unwrap();
        git(&origin, &["add", "."]);
        git(&origin, &["commit", "-m", name]);
        ids.push(git(&origin, &["rev-parse", "HEAD"]));
    }
    git(&root, &["init", "-b", "main"]);
    git(
        &root,
        &["submodule", "add", origin.to_str().unwrap(), "_dags"],
    );
    let child = root.join("_dags");
    git(&child, &["checkout", "--detach", &ids[0]]);
    fs::write(root.join("base.txt"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Main records a"]);
    git(&root, &["switch", "-c", "release"]);
    git(&child, &["checkout", "--detach", &ids[1]]);
    git(&root, &["add", "_dags"]);
    git(&root, &["commit", "-m", "Release records b"]);
    git(&root, &["switch", "-c", "feature"]);
    fs::write(root.join("query.sql"), "select 1;\n").unwrap();
    git(&root, &["add", "query.sql"]);
    git(&root, &["commit", "-m", "SQL feature"]);
    git(&root, &["switch", "release"]);
    git(
        &root,
        &["merge", "--no-ff", "feature", "-m", "Source merge"],
    );
    let source = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["switch", "main"]); // Deliberately keep the clean child at b.
    let app = Application::new(dir.path().join("state/catalog.json")).unwrap();
    let id = app.add_repository(&root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    F {
        _dir: dir,
        app,
        root,
        child,
        id,
        a: ids[0].clone(),
        b: ids[1].clone(),
        c: ids[2].clone(),
        source,
    }
}
fn request(f: &F, mainline: Option<u32>) -> MergeRequest {
    MergeRequest {
        acknowledge_rewrite: false,
        delete_source_branch: false,
        method: MergeMethod::Auto,
        operation: "cherryPick".into(),
        mainline,
        source_oid: f.source.clone(),
        source_ref: Some("refs/heads/release".into()),
        session_id: String::new(),
    }
}
fn action(kind: &str) -> MergeAction {
    MergeAction {
        kind: kind.into(),
        path: None,
        choice: None,
        content: None,
        message: if kind == "finish" {
            "Reviewed SQL change".into()
        } else {
            String::new()
        },
    }
}
fn child_state(f: &F) -> (String, String, Vec<u8>, Vec<u8>) {
    let index = git(
        &f.child,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    );
    (
        git(&f.child, &["rev-parse", "HEAD"]),
        git(&f.child, &["show-ref"]),
        fs::read(index).unwrap(),
        fs::read(f.child.join("child.txt")).unwrap(),
    )
}
fn start(f: &F) {
    let p = f.app.preview_merge(&f.id, &request(f, Some(1))).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert_eq!(p.preserved_submodules.len(), 1);
    assert_eq!(p.preserved_submodules[0].recorded_oid, f.a);
    assert_eq!(p.preserved_submodules[0].checkout_oid, f.b);
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.files[0].path.display, "query.sql");
    let r = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(r.status, "pending", "{} {:?}", r.summary, r.recovery);
}
#[test]
fn merge_pick_requires_parent_then_finishes_sql_only_without_touching_child() {
    let f = fixture();
    let before = child_state(&f);
    let original = git(&f.root, &["rev-parse", "HEAD"]);
    let p = f.app.preview_merge(&f.id, &request(&f, None)).unwrap();
    assert!(p.blocked_reason.unwrap().contains("mainline parent"));
    assert_eq!(child_state(&f), before);
    start(&f);
    assert_eq!(child_state(&f), before);
    let a = action("finish");
    let p = f.app.preview_merge_action(&f.id, &a).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let r = f.app.apply_merge_action(&f.id, &a, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{}", r.summary);
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%P", "HEAD"]),
        original
    );
    assert_eq!(git(&f.root, &["rev-parse", "HEAD:_dags"]), f.a);
    assert_eq!(
        fs::read_to_string(f.root.join("query.sql")).unwrap(),
        "select 1;\n"
    );
    assert_eq!(child_state(&f), before);
    assert_eq!(f.app.merge_state(&f.id).unwrap().phase, "none");
}
#[test]
fn abort_archives_parent_changes_without_backing_up_or_resetting_child() {
    let f = fixture();
    let before = child_state(&f);
    let original = git(&f.root, &["rev-parse", "HEAD"]);
    start(&f);
    let a = action("abort");
    let p = f.app.preview_merge_action(&f.id, &a).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert!(p.paths.iter().all(|p| p.display != "_dags"));
    let r = f.app.apply_merge_action(&f.id, &a, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{}", r.summary);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), original);
    assert!(!f.root.join("query.sql").exists());
    assert_eq!(child_state(&f), before);
    assert!(p
        .recovery_dir
        .unwrap()
        .to_path()
        .unwrap()
        .join("manifest.json")
        .exists());
}
#[test]
fn changed_child_invalidates_start_review_without_native_write() {
    let f = fixture();
    let p = f.app.preview_merge(&f.id, &request(&f, Some(1))).unwrap();
    git(&f.child, &["checkout", "--detach", &f.c]);
    assert!(f.app.begin_merge(&f.id, &p.request, &p.token).is_err());
    assert!(!f.root.join("query.sql").exists());
    assert!(!f.root.join(".git/gitnova-merge.json").exists());
    assert_eq!(git(&f.child, &["rev-parse", "HEAD"]), f.c);
}
#[test]
fn ordinary_dirty_staged_pointer_and_dirty_child_remain_blocked() {
    let f = fixture();
    for (path, content) in [
        (f.root.join("base.txt"), "parent work"),
        (f.child.join("child.txt"), "child work"),
        (f.child.join("new.txt"), "untracked child"),
    ] {
        let original = fs::read(&path).ok();
        fs::write(&path, content).unwrap();
        let p = f.app.preview_merge(&f.id, &request(&f, Some(1))).unwrap();
        assert!(p.blocked_reason.is_some());
        assert!(p.preserved_submodules.is_empty());
        if let Some(bytes) = original {
            fs::write(&path, bytes).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
    }
    git(&f.root, &["add", "_dags"]);
    assert!(f
        .app
        .preview_merge(&f.id, &request(&f, Some(1)))
        .unwrap()
        .blocked_reason
        .is_some());
}
#[test]
fn active_operation_refuses_child_or_parent_pointer_changes_before_finish_and_abort() {
    let f = fixture();
    start(&f);
    git(&f.child, &["checkout", "--detach", &f.c]);
    for kind in ["finish", "abort"] {
        assert!(f.app.preview_merge_action(&f.id, &action(kind)).is_err());
    }
    git(&f.child, &["checkout", "--detach", &f.b]);
    git(&f.root, &["add", "_dags"]);
    for kind in ["finish", "abort"] {
        assert!(f.app.preview_merge_action(&f.id, &action(kind)).is_err());
    }
    assert_eq!(git(&f.child, &["rev-parse", "HEAD"]), f.b);
    assert!(f.root.join("query.sql").exists());
}
#[test]
fn selected_delta_that_changes_gitlink_or_submodule_configuration_is_blocked() {
    let mut f = fixture();
    git(&f.root, &["switch", "release"]);
    git(&f.child, &["checkout", "--detach", &f.c]);
    git(&f.root, &["add", "_dags"]);
    git(&f.root, &["commit", "-m", "Pointer change"]);
    f.source = git(&f.root, &["rev-parse", "HEAD"]);
    git(&f.root, &["switch", "main"]);
    let p = f.app.preview_merge(&f.id, &request(&f, None)).unwrap();
    assert!(p.blocked_reason.is_some());
    git(&f.root, &["switch", "release"]);
    fs::write(f.root.join(".gitmodules"), "# changed configuration\n").unwrap();
    git(&f.root, &["commit", "-am", "Layout change"]);
    f.source = git(&f.root, &["rev-parse", "HEAD"]);
    git(&f.root, &["switch", "main"]);
    assert!(f
        .app
        .preview_merge(&f.id, &request(&f, None))
        .unwrap()
        .blocked_reason
        .is_some());
}
#[test]
fn revert_finishes_inverse_delta_while_preserving_clean_child_mismatch() {
    let f = fixture();
    start(&f);
    let a = action("finish");
    let p = f.app.preview_merge_action(&f.id, &a).unwrap();
    assert_eq!(
        f.app
            .apply_merge_action(&f.id, &a, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let mut r = request(&f, None);
    r.operation = "revert".into();
    r.source_ref = None;
    r.source_oid = git(&f.root, &["rev-parse", "HEAD"]);
    let before = child_state(&f);
    let p = f.app.preview_merge(&f.id, &r).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert_eq!(
        f.app
            .begin_merge(&f.id, &p.request, &p.token)
            .unwrap()
            .status,
        "pending"
    );
    let a = action("finish");
    let p = f.app.preview_merge_action(&f.id, &a).unwrap();
    assert_eq!(
        f.app
            .apply_merge_action(&f.id, &a, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert!(!f.root.join("query.sql").exists());
    assert_eq!(child_state(&f), before);
}
