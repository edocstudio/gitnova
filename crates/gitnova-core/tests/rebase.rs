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
        rebase_steps: vec![],
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
fn stale_start_is_refused_and_two_parent_topology_is_reviewed_without_flattening() {
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
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert!(p.rebase.as_ref().unwrap().preserve_merges);
    let original = git(&f.root, &["rev-parse", "HEAD"]);
    let side = git(&f.root, &["rev-parse", "side"]);
    let result = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(result.status, "succeeded", "{result:?}");
    assert_eq!(
        git(&f.root, &["rev-list", "--parents", "-n", "1", "HEAD"])
            .split_whitespace()
            .count(),
        3
    );
    assert_eq!(git(&f.root, &["rev-parse", "side"]), side);
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), original);
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

#[test]
fn interrupted_rebase_journal_uses_metadata_only_cleanup_after_external_recovery() {
    let f = fixture(true);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    let started = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(started.status, "conflicted", "{}", started.summary);
    assert!(f
        .app
        .preview_merge_action(&f.id, &action("release"))
        .unwrap()
        .blocked_reason
        .is_some());
    git(&f.root, &["rebase", "--abort"]);
    git(
        &f.root,
        &["commit", "--allow-empty", "-m", "Continued after recovery"],
    );
    let head = git(&f.root, &["rev-parse", "HEAD"]);
    fs::write(f.root.join("file"), "private staged\n").unwrap();
    git(&f.root, &["add", "file"]);
    fs::write(f.root.join("file"), "private unstaged\n").unwrap();
    let index = fs::read(f.root.join(".git/index")).unwrap();
    let refs = git(&f.root, &["show-ref"]);
    let marker = fs::read(f.root.join(".git/gitnova-merge.json")).unwrap();
    let p = f
        .app
        .preview_merge_action(&f.id, &action("release"))
        .unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert!(p.paths.is_empty());
    let archive = p.recovery_dir.unwrap().to_path().unwrap();
    let done = f
        .app
        .apply_merge_action(&f.id, &action("release"), &p.token)
        .unwrap();
    assert_eq!(done.status, "succeeded", "{}", done.summary);
    assert_eq!(fs::read(archive).unwrap(), marker);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&f.root, &["show-ref"]), refs);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(f.root.join("file")).unwrap(),
        "private unstaged\n"
    );
    assert_eq!(f.app.merge_state(&f.id).unwrap().phase, "none");
}

fn unrelated_fixture(conflict: bool) -> F {
    let mut f = fixture(false);
    git(&f.root, &["checkout", "--orphan", "unrelated"]);
    git(&f.root, &["rm", "-rf", "."]);
    fs::write(
        f.root.join(if conflict { "file" } else { "separate" }),
        "independent\n",
    )
    .unwrap();
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-m", "independent target"]);
    f.target = git(&f.root, &["rev-parse", "HEAD"]);
    git(&f.root, &["branch", "-f", "main", &f.target]);
    git(&f.root, &["checkout", "feature"]);
    f
}
#[test]
fn root_replay_onto_an_unrelated_target_keeps_every_author_and_target_history() {
    let f = unrelated_fixture(false);
    let old = git(
        &f.root,
        &["log", "--reverse", "--format=%an %ae %at", "feature"],
    );
    let before = git(&f.root, &["status", "--porcelain"]);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert!(p.rebase.as_ref().unwrap().root);
    assert_eq!(p.rebase.as_ref().unwrap().commits.len(), 2);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.original);
    assert_eq!(git(&f.root, &["status", "--porcelain"]), before);
    let result = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(result.status, "succeeded", "{result:?}");
    assert_eq!(
        git(
            &f.root,
            &[
                "log",
                "--reverse",
                "--format=%an %ae %at",
                &format!("{}..HEAD", f.target)
            ]
        ),
        old
    );
    assert_eq!(git(&f.root, &["rev-parse", "HEAD~2"]), f.target);
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.original);
    assert_eq!(
        fs::read_to_string(f.root.join("separate")).unwrap(),
        "independent\n"
    );
}
#[test]
fn conflicted_root_replay_can_abort_after_application_restart() {
    let f = unrelated_fixture(true);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert!(!p.conflicts.is_empty());
    let result = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert!(
        ["pending", "conflicted"].contains(&result.status.as_str()),
        "{result:?}"
    );
    let app = Application::new(f._dir.path().join("catalog.json")).unwrap();
    let q = app.preview_merge_action(&f.id, &action("abort")).unwrap();
    assert!(q.blocked_reason.is_none(), "{:?}", q.blocked_reason);
    let result = app
        .apply_merge_action(&f.id, &action("abort"), &q.token)
        .unwrap();
    assert_eq!(result.status, "succeeded", "{result:?}");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.original);
    assert_eq!(git(&f.root, &["status", "--porcelain"]), "");
}
#[test]
fn shallow_boundary_is_never_mistaken_for_a_root_replay() {
    let f = unrelated_fixture(false);
    fs::write(f.root.join(".git/shallow"), format!("{}\n", f.original)).unwrap();
    let error = f.app.preview_merge(&f.id, &request(&f)).unwrap_err();
    assert!(error.message.contains("shallow"), "{error:?}");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.original);
}

