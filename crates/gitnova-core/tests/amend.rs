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
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
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
struct F {
    dir: tempfile::TempDir,
    root: PathBuf,
    app: Application,
    id: String,
    oid: String,
}
fn fixture() -> F {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo ' ไทย");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Original Author"]);
    git(&root, &["config", "user.email", "original@example.invalid"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    git(&root, &["config", "core.hooksPath", "/dev/null"]);
    fs::write(root.join("file.txt"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Original root"]);
    let oid = git(&root, &["rev-parse", "HEAD"]);
    let app = Application::new(dir.path().join("catalog.json")).unwrap();
    let id = app.add_repository(&root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    F {
        dir,
        root,
        app,
        id,
        oid,
    }
}
fn request(f: &F, message: &str) -> AmendRequest {
    AmendRequest {
        oid: f.oid.clone(),
        message: message.into(),
    }
}
#[test]
fn message_only_root_amend_preserves_tree_author_date_and_recovery_after_restart() {
    let f = fixture();
    let tree = git(&f.root, &["rev-parse", "HEAD^{tree}"]);
    let author = git(&f.root, &["show", "-s", "--format=%an <%ae> %ai", "HEAD"]);
    git(&f.root, &["config", "user.name", "Current Committer"]);
    git(
        &f.root,
        &["config", "user.email", "committer@example.invalid"],
    );
    let r = request(&f, "Correct root\n\nรายละเอียดภาษาไทย");
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    assert!(p.commit.files.is_empty());
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.oid);
    let out = f.app.amend_commit(&f.id, &r, &p.token, false).unwrap();
    assert_eq!(out.status, "succeeded", "{out:?}");
    assert_ne!(git(&f.root, &["rev-parse", "HEAD"]), f.oid);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD^{tree}"]), tree);
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%an <%ae> %ai", "HEAD"]),
        author
    );
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%cn", "HEAD"]),
        "Current Committer"
    );
    assert_eq!(git(&f.root, &["show", "-s", "--format=%P", "HEAD"]), "");
    assert_eq!(git(&f.root, &["log", "-1", "--format=%B"]), r.message);
    let raw = Command::new("git")
        .arg("-C")
        .arg(&f.root)
        .args(["cat-file", "commit", "HEAD"])
        .output()
        .unwrap()
        .stdout;
    let text = String::from_utf8(raw).unwrap();
    assert_eq!(text.split_once("\n\n").unwrap().1, r.message);

    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.oid);
    drop(f.app);
    let app = Application::new(f.dir.path().join("catalog.json")).unwrap();
    assert!(app
        .recorded_activity(&f.id)
        .unwrap()
        .entries
        .iter()
        .any(|e| e.recovery_ref.as_deref() == Some(&p.recovery_ref)));
}
#[test]
fn amended_content_uses_exact_index_and_preserves_unstaged_and_untracked_work() {
    let f = fixture();
    fs::write(f.root.join("file.txt"), "staged\n").unwrap();
    git(&f.root, &["add", "file.txt"]);
    fs::write(f.root.join("file.txt"), "unstaged\n").unwrap();
    fs::write(f.root.join("local.txt"), "keep\n").unwrap();
    let r = request(&f, "Amended staged content");
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    assert_eq!(p.commit.files.len(), 1);
    let out = f.app.amend_commit(&f.id, &r, &p.token, false).unwrap();
    assert_eq!(out.status, "succeeded", "{out:?}");
    assert_eq!(git(&f.root, &["show", "HEAD:file.txt"]), "staged");
    assert_eq!(git(&f.root, &["show", ":file.txt"]), "staged");
    assert_eq!(
        fs::read_to_string(f.root.join("file.txt")).unwrap(),
        "unstaged\n"
    );
    assert_eq!(
        fs::read_to_string(f.root.join("local.txt")).unwrap(),
        "keep\n"
    );
    assert_eq!(git(&f.root, &["diff", "--cached", "--name-only"]), "");
    assert_eq!(
        git(&f.root, &["show", &format!("{}:file.txt", p.recovery_ref)]),
        "base"
    );
}
#[test]
fn cached_remote_history_requires_acknowledgment_and_is_never_pushed() {
    let f = fixture();
    let remote = f.dir.path().join("remote.git");
    git(f.dir.path(), &["init", "--bare", remote.to_str().unwrap()]);
    git(
        &f.root,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&f.root, &["push", "-u", "origin", "main"]);
    let r = request(&f, "Rewrite only local tip");
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    assert!(p
        .referenced_by
        .iter()
        .any(|r| r == "refs/remotes/origin/main"));
    let blocked = f.app.amend_commit(&f.id, &r, &p.token, false).unwrap();
    assert_ne!(blocked.status, "succeeded");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.oid);
    let out = f.app.amend_commit(&f.id, &r, &p.token, true).unwrap();
    assert_eq!(out.status, "succeeded", "{out:?}");
    assert_eq!(git(&remote, &["rev-parse", "main"]), f.oid);
    assert_eq!(
        git(&f.root, &["rev-parse", "refs/remotes/origin/main"]),
        f.oid
    );
}
#[test]
fn merge_amend_preserves_every_parent_and_can_keep_a_message_only_change() {
    let mut f = fixture();
    git(&f.root, &["checkout", "-b", "side"]);
    fs::write(f.root.join("side.txt"), "side\n").unwrap();
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-m", "Side"]);
    git(&f.root, &["checkout", "main"]);
    fs::write(f.root.join("main.txt"), "main\n").unwrap();
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-m", "Main"]);
    git(&f.root, &["merge", "--no-ff", "--no-edit", "side"]);
    f.oid = git(&f.root, &["rev-parse", "HEAD"]);
    let parents = git(&f.root, &["show", "-s", "--format=%P", "HEAD"]);
    assert_eq!(parents.split_whitespace().count(), 2);
    let r = request(&f, "Explain the merge");
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    let out = f.app.amend_commit(&f.id, &r, &p.token, false).unwrap();
    assert_eq!(out.status, "succeeded", "{out:?}");
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%P", "HEAD"]),
        parents
    );
}
#[test]
fn changed_message_index_configuration_and_references_invalidate_the_review() {
    let f = fixture();
    let r = request(&f, "Reviewed text");
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    let changed = request(&f, "Different text");
    assert_eq!(
        f.app
            .amend_commit(&f.id, &changed, &p.token, false)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    git(&f.root, &["tag", "v1"]);
    assert_eq!(
        f.app
            .amend_commit(&f.id, &r, &p.token, true)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    git(&f.root, &["config", "commit.cleanup", "strip"]);
    assert_eq!(
        f.app
            .amend_commit(&f.id, &r, &p.token, true)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    fs::write(f.root.join("file.txt"), "new stage\n").unwrap();
    git(&f.root, &["add", "file.txt"]);
    assert_eq!(
        f.app
            .amend_commit(&f.id, &r, &p.token, true)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.oid);
}
#[cfg(unix)]
#[test]
fn hooks_are_not_run_in_preview_and_modified_message_is_reported_unknown() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let hooks = f.root.join(".git/test-hooks");
    fs::create_dir(&hooks).unwrap();
    let path = hooks.join("commit-msg");
    fs::write(&path, "#!/bin/sh\nprintf '\\nHook addition\\n' >> \"$1\"\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        &f.root,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    let r = request(&f, "Reviewed message");
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.oid);
    let out = f.app.amend_commit(&f.id, &r, &p.token, false).unwrap();
    assert_eq!(out.status, "unknown", "{out:?}");
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.oid);
    assert!(git(&f.root, &["log", "-1", "--format=%B"]).contains("Hook addition"));
}

#[test]
fn empty_detached_native_operation_and_split_index_contexts_refuse_amend() {
    let f = fixture();
    let info = f.app.amend_original(&f.id, &f.oid).unwrap();
    let p = f
        .app
        .preview_amend(&f.id, &request(&f, &info.message))
        .unwrap();
    assert!(p.blocked_reason.as_deref().unwrap().contains("No staged"));
    git(&f.root, &["checkout", "--detach"]);
    assert_eq!(
        f.app
            .preview_amend(&f.id, &request(&f, "Changed"))
            .unwrap_err()
            .code,
        "amend_blocked"
    );
    git(&f.root, &["checkout", "main"]);
    fs::write(f.root.join(".git/MERGE_HEAD"), format!("{}\n", f.oid)).unwrap();
    assert_eq!(
        f.app
            .preview_amend(&f.id, &request(&f, "Changed"))
            .unwrap_err()
            .code,
        "operation_in_progress"
    );
    fs::remove_file(f.root.join(".git/MERGE_HEAD")).unwrap();
    git(&f.root, &["update-index", "--split-index"]);
    assert!(f
        .app
        .preview_amend(&f.id, &request(&f, "Changed"))
        .unwrap_err()
        .message
        .contains("Split indexes"));
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.oid);
}
#[test]
fn linked_worktree_amend_keeps_the_main_checkout_and_intent_to_add_file() {
    let f = fixture();
    let linked = f.dir.path().join("linked");
    git(
        &f.root,
        &["worktree", "add", "-b", "feature", linked.to_str().unwrap()],
    );
    fs::write(linked.join("intent.txt"), "not staged\n").unwrap();
    git(&linked, &["add", "--intent-to-add", "intent.txt"]);
    let id = f.app.add_repository(&linked).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    let request = AmendRequest {
        oid: f.oid.clone(),
        message: "Linked amendment".into(),
    };
    let p = f.app.preview_amend(&id, &request).unwrap();
    assert!(p.commit.files.is_empty());
    let out = f.app.amend_commit(&id, &request, &p.token, false).unwrap();
    assert_eq!(out.status, "succeeded", "{out:?}");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.oid);
    assert_ne!(git(&linked, &["rev-parse", "HEAD"]), f.oid);
    assert_eq!(git(&linked, &["ls-tree", "HEAD", "intent.txt"]), "");
    assert_eq!(
        fs::read_to_string(linked.join("intent.txt")).unwrap(),
        "not staged\n"
    );
    assert!(git(&linked, &["status", "--porcelain"]).contains("intent.txt"));
}
#[test]
fn symbolic_recovery_ref_cannot_redirect_amend_recovery() {
    let f = fixture();
    git(
        &f.root,
        &[
            "symbolic-ref",
            &format!("refs/gitnova/recovery/amend-{}", f.oid),
            "refs/heads/main",
        ],
    );
    let error = f
        .app
        .preview_amend(&f.id, &request(&f, "Changed"))
        .unwrap_err();
    assert_eq!(error.code, "amend_blocked");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.oid);
}

