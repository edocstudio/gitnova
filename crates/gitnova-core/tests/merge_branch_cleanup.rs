use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(root: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args(["--no-optional-locks", "-C"])
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim().into()
}
struct F {
    _dir: tempfile::TempDir,
    app: Application,
    root: PathBuf,
    id: String,
    source: String,
    original: String,
}
fn fixture() -> F {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    for (key, val) in [
        ("user.name", "Cleanup Test"),
        ("user.email", "test@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "/dev/null"),
    ] {
        git(&root, &["config", key, val]);
    }
    fs::write(root.join("base"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Base"]);
    let original = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["switch", "-c", "feature"]);
    fs::write(root.join("feature"), "work\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Feature"]);
    let source = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/feature", &source],
    );
    git(&root, &["switch", "main"]);
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
        source,
        original,
    }
}
fn request(f: &F, selected: bool, method: MergeMethod) -> MergeRequest {
    MergeRequest {
        acknowledge_rewrite: false,
        delete_source_branch: selected,
        method,
        operation: "merge".into(),
        mainline: None,
        source_oid: f.source.clone(),
        source_ref: Some("refs/heads/feature".into()),
        session_id: String::new(),
    }
}
fn begin(f: &F, selected: bool, method: MergeMethod) -> OperationReceipt {
    let r = request(f, selected, method);
    let p = f.app.preview_merge(&f.id, &r).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    f.app.begin_merge(&f.id, &p.request, &p.token).unwrap()
}
fn apply(f: &F, kind: &str) -> OperationReceipt {
    let a = MergeAction {
        kind: kind.into(),
        path: None,
        choice: None,
        content: None,
        message: if kind == "finish" {
            "Merge reviewed feature".into()
        } else {
            String::new()
        },
    };
    let p = f.app.preview_merge_action(&f.id, &a).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    f.app.apply_merge_action(&f.id, &a, &p.token).unwrap()
}
fn has_feature(f: &F) -> bool {
    git(
        &f.root,
        &["for-each-ref", "--format=%(refname)", "refs/heads/feature"],
    ) == "refs/heads/feature"
}
fn assert_deleted(f: &F, r: &OperationReceipt) {
    assert_eq!(r.status, "succeeded", "{}", r.summary);
    let c = r.branch_cleanup.as_ref().unwrap();
    assert_eq!(c.status, "deleted", "{}", c.message);
    assert!(!has_feature(f));
    assert_eq!(
        git(&f.root, &["rev-parse", c.recovery_ref.as_ref().unwrap()]),
        f.source
    );
    assert_eq!(
        git(&f.root, &["rev-parse", "refs/remotes/origin/feature"]),
        f.source
    );
    assert!(c.receipt_id.is_some());
    assert_eq!(git(&f.root, &["symbolic-ref", "--short", "HEAD"]), "main");
}
#[test]
fn selected_merge_deletes_only_after_finish_and_retains_source_tip_and_remote() {
    let f = fixture();
    let r = begin(&f, true, MergeMethod::MergeCommit);
    assert_eq!(r.status, "pending");
    assert!(r.branch_cleanup.is_none());
    assert!(has_feature(&f));
    let state = f.app.merge_state(&f.id).unwrap();
    assert!(state.delete_source_branch);
    assert_eq!(state.source_ref.as_deref(), Some("refs/heads/feature"));
    let reopened = Application::new(f._dir.path().join("state/catalog.json")).unwrap();
    assert!(reopened.merge_state(&f.id).unwrap().delete_source_branch);
    let r = apply(&f, "finish");
    assert_deleted(&f, &r);
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%P", "HEAD"]),
        format!("{} {}", f.original, f.source)
    );
}
#[test]
fn fast_forward_cleans_up_after_verified_tip_advance() {
    let f = fixture();
    let r = begin(&f, true, MergeMethod::FastForwardOnly);
    assert_deleted(&f, &r);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.source);
}
#[test]
fn unchecked_merge_and_aborted_selected_merge_keep_source_branch() {
    for selected in [false, true] {
        let f = fixture();
        begin(&f, selected, MergeMethod::MergeCommit);
        let r = apply(&f, if selected { "abort" } else { "finish" });
        assert_eq!(r.status, "succeeded");
        assert!(r.branch_cleanup.is_none());
        assert!(has_feature(&f));
    }
}
#[test]
fn source_advance_after_begin_keeps_new_work_and_reports_merge_success_separately() {
    let f = fixture();
    begin(&f, true, MergeMethod::MergeCommit);
    let newer = git(
        &f.root,
        &[
            "commit-tree",
            &format!("{}^{{tree}}", f.source),
            "-p",
            &f.source,
            "-m",
            "Unmerged new source work",
        ],
    );
    git(
        &f.root,
        &["update-ref", "refs/heads/feature", &newer, &f.source],
    );
    let r = apply(&f, "finish");
    assert_eq!(r.status, "succeeded");
    let c = r.branch_cleanup.unwrap();
    assert_eq!(c.status, "kept");
    assert!(has_feature(&f));
    assert_eq!(git(&f.root, &["rev-parse", "feature"]), newer);
    assert!(f.root.join("feature").exists());
}
#[test]
fn newly_checked_out_worktree_keeps_source_after_successful_merge() {
    let f = fixture();
    begin(&f, true, MergeMethod::MergeCommit);
    let path = f._dir.path().join("linked");
    git(
        &f.root,
        &["worktree", "add", path.to_str().unwrap(), "feature"],
    );
    let r = apply(&f, "finish");
    assert_eq!(r.status, "succeeded");
    assert_eq!(r.branch_cleanup.unwrap().status, "kept");
    assert!(has_feature(&f));
    assert_eq!(fs::read_to_string(path.join("feature")).unwrap(), "work\n");
}
#[test]
fn cleanup_choice_is_token_bound_and_remote_current_or_patch_sources_cannot_select_it() {
    let f = fixture();
    let p = f
        .app
        .preview_merge(&f.id, &request(&f, false, MergeMethod::MergeCommit))
        .unwrap();
    let mut r = p.request.clone();
    r.delete_source_branch = true;
    assert!(f.app.begin_merge(&f.id, &r, &p.token).is_err());
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.original);
    r.source_ref = Some("refs/remotes/origin/feature".into());
    assert!(f.app.preview_merge(&f.id, &r).is_err());
    r.source_ref = Some("refs/heads/feature".into());
    r.operation = "cherryPick".into();
    r.method = MergeMethod::Auto;
    assert!(f.app.preview_merge(&f.id, &r).is_err());
    r = request(&f, true, MergeMethod::MergeCommit);
    r.source_ref = Some("refs/heads/main".into());
    r.source_oid = f.original.clone();
    assert!(f.app.preview_merge(&f.id, &r).is_err());
    assert!(has_feature(&f));
}

#[cfg(unix)]
#[test]
fn rejected_cleanup_transaction_does_not_turn_merge_success_into_failure_or_retry_deletion() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let hooks = f._dir.path().join("hooks");
    fs::create_dir(&hooks).unwrap();
    let hook = hooks.join("reference-transaction");
    fs::write(&hook, "#!/bin/sh\n[ \"$1\" = prepared ] || exit 0\nwhile read old new ref; do\nif [ \"$ref\" = refs/heads/feature ] && [ \"$new\" = 0000000000000000000000000000000000000000 ]; then exit 1; fi\ndone\nexit 0\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        &f.root,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    begin(&f, true, MergeMethod::MergeCommit);
    let r = apply(&f, "finish");
    assert_eq!(r.status, "succeeded");
    assert_eq!(r.branch_cleanup.unwrap().status, "unknown");
    assert!(has_feature(&f));
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%P", "HEAD"]),
        format!("{} {}", f.original, f.source)
    );
}