#[test]
fn recreated_merge_conflict_requires_review_and_can_abort_without_flattening() {
    let f = fixture(false);
    let base = git(&f.root, &["merge-base", "main", "feature"]);
    git(&f.root, &["checkout", "-b", "side", &base]);
    fs::write(f.root.join("file"), "side edit\n").unwrap();
    git(&f.root, &["commit", "-am", "side edit"]);
    git(&f.root, &["checkout", "feature"]);
    // Build an original two-parent merge with an explicit resolution, without relying on merge's failure exit.
    let _ = Command::new("git")
        .arg("-C")
        .arg(&f.root)
        .args(["merge", "--no-ff", "side", "-m", "resolved merge"])
        .output()
        .unwrap();
    fs::write(f.root.join("file"), "manual original resolution\n").unwrap();
    git(&f.root, &["add", "file"]);
    git(&f.root, &["commit", "-m", "resolved merge"]);
    let original = git(&f.root, &["rev-parse", "HEAD"]);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert!(p.rebase.as_ref().unwrap().preserve_merges);
    assert!(!p.conflicts.is_empty());
    let r = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert!(
        ["pending", "conflicted"].contains(&r.status.as_str()),
        "{r:?}"
    );
    let app = Application::new(f._dir.path().join("catalog.json")).unwrap();
    let q = app.preview_merge_action(&f.id, &action("abort")).unwrap();
    let r = app
        .apply_merge_action(&f.id, &action("abort"), &q.token)
        .unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), original);
    assert_eq!(
        fs::read_to_string(f.root.join("file")).unwrap(),
        "manual original resolution\n"
    );
}

