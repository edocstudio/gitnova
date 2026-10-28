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
struct F {
    _dir: tempfile::TempDir,
    root: PathBuf,
    app: Application,
    id: String,
    target: String,
    original: String,
}
fn fixture(conflict: bool) -> F {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Fixture"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    git(&root, &["config", "core.hooksPath", ".git/no-hooks"]);
    fs::write(root.join("file"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);
    git(&root, &["checkout", "-b", "feature"]);
    fs::write(root.join("file"), "feature\n").unwrap();
    git(&root, &["add", "."]);
    git(
        &root,
        &[
            "commit",
            "--author",
            "Original Author <original@example.invalid>",
            "--date",
            "2020-01-02T03:04:05+0530",
            "-m",
            "feature",
        ],
    );
    let original = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "main"]);
    fs::write(
        root.join(if conflict { "file" } else { "target" }),
        "target\n",
    )
    .unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "target"]);
    let target = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "feature"]);
    let app = Application::new(dir.path().join("catalog.json")).unwrap();
    let id = app.add_repository(&root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    F {
        _dir: dir,
        root,
        app,
        id,
        target,
        original,
    }
}
fn request(f: &F) -> MergeRequest {
    MergeRequest {
        acknowledge_rewrite: true,
        delete_source_branch: false,
        method: MergeMethod::Auto,
        operation: "rebase".into(),
        mainline: None,
        source_oid: f.target.clone(),
        source_ref: Some("refs/heads/main".into()),
        session_id: String::new(),
    }
}
fn action(kind: &str) -> MergeAction {
    MergeAction {
        kind: kind.into(),
        path: None,
        choice: None,
        content: None,
        message: String::new(),
    }
}
#[test]
fn clean_native_replay_preserves_authors_target_and_recovery() {
    let f = fixture(false);
    let original_author = git(
        &f.root,
        &["show", "-s", "--format=%an %ae %at", &f.original],
    );
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert_eq!(p.rebase.as_ref().unwrap().commits, vec![f.original.clone()]);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.original);
    let r = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_ne!(git(&f.root, &["rev-parse", "HEAD"]), f.original);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD^"]), f.target);
    assert_eq!(git(&f.root, &["rev-parse", "main"]), f.target);
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%an %ae %at", "HEAD"]),
        original_author
    );
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.original);
    assert!(git(&f.root, &["status", "--porcelain"]).is_empty());
}
#[test]
fn conflict_restart_resolve_continue_uses_owned_state() {
    let f = fixture(true);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let r = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(r.status, "conflicted", "{r:?}");
    let app = Application::new(f._dir.path().join("catalog.json")).unwrap();
    let state = app.snapshot(&f.id).unwrap().merge;
    assert_eq!(state.operation, "rebase");
    assert_eq!(state.phase, "pending", "{state:?}");
    assert_eq!(state.conflicts.len(), 1);
    let resolve = MergeAction {
        kind: "resolve".into(),
        path: Some(state.conflicts[0].path.clone()),
        choice: Some("edited".into()),
        content: Some("combined\n".into()),
        message: String::new(),
    };
    let p = app.preview_merge_action(&f.id, &resolve).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let r = app.apply_merge_action(&f.id, &resolve, &p.token).unwrap();
    assert_eq!(r.status, "pending", "{r:?}");
    let next = action("finish");
    let p = app.preview_merge_action(&f.id, &next).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let r = app.apply_merge_action(&f.id, &next, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(
        fs::read_to_string(f.root.join("file")).unwrap(),
        "combined\n"
    );
    assert_eq!(git(&f.root, &["rev-parse", "HEAD^"]), f.target);
}
#[test]
fn abort_retains_edited_conflict_and_restores_original_branch() {
    let f = fixture(true);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    fs::write(f.root.join("file"), "precious conflict edit\n").unwrap();
    let a = action("abort");
    let p = f.app.preview_merge_action(&f.id, &a).unwrap();
    let location = p.recovery_dir.clone().unwrap().to_path().unwrap();
    let r = f.app.apply_merge_action(&f.id, &a, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.original);
    assert_eq!(git(&f.root, &["branch", "--show-current"]), "feature");
    assert!(location.exists());
    let files = fs::read_dir(location)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert!(!files.is_empty());
}
#[test]
fn dirty_merge_range_custom_driver_and_foreign_metadata_are_refused() {
    let f = fixture(false);
    fs::write(f.root.join("untracked"), "keep\n").unwrap();
    assert!(f
        .app
        .preview_merge(&f.id, &request(&f))
        .unwrap()
        .blocked_reason
        .is_some());
    fs::remove_file(f.root.join("untracked")).unwrap();
    git(
        &f.root,
        &["config", "merge.custom.driver", "touch SHOULD_NOT_EXIST"],
    );
    assert!(f
        .app
        .preview_merge(&f.id, &request(&f))
        .unwrap()
        .blocked_reason
        .is_some());
    assert!(!f.root.join("SHOULD_NOT_EXIST").exists());
    git(&f.root, &["config", "--unset", "merge.custom.driver"]);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    let mut no_ack = p.request.clone();
    no_ack.acknowledge_rewrite = false;
    let p = f.app.preview_merge(&f.id, &no_ack).unwrap();
    assert_ne!(
        f.app
            .begin_merge(&f.id, &p.request, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.original);
}
#[test]
fn empty_replay_requires_explicit_skip_and_keeps_original_reachable() {
    let f = fixture(true);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    let state = f.app.snapshot(&f.id).unwrap().merge;
    let resolve = MergeAction {
        kind: "resolve".into(),
        path: Some(state.conflicts[0].path.clone()),
        choice: Some("ours".into()),
        content: None,
        message: String::new(),
    };
    let q = f.app.preview_merge_action(&f.id, &resolve).unwrap();
    f.app.apply_merge_action(&f.id, &resolve, &q.token).unwrap();
    assert!(f
        .app
        .preview_merge_action(&f.id, &action("finish"))
        .unwrap()
        .blocked_reason
        .is_some());
    let q = f.app.preview_merge_action(&f.id, &action("skip")).unwrap();
    assert!(q.blocked_reason.is_none(), "{:?}", q.blocked_reason);
    let r = f
        .app
        .apply_merge_action(&f.id, &action("skip"), &q.token)
        .unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.target);
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.original);
}
#[test]
fn originally_empty_commit_and_binary_rename_delete_keep_the_complete_sequence() {
    let f = fixture(false);
    git(
        &f.root,
        &["commit", "--allow-empty", "-m", "intentional empty"],
    );
    git(&f.root, &["mv", "file", "renamed file\t.txt"]);
    fs::write(f.root.join("binary"), b"hello\0binary").unwrap();
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-m", "rename and binary"]);
    git(&f.root, &["rm", "renamed file\t.txt"]);
    git(&f.root, &["commit", "-m", "delete renamed"]);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let count = p.rebase.as_ref().unwrap().commits.len();
    let r = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(
        git(
            &f.root,
            &["rev-list", "--count", &format!("{}..HEAD", f.target)]
        )
        .parse::<usize>()
        .unwrap(),
        count
    );
    assert_eq!(fs::read(f.root.join("binary")).unwrap(), b"hello\0binary");
    assert!(!f.root.join("renamed file\t.txt").exists());
}
#[test]
fn merge_topology_and_stale_start_are_refused() {
    let f = fixture(false);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    git(
        &f.root,
        &["commit", "--allow-empty", "-m", "external advance"],
    );
    assert!(f.app.begin_merge(&f.id, &p.request, &p.token).is_err());
    let base = git(&f.root, &["merge-base", "feature", "main"]);
    git(&f.root, &["checkout", "-b", "side", &base]);
    fs::write(f.root.join("side"), "side\n").unwrap();
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-m", "side"]);
    git(&f.root, &["checkout", "feature"]);
    git(&f.root, &["merge", "--no-ff", "side", "-m", "merge side"]);
    assert!(f
        .app
        .preview_merge(&f.id, &request(&f))
        .unwrap()
        .blocked_reason
        .unwrap()
        .contains("merge or root"));
}
#[test]
fn foreign_native_todo_and_interrupted_journal_never_authorize_continue() {
    let f = fixture(true);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    let todo = f.root.join(".git/rebase-merge/git-rebase-todo");
    fs::write(&todo, "exec echo forbidden\n").unwrap();
    let state = f.app.snapshot(&f.id).unwrap().merge;
    assert_eq!(state.phase, "incomplete");
    assert!(f
        .app
        .preview_merge_action(&f.id, &action("finish"))
        .unwrap()
        .blocked_reason
        .is_some());
    let marker = f.root.join(".git/gitnova-merge.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
    value["prepared"] = serde_json::json!(false);
    fs::write(&marker, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(f.app.snapshot(&f.id).unwrap().merge.phase, "incomplete");
    assert!(marker.exists());
}
#[test]
fn linked_worktree_rebase_leaves_the_other_checkout_and_branch_unchanged() {
    let f = fixture(false);
    let linked = f._dir.path().join("linked");
    git(
        &f.root,
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            linked.to_str().unwrap(),
            "feature",
        ],
    );
    let catalog = f.app.add_repository(&linked).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    let p = f.app.preview_merge(&id, &request(&f)).unwrap();
    assert_eq!(p.branch, "linked");
    let r = f.app.begin_merge(&id, &p.request, &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.original);
    assert_eq!(git(&f.root, &["branch", "--show-current"]), "feature");
    assert_eq!(
        fs::read_to_string(f.root.join("file")).unwrap(),
        "feature\n"
    );
    assert_eq!(git(&linked, &["rev-parse", "HEAD^"]), f.target);
}

#[test]
fn two_conflict_stops_require_two_separate_continues_and_keep_original_authors() {
    let f = fixture(true);
    fs::write(f.root.join("file"), "feature second\n").unwrap();
    git(&f.root, &["commit", "-am", "Second replay"]);
    let original = git(&f.root, &["rev-parse", "HEAD"]);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    let r = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(r.status, "conflicted");
    for text in ["combined first\n", "combined second\n"] {
        let state = f.app.snapshot(&f.id).unwrap().merge;
        assert_eq!(state.phase, "pending");
        assert_eq!(state.conflicts.len(), 1);
        let resolve = MergeAction {
            kind: "resolve".into(),
            path: Some(state.conflicts[0].path.clone()),
            choice: Some("edited".into()),
            content: Some(text.into()),
            message: String::new(),
        };
        let q = f.app.preview_merge_action(&f.id, &resolve).unwrap();
        f.app.apply_merge_action(&f.id, &resolve, &q.token).unwrap();
        let q = f
            .app
            .preview_merge_action(&f.id, &action("finish"))
            .unwrap();
        let r = f
            .app
            .apply_merge_action(&f.id, &action("finish"), &q.token)
            .unwrap();
        assert!(
            matches!(r.status.as_str(), "succeeded" | "conflicted"),
            "{r:?}"
        );
    }
    assert_eq!(git(&f.root, &["rev-parse", "HEAD^^"]), f.target);
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), original);
    assert_eq!(
        fs::read_to_string(f.root.join("file")).unwrap(),
        "combined second\n"
    );
}

#[test]
fn first_predicted_conflict_cannot_hide_later_submodule_pointer_changes() {
    let f = fixture(true);
    // A pair of gitlink edits leaves the final tree unchanged. The first replay
    // conflicts, but a later native Continue would still encounter these edits.
    git(
        &f.root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},child", f.target),
        ],
    );
    git(&f.root, &["commit", "-m", "add gitlink later in replay"]);
    git(&f.root, &["update-index", "--force-remove", "child"]);
    git(&f.root, &["commit", "-m", "remove gitlink again"]);
    let original = git(&f.root, &["rev-parse", "HEAD"]);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(p
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("submodule pointers"));
    assert_ne!(
        f.app
            .begin_merge(&f.id, &p.request, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), original);
    assert!(!f.root.join(".git/rebase-merge").exists());
}