#[cfg(unix)]
#[test]
fn a_post_commit_hook_that_changes_staging_is_reported_unknown_without_resetting_it() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let hooks = f.root.join(".git/post-hooks");
    fs::create_dir(&hooks).unwrap();
    let path = hooks.join("post-commit");
    fs::write(
        &path,
        "#!/bin/sh\nprintf 'Hook staged work\\n' > hook.txt\ngit add -- hook.txt\n",
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        &f.root,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    let r = request(&f, "Post-hook amend");
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    let out = f.app.amend_commit(&f.id, &r, &p.token, false).unwrap();
    assert_eq!(out.status, "unknown", "{out:?}");
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.oid);
    assert!(git(&f.root, &["diff", "--cached", "--name-only"]).contains("hook.txt"));
    assert_eq!(
        fs::read_to_string(f.root.join("hook.txt")).unwrap(),
        "Hook staged work\n"
    );
}

#[test]
fn message_only_empty_root_and_empty_linear_commits_are_retained_not_dropped() {
    for root in [true, false] {
        let mut f = fixture();
        if root {
            git(&f.root, &["rm", "file.txt"]);
            git(
                &f.root,
                &["commit", "--amend", "--allow-empty", "-m", "Empty root"],
            );
        } else {
            git(&f.root, &["commit", "--allow-empty", "-m", "Empty linear"]);
        }
        f.oid = git(&f.root, &["rev-parse", "HEAD"]);
        let parents = git(&f.root, &["show", "-s", "--format=%P", "HEAD"]);
        let tree = git(&f.root, &["rev-parse", "HEAD^{tree}"]);
        let r = request(&f, "Clarify intentional empty commit");
        let p = f.app.preview_amend(&f.id, &r).unwrap();
        assert!(p.empty_replacement);
        assert!(p.commit.files.is_empty());
        let out = f.app.amend_commit(&f.id, &r, &p.token, false).unwrap();
        assert_eq!(out.status, "succeeded", "{out:?}");
        assert_eq!(
            git(&f.root, &["show", "-s", "--format=%P", "HEAD"]),
            parents
        );
        assert_eq!(git(&f.root, &["rev-parse", "HEAD^{tree}"]), tree);
        assert_ne!(git(&f.root, &["rev-parse", "HEAD"]), f.oid);
        assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.oid);
    }
}