#[test]
fn recreated_merge_can_resolve_continue_and_foreign_label_changes_block_writes() {
    let f = fixture(false);
    let base = git(&f.root, &["merge-base", "main", "feature"]);
    git(&f.root, &["checkout", "-b", "side", &base]);
    fs::write(f.root.join("file"), "side edit\n").unwrap();
    git(&f.root, &["commit", "-am", "side edit"]);
    git(&f.root, &["checkout", "feature"]);
    let _ = Command::new("git")
        .arg("-C")
        .arg(&f.root)
        .args(["merge", "--no-ff", "side", "-m", "resolved merge"])
        .output()
        .unwrap();
    fs::write(f.root.join("file"), "original resolution\n").unwrap();
    git(&f.root, &["add", "file"]);
    git(&f.root, &["commit", "-m", "resolved merge"]);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let r = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert!(
        ["pending", "conflicted"].contains(&r.status.as_str()),
        "{r:?}"
    );
    let labels = git(
        &f.root,
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/rewritten/",
        ],
    );
    let fields = labels
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>();
    git(&f.root, &["update-ref", fields[0], &f.original]);
    assert_eq!(f.app.snapshot(&f.id).unwrap().merge.phase, "incomplete");
    assert!(f
        .app
        .preview_merge_action(&f.id, &action("finish"))
        .unwrap()
        .blocked_reason
        .is_some());
    git(&f.root, &["update-ref", fields[0], fields[1]]);
    git(&f.root, &["symbolic-ref", fields[0], "refs/heads/main"]);
    match f.app.preview_merge_action(&f.id, &action("finish")) {
        Err(_) => {}
        Ok(p) => {
            assert_ne!(p.state.phase, "pending");
            assert!(p.blocked_reason.is_some());
        }
    }
    git(&f.root, &["update-ref", "--no-deref", fields[0], fields[1]]);
    let state = f.app.snapshot(&f.id).unwrap().merge;
    assert_eq!(state.phase, "pending");
    let resolve = MergeAction {
        kind: "resolve".into(),
        path: Some(state.conflicts[0].path.clone()),
        choice: Some("edited".into()),
        content: Some("reviewed new resolution\n".into()),
        message: String::new(),
    };
    let q = f.app.preview_merge_action(&f.id, &resolve).unwrap();
    assert!(q.blocked_reason.is_none(), "{:?}", q.blocked_reason);
    f.app.apply_merge_action(&f.id, &resolve, &q.token).unwrap();
    let next = action("finish");
    let q = f.app.preview_merge_action(&f.id, &next).unwrap();
    assert!(q.blocked_reason.is_none(), "{:?}", q.blocked_reason);
    let r = f.app.apply_merge_action(&f.id, &next, &q.token).unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
    assert_eq!(
        git(&f.root, &["rev-list", "--parents", "-n", "1", "HEAD"])
            .split_whitespace()
            .count(),
        3
    );
    assert_eq!(
        fs::read_to_string(f.root.join("file")).unwrap(),
        "reviewed new resolution\n"
    );
}

#[test]
fn cousin_replay_retains_its_original_branch_point_and_other_refs() {
    let f = fixture(false);
    let base = git(&f.root, &["merge-base", "main", "feature"]);
    git(&f.root, &["checkout", "-b", "middle", &base]);
    fs::write(f.root.join("middle"), "middle\n").unwrap();
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-m", "middle"]);
    let middle = git(&f.root, &["rev-parse", "HEAD"]);
    git(&f.root, &["checkout", "main"]);
    git(
        &f.root,
        &["merge", "--no-ff", "middle", "-m", "target with middle"],
    );
    let target = git(&f.root, &["rev-parse", "HEAD"]);
    git(&f.root, &["checkout", "feature"]);
    git(
        &f.root,
        &["merge", "--no-ff", "middle", "-m", "merge cousin"],
    );
    let original = git(&f.root, &["rev-parse", "HEAD"]);
    let mut r = request(&f);
    r.source_oid = target.clone();
    let p = f.app.preview_merge(&f.id, &r).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    assert!(p.rebase.as_ref().unwrap().preserve_merges);
    let done = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(done.status, "succeeded", "{done:?}");
    assert_ne!(git(&f.root, &["rev-parse", "HEAD"]), original);
    assert_eq!(git(&f.root, &["rev-parse", "main"]), target);
    assert_eq!(git(&f.root, &["rev-parse", "middle"]), middle);
    assert_eq!(git(&f.root, &["status", "--porcelain"]), "");
}

