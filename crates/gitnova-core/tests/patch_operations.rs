use gitnova_core::Application;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim_end().into()
}
fn setup() -> (tempfile::TempDir, Application, PathBuf, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "GitNova Test"),
        ("user.email", "test@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", ".git/hooks"),
        ("core.attributesFile", ".git/no-global-attributes"),
    ] {
        git(&repo, &["config", key, value]);
    }
    fs::write(repo.join("file.txt"), "main\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Main"]);
    git(&repo, &["switch", "-c", "feature"]);
    fs::write(repo.join("file.txt"), "feature\n").unwrap();
    git(&repo, &["commit", "-am", "Feature"]);
    git(&repo, &["switch", "main"]);
    let app = Application::new(directory.path().join("app/state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    (directory, app, repo, id)
}

use gitnova_domain::*;
fn request(oid: &str) -> MergeRequest {
    MergeRequest {
        rebase_steps: vec![],
        acknowledge_rewrite: false,
        delete_source_branch: false,
        method: MergeMethod::Auto,
        operation: "merge".into(),
        mainline: None,
        source_oid: oid.into(),
        source_ref: Some("refs/heads/feature".into()),
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
fn resolution(choice: &str, text: Option<&str>) -> MergeAction {
    MergeAction {
        kind: "resolve".into(),
        path: Some(NativePath::new(Path::new("file.txt"))),
        choice: Some(choice.into()),
        content: text.map(str::to_owned),
        message: String::new(),
    }
}
fn diverge(repo: &Path, conflict: bool) {
    if conflict {
        fs::write(repo.join("file.txt"), "current\n").unwrap();
    } else {
        fs::write(repo.join("destination.txt"), "destination\n").unwrap();
    }
    fs::write(repo.join("notes.txt"), "original notes\n").unwrap();
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", "Destination work"]);
}
fn object_files(path: &Path) -> Vec<PathBuf> {
    fn walk(path: &Path, result: &mut Vec<PathBuf>) {
        for item in fs::read_dir(path).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                walk(&path, result);
            } else {
                result.push(path);
            }
        }
    }
    let mut files = vec![];
    walk(path, &mut files);
    files.sort();
    files
}
fn preview(
    app: &Application,
    id: &str,
    oid: &str,
    operation: &str,
    mainline: Option<u32>,
) -> MergeStartPreview {
    let mut r = request(oid);
    r.operation = operation.into();
    r.mainline = mainline;
    r.source_ref = None;
    app.preview_merge(id, &r).unwrap()
}
fn start(app: &Application, id: &str, p: &MergeStartPreview) -> OperationReceipt {
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let receipt = app.begin_merge(id, &p.request, &p.token).unwrap();
    assert!(
        matches!(receipt.status.as_str(), "pending" | "conflicted"),
        "{} {:?}",
        receipt.summary,
        receipt.merge
    );
    receipt
}
fn finish(app: &Application, id: &str) -> OperationReceipt {
    let mut a = action("finish");
    a.message = app.merge_state(id).unwrap().suggested_message.unwrap();
    let p = app.preview_merge_action(id, &a).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    app.apply_merge_action(id, &a, &p.token).unwrap()
}
#[test]
fn cherry_pick_is_readonly_until_start_then_preserves_author_date_and_one_parent() {
    let (_temp, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    git(
        &repo,
        &[
            "commit",
            "--amend",
            "--no-edit",
            "--author=Original Writer <original@example.invalid>",
            "--date=2020-01-02T03:04:05Z",
        ],
    );
    let source = git(&repo, &["rev-parse", "HEAD"]);
    let author = git(&repo, &["show", "-s", "--format=%an|%ae|%aI", "HEAD"]);
    git(&repo, &["switch", "main"]);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    let objects = object_files(&repo.join(".git/objects"));
    let p = preview(&app, &id, &source, "cherryPick", None);
    assert_eq!(object_files(&repo.join(".git/objects")), objects);
    assert_eq!(p.mode, "cherryPick");
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    let r = start(&app, &id, &p);
    assert_eq!(r.status, "pending");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert!(app.commit_preview(&id).is_err());
    let r = finish(&app, &id);
    assert_eq!(r.status, "succeeded", "{}", r.summary);
    assert_eq!(git(&repo, &["show", "-s", "--format=%P", "HEAD"]), original);
    assert_eq!(
        git(&repo, &["show", "-s", "--format=%an|%ae|%aI", "HEAD"]),
        author
    );
    assert_eq!(
        git(&repo, &["rev-parse", "HEAD^{tree}"]),
        p.result_tree.unwrap()
    );
    assert_eq!(app.merge_state(&id).unwrap().phase, "none");
}
#[test]
fn revert_creates_an_inverse_commit_without_rewriting_source_history() {
    let (_temp, app, repo, id) = setup();
    git(&repo, &["merge", "--ff-only", "feature"]);
    let source = git(&repo, &["rev-parse", "HEAD"]);
    let p = preview(&app, &id, &source, "revert", None);
    assert!(p.suggested_message.starts_with("Revert \"Feature\"\n"));
    start(&app, &id, &p);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), source);
    let r = finish(&app, &id);
    assert_eq!(r.status, "succeeded", "{}", r.summary);
    assert_eq!(git(&repo, &["show", "-s", "--format=%P", "HEAD"]), source);
    assert_eq!(git(&repo, &["rev-parse", "feature"]), source);
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "main\n");
}
#[test]
fn conflicted_cherry_pick_uses_shared_resolution_then_single_parent_finish() {
    let (_temp, app, repo, id) = setup();
    diverge(&repo, true);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let source = git(&repo, &["rev-parse", "feature"]);
    let p = preview(&app, &id, &source, "cherryPick", None);
    assert_eq!(p.conflicts.len(), 1);
    assert_eq!(start(&app, &id, &p).status, "conflicted");
    let a = resolution("theirs", None);
    let p = app.preview_merge_action(&id, &a).unwrap();
    let r = app.apply_merge_action(&id, &a, &p.token).unwrap();
    assert_eq!(r.status, "pending", "{}", r.summary);
    assert_eq!(finish(&app, &id).status, "succeeded");
    assert_eq!(git(&repo, &["show", "-s", "--format=%P", "HEAD"]), original);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "feature\n"
    );
}
#[test]
fn clean_and_conflicted_abort_preserve_manual_edits_in_private_archive() {
    for conflict in [false, true] {
        let (_temp, app, repo, id) = setup();
        diverge(&repo, conflict);
        let original = git(&repo, &["rev-parse", "HEAD"]);
        let source = git(&repo, &["rev-parse", "feature"]);
        start(&app, &id, &preview(&app, &id, &source, "cherryPick", None));
        fs::write(repo.join("file.txt"), "manual work\n").unwrap();
        let a = action("abort");
        let p = app.preview_merge_action(&id, &a).unwrap();
        let r = app.apply_merge_action(&id, &a, &p.token).unwrap();
        if conflict {
            assert_eq!(r.status, "succeeded", "{} {:?}", r.summary, r.recovery);
            assert_eq!(app.merge_state(&id).unwrap().phase, "none");
        } else {
            assert_eq!(r.status, "unknown");
            assert_eq!(
                fs::read_to_string(repo.join("file.txt")).unwrap(),
                "manual work\n"
            );
            assert_ne!(app.merge_state(&id).unwrap().phase, "none");
            assert!(r.recovery.is_some());
        }
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
        let dir = p.recovery_dir.unwrap().to_path().unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
        assert!(manifest["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["file"].as_str())
            .any(|f| fs::read(dir.join(f)).unwrap() == b"manual work\n"));
    }
}
#[test]
fn revert_conflict_can_select_inverse_side_and_abort_after_native_marker() {
    let (_temp, app, repo, id) = setup();
    git(&repo, &["merge", "--ff-only", "feature"]);
    let source = git(&repo, &["rev-parse", "HEAD"]);
    diverge(&repo, true);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let p = preview(&app, &id, &source, "revert", None);
    assert_eq!(start(&app, &id, &p).status, "conflicted");
    assert!(repo.join(".git/REVERT_HEAD").exists());
    let a = resolution("theirs", None);
    let p = app.preview_merge_action(&id, &a).unwrap();
    assert_eq!(
        app.apply_merge_action(&id, &a, &p.token).unwrap().status,
        "pending"
    );
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "main\n");
    let a = action("abort");
    let p = app.preview_merge_action(&id, &a).unwrap();
    assert_eq!(
        app.apply_merge_action(&id, &a, &p.token).unwrap().status,
        "succeeded"
    );
    assert!(!repo.join(".git/REVERT_HEAD").exists());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
}
#[test]
fn merge_commit_requires_explicit_mainline_and_predicts_each_parent_delta() {
    let (_temp, app, repo, id) = setup();
    diverge(&repo, false);
    git(
        &repo,
        &["merge", "--no-ff", "feature", "-m", "Source merge"],
    );
    let source = git(&repo, &["rev-parse", "HEAD"]);
    let p = preview(&app, &id, &source, "revert", None);
    assert_eq!(p.parents.len(), 2);
    assert!(p.blocked_reason.is_some());
    let first = preview(&app, &id, &source, "revert", Some(1));
    let second = preview(&app, &id, &source, "revert", Some(2));
    assert_ne!(first.result_tree, second.result_tree);
    assert_ne!(first.token, second.token);
    assert_eq!(
        first.result_tree.as_ref(),
        Some(&git(&repo, &["rev-parse", "HEAD^1^{tree}"]))
    );
    assert_eq!(
        second.result_tree.as_ref(),
        Some(&git(&repo, &["rev-parse", "HEAD^2^{tree}"]))
    );
    start(&app, &id, &first);
    assert_eq!(finish(&app, &id).status, "succeeded");
    git(
        &repo,
        &["switch", "-c", "pick-target", &format!("{source}^1")],
    );
    let p = preview(&app, &id, &source, "cherryPick", Some(1));
    start(&app, &id, &p);
    assert_eq!(finish(&app, &id).status, "succeeded");
    assert_eq!(
        git(&repo, &["rev-parse", "HEAD^{tree}"]),
        git(&repo, &["rev-parse", &format!("{source}^{{tree}}")])
    );
}
#[test]
fn root_revert_and_empty_pick_are_explicit() {
    let (_temp, app, repo, id) = setup();
    let source = git(&repo, &["rev-parse", "HEAD"]);
    assert!(preview(&app, &id, &source, "cherryPick", None)
        .blocked_reason
        .is_some());
    let p = preview(&app, &id, &source, "revert", None);
    assert!(p.parents.is_empty());
    start(&app, &id, &p);
    assert_eq!(finish(&app, &id).status, "succeeded");
    assert!(!repo.join("file.txt").exists());
}
#[test]
fn ignored_collision_and_stale_or_invalid_mainline_never_start() {
    let (_temp, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    fs::write(repo.join("ignored.txt"), "incoming\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Ignored collision"]);
    let source = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["switch", "main"]);
    fs::write(repo.join(".git/info/exclude"), "ignored.txt\n").unwrap();
    fs::write(repo.join("ignored.txt"), "local secret\n").unwrap();
    let p = preview(&app, &id, &source, "cherryPick", None);
    assert!(p.blocked_reason.is_some());
    assert!(app
        .begin_merge(&id, &p.request, &p.token)
        .is_ok_and(|r| r.status != "succeeded"));
    assert_eq!(
        fs::read_to_string(repo.join("ignored.txt")).unwrap(),
        "local secret\n"
    );
    let mut req = request(&source);
    req.operation = "revert".into();
    req.mainline = Some(99);
    assert!(app.preview_merge(&id, &req).is_err());
}