fn stage_large_amend(f: &F) {
    for i in 0..107 {
        let content = format!("file {i:03}\n{}\n", "reviewable content\n".repeat(600));
        fs::write(f.root.join(format!("change-{i:03}.txt")), content).unwrap();
    }
    git(&f.root, &["add", "."]);
}

#[test]
fn large_amend_uses_complete_manifest_and_per_file_review_preserving_unstaged_work() {
    let f = fixture();
    stage_large_amend(&f);
    let tree = git(&f.root, &["write-tree"]);
    let author = git(&f.root, &["show", "-s", "--format=%an <%ae> %ai", "HEAD"]);
    fs::write(f.root.join("file.txt"), "private unstaged work\n").unwrap();
    let r = request(&f, "Complete large amendment");
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    assert!(p.commit.truncated);
    assert!(p.commit.complete_manifest);
    assert_eq!(p.commit.files.len(), 107);
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let last = p.commit.files.last().unwrap();
    assert_eq!(last.path.display, "change-106.txt");
    assert!(!p.commit.diff.contains("file 106"));
    let patch = f
        .app
        .commit_file_preview(&f.id, &last.path, &p.commit.token)
        .unwrap();
    assert!(!patch.truncated);
    assert!(patch.diff.contains("file 106"));
    let out = f.app.amend_commit(&f.id, &r, &p.token, false).unwrap();
    assert_eq!(out.status, "succeeded", "{out:?}");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD^{tree}"]), tree);
    assert_eq!(
        git(&f.root, &["ls-tree", "-r", "--name-only", "HEAD"])
            .lines()
            .count(),
        108
    );
    assert_eq!(git(&f.root, &["show", "-s", "--format=%P", "HEAD"]), "");
    assert_eq!(
        git(&f.root, &["show", "-s", "--format=%an <%ae> %ai", "HEAD"]),
        author
    );
    assert_eq!(git(&f.root, &["rev-parse", &p.recovery_ref]), f.oid);
    assert_eq!(
        fs::read_to_string(f.root.join("file.txt")).unwrap(),
        "private unstaged work\n"
    );
}

#[test]
fn large_amend_rejects_stale_index_in_file_review_and_confirmation() {
    let f = fixture();
    stage_large_amend(&f);
    let r = request(&f, "Large amendment");
    let p = f.app.preview_amend(&f.id, &r).unwrap();
    assert!(p.commit.truncated && p.commit.complete_manifest);
    fs::write(f.root.join("change-106.txt"), "new staged content\n").unwrap();
    git(&f.root, &["add", "change-106.txt"]);
    let tree = git(&f.root, &["write-tree"]);
    assert!(f
        .app
        .commit_file_preview(&f.id, &p.commit.files.last().unwrap().path, &p.commit.token)
        .is_err());
    assert!(f.app.amend_commit(&f.id, &r, &p.token, false).is_err());
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.oid);
    assert_eq!(git(&f.root, &["write-tree"]), tree);
}
