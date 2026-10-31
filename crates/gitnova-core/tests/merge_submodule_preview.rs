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
fn fixture(ours: bool, incoming: bool) -> F {
    let dir = tempfile::tempdir().unwrap();
    let child_source = dir.path().join("child-source");
    let root = dir.path().join("parent");
    fs::create_dir(&child_source).unwrap();
    fs::create_dir(&root).unwrap();
    git(&child_source, &["init", "-b", "main"]);
    let mut ids = vec![];
    for name in ["a", "b", "c"] {
        fs::write(child_source.join("child.txt"), format!("{name}\n")).unwrap();
        git(&child_source, &["add", "."]);
        git(&child_source, &["commit", "-m", name]);
        ids.push(git(&child_source, &["rev-parse", "HEAD"]));
    }
    git(&root, &["init", "-b", "release"]);
    git(&root, &["config", "user.name", "Merge Pointer Test"]);
    git(&root, &["config", "user.email", "pointer@example.invalid"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    git(
        &root,
        &["submodule", "add", child_source.to_str().unwrap(), "_dags"],
    );
    let child = root.join("_dags");
    git(&child, &["checkout", "--detach", &ids[0]]);
    fs::write(root.join("base.txt"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Base pointer"]);
    git(&root, &["switch", "-c", "feature"]);
    if incoming {
        git(&child, &["checkout", "--detach", &ids[1]]);
        git(&root, &["add", "_dags"]);
    }
    fs::write(root.join("feature.txt"), "feature\n").unwrap();
    git(&root, &["add", "feature.txt"]);
    git(&root, &["commit", "-m", "Feature changes"]);
    let source = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["switch", "release"]);
    git(
        &child,
        &["checkout", "--detach", if ours { &ids[2] } else { &ids[0] }],
    );
    if ours {
        git(&root, &["add", "_dags"]);
        git(&root, &["commit", "-m", "Keep current child version"]);
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
        child,
        id,
        a: ids[0].clone(),
        b: ids[1].clone(),
        c: ids[2].clone(),
        source,
    }
}
fn request(f: &F) -> MergeRequest {
    MergeRequest {
        rebase_steps: vec![],
        acknowledge_rewrite: false,
        delete_source_branch: false,
        method: MergeMethod::MergeCommit,
        operation: "merge".into(),
        mainline: None,
        source_oid: f.source.clone(),
        source_ref: Some("refs/heads/feature".into()),
        session_id: String::new(),
    }
}
fn child_index(f: &F) -> Vec<u8> {
    let path = git(
        &f.child,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    );
    fs::read(path).unwrap()
}
#[test]
fn merge_that_keeps_current_gitlink_is_allowed_and_preserves_child_bytes_refs_and_index() {
    let f = fixture(true, false);
    let original = git(&f.root, &["rev-parse", "HEAD"]);
    let bytes = child_index(&f);
    let refs = git(&f.child, &["show-ref"]);
    let preview = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(
        preview.blocked_reason.is_none(),
        "{:?}",
        preview.blocked_reason
    );
    assert_eq!(preview.submodules.len(), 1);
    let s = &preview.submodules[0];
    assert_eq!(s.path.display, "_dags");
    assert_eq!(s.current_oid.as_deref(), Some(f.c.as_str()));
    assert_eq!(s.incoming_oid.as_deref(), Some(f.a.as_str()));
    assert_eq!(s.result_oid, s.current_oid);
    assert!(!s.conflicted);
    assert_eq!(child_index(&f), bytes);
    assert_eq!(git(&f.child, &["rev-parse", "HEAD"]), f.c);
    let receipt = f
        .app
        .begin_merge(&f.id, &preview.request, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "pending", "{receipt:?}");
    let finish = MergeAction {
        kind: "finish".into(),
        path: None,
        choice: None,
        content: None,
        message: "Merge feature while preserving current _dags".into(),
    };
    let review = f.app.preview_merge_action(&f.id, &finish).unwrap();
    assert!(
        review.blocked_reason.is_none(),
        "{:?}",
        review.blocked_reason
    );
    assert_eq!(
        f.app
            .apply_merge_action(&f.id, &finish, &review.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%P", "HEAD"]),
        format!("{} {}", original, f.source)
    );
    assert_eq!(git(&f.root, &["rev-parse", "HEAD:_dags"]), f.c);
    assert_eq!(git(&f.child, &["rev-parse", "HEAD"]), f.c);
    assert_eq!(child_index(&f), bytes);
    assert_eq!(git(&f.child, &["show-ref"]), refs);
    assert_eq!(
        fs::read_to_string(f.child.join("child.txt")).unwrap(),
        "c\n"
    );
}
#[test]
fn already_included_history_keeps_its_reason_even_if_branch_gitlinks_differ() {
    let f = fixture(true, false);
    git(
        &f.root,
        &["merge", "--no-ff", "feature", "-m", "Already included"],
    );
    let head = git(&f.root, &["rev-parse", "HEAD"]);
    let index = fs::read(f.root.join(".git/index")).unwrap();
    let preview = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert_eq!(preview.mode, "up_to_date");
    assert!(preview.blocked_reason.unwrap().contains("already included"));
    assert!(preview.submodules.is_empty());
    assert!(preview.result_tree.is_none());
    assert!(preview.files.is_empty());
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert_eq!(git(&f.child, &["rev-parse", "HEAD"]), f.c);
}
#[test]
fn actual_incoming_pointer_change_is_still_blocked_with_exact_evidence_without_child_updates() {
    let f = fixture(false, true);
    let head = git(&f.root, &["rev-parse", "HEAD"]);
    let bytes = child_index(&f);
    let preview = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(preview
        .blocked_reason
        .as_ref()
        .unwrap()
        .contains("predicted merge changes submodule"));
    let s = &preview.submodules[0];
    assert_eq!(s.current_oid.as_deref(), Some(f.a.as_str()));
    assert_eq!(s.incoming_oid.as_deref(), Some(f.b.as_str()));
    assert_eq!(s.result_oid.as_deref(), Some(f.b.as_str()));
    let receipt = f
        .app
        .begin_merge(&f.id, &preview.request, &preview.token)
        .unwrap();
    assert!(!["succeeded", "pending", "conflicted"].contains(&receipt.status.as_str()));
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&f.child, &["rev-parse", "HEAD"]), f.a);
    assert_eq!(child_index(&f), bytes);
    assert!(!f.root.join(".git/MERGE_HEAD").exists());
}
#[test]
fn unresolved_submodule_conflict_remains_blocked_even_when_result_tree_keeps_ours() {
    let f = fixture(true, true);
    let bytes = child_index(&f);
    let sentinel = f.root.join("must-not-run");
    git(
        &f.root,
        &[
            "config",
            "submodule._dags.update",
            &format!("!touch {}", sentinel.display()),
        ],
    );
    let preview = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(preview.blocked_reason.is_some());
    let s = &preview.submodules[0];
    assert!(s.conflicted);
    assert!(s.result_oid.is_none());
    assert_eq!(s.current_oid.as_deref(), Some(f.c.as_str()));
    assert_eq!(git(&f.child, &["rev-parse", "HEAD"]), f.c);
    assert_eq!(child_index(&f), bytes);
    assert!(!sentinel.exists());
}