#[test]
fn cherry_pick_applies_only_selected_delta_not_ancestor_changes() {
    let (_temp, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    fs::write(repo.join("selected.txt"), "selected change\n").unwrap();
    git(&repo, &["add", "selected.txt"]);
    git(&repo, &["commit", "-m", "Selected delta"]);
    let source = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["switch", "main"]);
    let p = preview(&app, &id, &source, "cherryPick", None);
    assert_eq!(p.files.len(), 1);
    assert_eq!(p.files[0].path.display, "selected.txt");
    start(&app, &id, &p);
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "main\n");
    assert_eq!(finish(&app, &id).status, "succeeded");
}
#[test]
fn resolved_empty_result_is_blocked_and_owned_state_survives_reopen() {
    let (temp, app, repo, id) = setup();
    diverge(&repo, true);
    let source = git(&repo, &["rev-parse", "feature"]);
    start(&app, &id, &preview(&app, &id, &source, "cherryPick", None));
    let reopened = Application::new(temp.path().join("app/state.json")).unwrap();
    assert_eq!(reopened.merge_state(&id).unwrap().operation, "cherryPick");
    let a = resolution("ours", None);
    let p = reopened.preview_merge_action(&id, &a).unwrap();
    assert_eq!(
        reopened
            .apply_merge_action(&id, &a, &p.token)
            .unwrap()
            .status,
        "pending"
    );
    let mut a = action("finish");
    a.message = "Empty should not commit".into();
    assert!(reopened
        .preview_merge_action(&id, &a)
        .unwrap()
        .blocked_reason
        .is_some());
    let a = action("abort");
    let p = reopened.preview_merge_action(&id, &a).unwrap();
    assert_eq!(
        reopened
            .apply_merge_action(&id, &a, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
}
#[test]
fn native_revert_marker_tampering_blocks_ownership_and_source_changes_stale_preview() {
    let (_temp, app, repo, id) = setup();
    let source = git(&repo, &["rev-parse", "feature"]);
    let p = preview(&app, &id, &source, "cherryPick", None);
    fs::write(repo.join("untracked.txt"), "work\n").unwrap();
    assert!(app.begin_merge(&id, &p.request, &p.token).is_err());
    fs::remove_file(repo.join("untracked.txt")).unwrap();
    git(&repo, &["merge", "--ff-only", "feature"]);
    diverge(&repo, true);
    start(&app, &id, &preview(&app, &id, &source, "revert", None));
    let old = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(repo.join(".git/REVERT_HEAD"), format!("{old}\n")).unwrap();
    assert_eq!(app.merge_state(&id).unwrap().phase, "incomplete");
    assert!(app.preview_merge_action(&id, &action("abort")).is_err());
}

#[test]
fn oversized_source_message_is_rejected_before_any_owned_state_is_written() {
    let (_temp, app, repo, id) = setup();
    git(&repo, &["switch", "feature"]);
    let message = repo.parent().unwrap().join("long-message");
    fs::write(&message, "x".repeat(17000)).unwrap();
    git(
        &repo,
        &["commit", "--amend", "-F", message.to_str().unwrap()],
    );
    let source = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["switch", "main"]);
    let mut r = request(&source);
    r.operation = "cherryPick".into();
    assert!(app.preview_merge(&id, &r).is_err());
    assert!(!repo.join(".git/gitnova-merge.json").exists());
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "main\n");
}

#[test]
fn shallow_boundaries_do_not_turn_a_selected_delta_into_an_inferred_root_change() {
    let (_temp, app, repo, id) = setup();
    let source = git(&repo, &["rev-parse", "feature"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    fs::write(repo.join(".git/shallow"), format!("{source}\n")).unwrap();
    assert!(git(&repo, &["show", "-s", "--format=%P", &source]).is_empty());
    for operation in ["cherryPick", "revert"] {
        let p = preview(&app, &id, &source, operation, None);
        assert!(p.blocked_reason.unwrap().contains("Shallow"));
        assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
        assert!(!repo.join(".git/gitnova-merge.json").exists());
    }
    fs::remove_file(repo.join(".git/shallow")).unwrap();
    assert!(preview(&app, &id, &source, "cherryPick", None)
        .blocked_reason
        .is_none());
}