#[test]
fn interactive_reorder_drop_and_fixup_preserve_reviewed_tree_author_and_other_refs() {
    let f = fixture(false);
    let first = f.original.clone();
    let mut added = Vec::new();
    for name in ["second", "third"] {
        fs::write(f.root.join(name), format!("{name}\n")).unwrap();
        git(&f.root, &["add", name]);
        git(&f.root, &["commit", "-m", name]);
        added.push(git(&f.root, &["rev-parse", "HEAD"]));
    }
    let original = git(&f.root, &["rev-parse", "HEAD"]);
    let mut r = request(&f);
    r.rebase_steps = vec![
        RebaseEdit {
            oid: added[1].clone(),
            action: "pick".into(),
        },
        RebaseEdit {
            oid: added[0].clone(),
            action: "fixup".into(),
        },
        RebaseEdit {
            oid: first.clone(),
            action: "drop".into(),
        },
    ];
    let p = f.app.preview_merge(&f.id, &r).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    assert_eq!(p.rebase.as_ref().unwrap().commits, vec![added[1].clone()]);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), original);
    let done = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(done.status, "succeeded", "{done:?}");
    assert_eq!(
        git(
            &f.root,
            &["rev-list", "--count", &format!("{}..HEAD", f.target)]
        ),
        "1"
    );
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%s", "HEAD"]),
        "third"
    );
    assert_eq!(git(&f.root, &["rev-parse", "main"]), f.target);
    assert_eq!(fs::read_to_string(f.root.join("file")).unwrap(), "base\n");
    assert!(f.root.join("second").exists() && f.root.join("third").exists());
    assert_eq!(git(&f.root, &["status", "--porcelain"]), "");
}
#[test]
fn interactive_plan_rejects_duplicates_missing_all_drop_first_fixup_and_exec() {
    let f = fixture(false);
    for steps in [
        vec![RebaseEdit {
            oid: f.original.clone(),
            action: "exec".into(),
        }],
        vec![RebaseEdit {
            oid: f.original.clone(),
            action: "fixup".into(),
        }],
        vec![RebaseEdit {
            oid: f.original.clone(),
            action: "drop".into(),
        }],
        vec![
            RebaseEdit {
                oid: f.original.clone(),
                action: "pick".into(),
            },
            RebaseEdit {
                oid: f.original.clone(),
                action: "pick".into(),
            },
        ],
    ] {
        let mut r = request(&f);
        r.rebase_steps = steps;
        assert!(f.app.preview_merge(&f.id, &r).is_err());
    }
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.original);
}

#[test]
fn clean_octopus_rebase_preserves_all_three_ordered_parents() {
    let f = fixture(false);
    let base = git(&f.root, &["merge-base", "main", "feature"]);
    for name in ["side-b", "side-c"] {
        git(&f.root, &["checkout", "-b", name, &base]);
        fs::write(f.root.join(name), format!("{name}\n")).unwrap();
        git(&f.root, &["add", name]);
        git(&f.root, &["commit", "-m", name]);
    }
    git(&f.root, &["checkout", "feature"]);
    git(
        &f.root,
        &[
            "merge",
            "--no-ff",
            "side-b",
            "side-c",
            "-m",
            "Octopus fixture",
        ],
    );
    let before_b = git(&f.root, &["rev-parse", "side-b"]);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    let done = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(done.status, "succeeded", "{done:?}");
    assert_eq!(
        git(&f.root, &["rev-list", "--parents", "-n", "1", "HEAD"])
            .split_whitespace()
            .count(),
        4
    );
    assert_eq!(git(&f.root, &["rev-parse", "main"]), f.target);
    assert_eq!(git(&f.root, &["rev-parse", "side-b"]), before_b);
    assert!(f.root.join("side-b").exists() && f.root.join("side-c").exists());
}

#[test]
fn interactive_fixup_onto_existing_base_handles_quoted_paths_and_original_author() {
    let mut f = fixture(false);
    let moved = f._dir.path().join("repo's work");
    fs::rename(&f.root, &moved).unwrap();
    f.root = moved;
    f.app = Application::new(f._dir.path().join("quoted-catalog.json")).unwrap();
    f.id = f.app.add_repository(&f.root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    fs::write(f.root.join("second"), "second\n").unwrap();
    git(&f.root, &["add", "second"]);
    git(&f.root, &["commit", "-m", "fixup payload"]);
    let second = git(&f.root, &["rev-parse", "HEAD"]);
    let mut r = request(&f);
    r.source_oid = git(&f.root, &["merge-base", "main", "feature"]);
    r.source_ref = None;
    r.rebase_steps = vec![
        RebaseEdit {
            oid: f.original.clone(),
            action: "pick".into(),
        },
        RebaseEdit {
            oid: second,
            action: "fixup".into(),
        },
    ];
    let p = f.app.preview_merge(&f.id, &r).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    let result = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert_eq!(result.status, "succeeded", "{result:?}");
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%an <%ae>", "HEAD"]),
        "Original Author <original@example.invalid>"
    );
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%s", "HEAD"]),
        "feature"
    );
    assert!(f.root.join("second").exists());
}
#[test]
fn interactive_conflict_abort_after_restart_restores_original_tip() {
    let f = fixture(true);
    fs::write(f.root.join("second"), "second\n").unwrap();
    git(&f.root, &["add", "second"]);
    git(&f.root, &["commit", "-m", "fixup"]);
    let original = git(&f.root, &["rev-parse", "HEAD"]);
    let mut r = request(&f);
    r.rebase_steps = vec![
        RebaseEdit {
            oid: f.original.clone(),
            action: "pick".into(),
        },
        RebaseEdit {
            oid: original.clone(),
            action: "fixup".into(),
        },
    ];
    let p = f.app.preview_merge(&f.id, &r).unwrap();
    assert!(!p.conflicts.is_empty());
    let start = f.app.begin_merge(&f.id, &p.request, &p.token).unwrap();
    assert!(
        ["pending", "conflicted"].contains(&start.status.as_str()),
        "{start:?}"
    );
    let a = Application::new(f._dir.path().join("catalog.json")).unwrap();
    let action = action("abort");
    let preview = a.preview_merge_action(&f.id, &action).unwrap();
    assert!(preview.blocked_reason.is_none(), "{preview:?}");
    let done = a
        .apply_merge_action(&f.id, &action, &preview.token)
        .unwrap();
    assert_eq!(done.status, "succeeded", "{done:?}");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), original);
    assert_eq!(git(&f.root, &["rev-parse", "main"]), f.target);
}
#[test]
fn octopus_replay_with_conflicting_target_is_blocked_before_writes() {
    let f = fixture(true);
    let base = git(&f.root, &["merge-base", "main", "feature"]);
    for name in ["b", "c"] {
        git(&f.root, &["checkout", "-b", name, &base]);
        fs::write(f.root.join(name), "independent\n").unwrap();
        git(&f.root, &["add", name]);
        git(&f.root, &["commit", "-m", name]);
    }
    git(&f.root, &["checkout", "feature"]);
    git(&f.root, &["merge", "--no-ff", "b", "c", "-m", "octopus"]);
    let before = git(&f.root, &["rev-parse", "HEAD"]);
    let p = f.app.preview_merge(&f.id, &request(&f)).unwrap();
    assert!(
        p.blocked_reason
            .as_deref()
            .is_some_and(|s| s.contains("Octopus replay")),
        "{p:?}"
    );
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), before);
}

#[test]
fn publication_evidence_includes_remote_refs_containing_dropped_and_kept_commits() {
    let f = fixture(false);
    git(
        &f.root,
        &["update-ref", "refs/remotes/origin/shared", &f.original],
    );
    fs::write(f.root.join("another"), "another\n").unwrap();
    git(&f.root, &["add", "another"]);
    git(&f.root, &["commit", "-m", "another"]);
    let tip = git(&f.root, &["rev-parse", "HEAD"]);
    git(&f.root, &["update-ref", "refs/remotes/other/new", &tip]);
    let mut r = request(&f);
    r.rebase_steps = vec![
        RebaseEdit {
            oid: f.original.clone(),
            action: "drop".into(),
        },
        RebaseEdit {
            oid: tip,
            action: "pick".into(),
        },
    ];
    let p = f.app.preview_merge(&f.id, &r).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    assert_eq!(
        p.rebase.unwrap().referenced_by,
        vec!["refs/remotes/origin/shared", "refs/remotes/other/new"]
    );
}
