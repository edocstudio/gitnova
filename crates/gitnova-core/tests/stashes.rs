use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(path: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "--no-optional-locks",
            "-c",
            "user.name=Stash Test",
            "-c",
            "user.email=stash@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
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
    String::from_utf8_lossy(&o.stdout).trim().into()
}
fn setup() -> (tempfile::TempDir, Application, PathBuf, String) {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    fs::write(root.join("file.txt"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Base"]);
    let app = Application::new(t.path().join("app/state.json")).unwrap();
    let c = app.add_repository(&root).unwrap();
    let id = c.workspaces[0].selected_id.clone().unwrap();
    (t, app, root, id)
}
fn request(kind: StashKind) -> StashRequest {
    StashRequest {
        kind,
        index: None,
        oid: None,
        message: "Saved ภาษาไทย".into(),
        include_untracked: false,
        restore_index: true,
        id: String::new(),
    }
}
fn review(app: &Application, id: &str, r: StashRequest) -> StashPreview {
    app.preview_stash(id, &r).unwrap()
}
fn apply(app: &Application, id: &str, p: &StashPreview) -> OperationReceipt {
    let r = app.apply_stash(id, &p.request, &p.token).unwrap();
    assert_eq!(
        r.status, "succeeded",
        "{:?}: {} {:?}",
        p.request.kind, r.summary, r.recovery
    );
    r
}
fn selected(app: &Application, id: &str, index: usize, kind: StashKind) -> StashRequest {
    let e = app.stashes(id).unwrap().entries[index].clone();
    StashRequest {
        index: Some(index),
        oid: Some(e.oid),
        ..request(kind)
    }
}
#[test]
fn push_and_apply_restore_both_index_and_working_versions_without_moving_head() {
    let (_t, app, root, id) = setup();
    let head = git(&root, &["rev-parse", "HEAD"]);
    fs::write(root.join("file.txt"), "staged\n").unwrap();
    git(&root, &["add", "file.txt"]);
    fs::write(root.join("file.txt"), "working\n").unwrap();
    fs::write(root.join("new.txt"), "untracked\n").unwrap();
    let p = review(&app, &id, request(StashKind::Push));
    assert!(p.blocked_reason.is_none());
    assert!(p.diff.contains("+working"));
    assert!(p.index_diff.contains("+staged"));
    assert_eq!(p.files.len(), 1);
    assert!(app.stashes(&id).unwrap().entries.is_empty());
    apply(&app, &id, &p);
    assert_eq!(fs::read_to_string(root.join("file.txt")).unwrap(), "base\n");
    assert_eq!(
        fs::read_to_string(root.join("new.txt")).unwrap(),
        "untracked\n"
    );
    let e = app.stashes(&id).unwrap().entries[0].clone();
    assert_eq!(git(&root, &["rev-parse", &p.recovery_ref]), e.oid);
    fs::remove_file(root.join("new.txt")).unwrap();
    let p = review(&app, &id, selected(&app, &id, 0, StashKind::Apply));
    apply(&app, &id, &p);
    assert_eq!(git(&root, &["show", ":file.txt"]), "staged");
    assert_eq!(
        fs::read_to_string(root.join("file.txt")).unwrap(),
        "working\n"
    );
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    assert_eq!(app.stashes(&id).unwrap().entries.len(), 1);
}
#[test]
fn include_untracked_is_explicit_and_pop_removes_only_the_selected_saved_entry() {
    let (_t, app, root, id) = setup();
    for n in [1, 2] {
        fs::write(root.join("file.txt"), format!("version {n}\n")).unwrap();
        if n == 1 {
            fs::write(root.join("space ภาษาไทย.txt"), "new saved file\n").unwrap();
        }
        let mut r = request(StashKind::Push);
        r.include_untracked = true;
        let p = review(&app, &id, r);
        apply(&app, &id, &p);
    }
    assert_eq!(
        git(&root, &["status", "--porcelain"]),
        "",
        "Stash pushes must leave the selected tracked/untracked scope clean"
    );
    let list = app.stashes(&id).unwrap();
    assert_eq!(list.entries.len(), 2);
    let newer = list.entries[0].oid.clone();
    let older = list.entries[1].oid.clone();
    let p = review(&app, &id, selected(&app, &id, 1, StashKind::Pop));
    assert!(p.diff.contains("new saved file"));
    apply(&app, &id, &p);
    assert_eq!(
        fs::read_to_string(root.join("file.txt")).unwrap(),
        "version 1\n"
    );
    assert!(root.join("space ภาษาไทย.txt").exists());
    assert_eq!(git(&root, &["rev-parse", &p.recovery_ref]), older);
    let l = app.stashes(&id).unwrap();
    assert_eq!(l.entries.len(), 1);
    assert_eq!(l.entries[0].oid, newer);
    assert!(p
        .recovery_dir
        .to_path()
        .unwrap()
        .join("original-reflog")
        .exists());
    let p = review(&app, &id, selected(&app, &id, 0, StashKind::Drop));
    apply(&app, &id, &p);
    assert!(app.stashes(&id).unwrap().entries.is_empty());
    assert_eq!(git(&root, &["rev-parse", &p.recovery_ref]), newer);
    assert_eq!(
        fs::read_to_string(root.join("file.txt")).unwrap(),
        "version 1\n"
    );
}
#[test]
fn stale_positions_and_changed_working_content_never_modify_the_reviewed_stash() {
    let (_t, app, root, id) = setup();
    fs::write(root.join("file.txt"), "one\n").unwrap();
    let p = review(&app, &id, request(StashKind::Push));
    let modified = fs::metadata(root.join("file.txt"))
        .unwrap()
        .modified()
        .unwrap();
    fs::write(root.join("file.txt"), "two\n").unwrap();
    fs::File::options()
        .write(true)
        .open(root.join("file.txt"))
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    let receipt = app.apply_stash(&id, &p.request, &p.token).unwrap();
    assert_eq!(receipt.status, "failed");
    assert!(app.stashes(&id).unwrap().entries.is_empty());
    let p = review(&app, &id, request(StashKind::Push));
    apply(&app, &id, &p);
    let p = review(&app, &id, selected(&app, &id, 0, StashKind::Drop));
    fs::write(root.join("file.txt"), "external\n").unwrap();
    git(&root, &["stash", "push", "-m", "External"]);
    let before = git(&root, &["stash", "list"]);
    assert!(app.apply_stash(&id, &p.request, &p.token).is_err());
    assert_eq!(git(&root, &["stash", "list"]), before);
}
#[test]
fn conflicted_pop_retains_the_stash_and_recovery_without_force_reset() {
    let (_t, app, root, id) = setup();
    fs::write(root.join("file.txt"), "stashed\n").unwrap();
    let p = review(&app, &id, request(StashKind::Push));
    apply(&app, &id, &p);
    let oid = app.stashes(&id).unwrap().entries[0].oid.clone();
    fs::write(root.join("file.txt"), "destination\n").unwrap();
    git(&root, &["commit", "-am", "Destination"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    let mut r = selected(&app, &id, 0, StashKind::Pop);
    r.restore_index = false;
    let p = review(&app, &id, r);
    let result = app.apply_stash(&id, &p.request, &p.token).unwrap();
    assert_eq!(result.status, "conflicted", "{}", result.summary);
    assert_eq!(app.stashes(&id).unwrap().entries[0].oid, oid);
    assert_eq!(git(&root, &["rev-parse", &p.recovery_ref]), oid);
    assert!(!git(&root, &["ls-files", "--unmerged"]).is_empty());
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    assert!(result
        .recovery
        .unwrap()
        .contains("Resolve stash conflicts with Git"));
}
#[test]
fn metadata_native_locks_are_preserved_and_saved_entries_do_not_pollute_history() {
    let (_t, app, root, id) = setup();
    fs::write(root.join("file.txt"), "saved\n").unwrap();
    let p = review(&app, &id, request(StashKind::Push));
    apply(&app, &id, &p);
    let page = app.graph(&id, None).unwrap();
    assert_eq!(page.rows.len(), 1);
    let p = review(&app, &id, selected(&app, &id, 0, StashKind::Drop));
    let path = root.join(".git/refs/stash.lock");
    fs::write(&path, "External Git lock").unwrap();
    let receipt = app.apply_stash(&id, &p.request, &p.token).unwrap();
    assert_ne!(receipt.status, "succeeded");
    assert_eq!(fs::read_to_string(path).unwrap(), "External Git lock");
    assert_eq!(app.stashes(&id).unwrap().entries.len(), 1);
}
#[test]
fn linked_worktrees_share_stashes_and_drop_rewrites_the_native_reflog_compatibly() {
    let (t, app, root, id) = setup();
    let linked = t.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "linked", linked.to_str().unwrap()],
    );
    let c = app.add_repository(&linked).unwrap();
    let other = c.workspaces[0].selected_id.clone().unwrap();
    for n in [1, 2, 3] {
        fs::write(root.join("file.txt"), format!("{n}\n")).unwrap();
        apply(&app, &id, &review(&app, &id, request(StashKind::Push)));
    }
    assert_eq!(app.stashes(&other).unwrap().entries.len(), 3);
    let old = app.stashes(&id).unwrap().entries;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [".git/refs/stash", ".git/logs/refs/stash"] {
            fs::set_permissions(root.join(path), fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    let p = review(&app, &other, selected(&app, &other, 1, StashKind::Drop));
    apply(&app, &other, &p);
    let new = app.stashes(&id).unwrap().entries;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [".git/refs/stash", ".git/logs/refs/stash"] {
            assert_eq!(
                fs::metadata(root.join(path)).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    assert_eq!(
        new.iter().map(|e| &e.oid).collect::<Vec<_>>(),
        vec![&old[0].oid, &old[2].oid]
    );
    git(&root, &["stash", "drop", "stash@{0}"]);
    assert_eq!(app.stashes(&other).unwrap().entries[0].oid, old[2].oid);
}

#[test]
fn preview_does_not_run_helpers_and_drop_respects_reference_hook_policy() {
    let (_t, app, root, id) = setup();
    fs::write(root.join("file.txt"), "saved\n").unwrap();
    apply(&app, &id, &review(&app, &id, request(StashKind::Push)));
    let marker = root.join("must-not-run");
    let hook = root.join(".git/hooks/reference-transaction");
    fs::write(
        &hook,
        format!("#!/bin/sh\nprintf ran > '{}'\n", marker.display()),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let before = fs::read(root.join(".git/index")).unwrap();
    let p = review(&app, &id, selected(&app, &id, 0, StashKind::Drop));
    assert!(p
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("Reference-transaction hooks"));
    assert!(!marker.exists());
    let result = app.apply_stash(&id, &p.request, &p.token).unwrap();
    assert_eq!(result.status, "failed");
    assert!(!marker.exists());
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), before);
    assert_eq!(app.stashes(&id).unwrap().entries.len(), 1);
}
#[test]
fn packed_stash_refs_stay_blocked_and_large_previews_are_bounded_read_only() {
    let (_t, app, root, id) = setup();
    fs::write(root.join("file.txt"), "line\n".repeat(120000)).unwrap();
    let head = git(&root, &["rev-parse", "HEAD"]);
    let before = fs::read(root.join("file.txt")).unwrap();
    let index = fs::read(root.join(".git/index")).unwrap();
    let preview = review(&app, &id, request(StashKind::Push));
    assert!(preview.truncated);
    assert!(preview.diff.len() <= 512 * 1024);
    assert!(preview.blocked_reason.is_none());
    assert_eq!(preview.files.len(), 1);
    assert_eq!(fs::read(root.join("file.txt")).unwrap(), before);
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    assert!(app.stashes(&id).unwrap().entries.is_empty());
    fs::write(root.join("file.txt"), "small\n").unwrap();
    apply(&app, &id, &review(&app, &id, request(StashKind::Push)));
    git(&root, &["pack-refs", "--all"]);
    let p = review(&app, &id, selected(&app, &id, 0, StashKind::Drop));
    assert!(
        p.blocked_reason
            .as_deref()
            .unwrap()
            .contains("loose stash ref")
            || p.blocked_reason
                .as_deref()
                .unwrap()
                .contains("Packed stash")
    );
    assert_eq!(app.stashes(&id).unwrap().entries.len(), 1);
}
#[test]
fn ignored_collisions_and_deleted_index_paths_never_discard_local_contents() {
    let (_t, app, root, id) = setup();
    fs::write(root.join("collision.txt"), "saved new file\n").unwrap();
    let mut r = request(StashKind::Push);
    r.include_untracked = true;
    apply(&app, &id, &review(&app, &id, r));
    fs::write(root.join(".git/info/exclude"), "collision.txt\nfile.txt\n").unwrap();
    fs::write(root.join("collision.txt"), "private ignored contents\n").unwrap();
    assert_eq!(git(&root, &["status", "--porcelain"]), "");
    let p = review(&app, &id, selected(&app, &id, 0, StashKind::Apply));
    assert!(p
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("Local files occupy"));
    let result = app.apply_stash(&id, &p.request, &p.token).unwrap();
    assert_eq!(result.status, "failed");
    assert_eq!(
        fs::read_to_string(root.join("collision.txt")).unwrap(),
        "private ignored contents\n"
    );
    git(&root, &["rm", "--cached", "file.txt"]);
    fs::write(root.join("file.txt"), "preserve removed-index work\n").unwrap();
    let p = review(&app, &id, request(StashKind::Push));
    assert!(p
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("Removed index paths"));
    assert_eq!(
        app.apply_stash(&id, &p.request, &p.token).unwrap().status,
        "failed"
    );
    assert_eq!(
        fs::read_to_string(root.join("file.txt")).unwrap(),
        "preserve removed-index work\n"
    );
}
#[test]
fn sha256_stashes_keep_exact_objects_during_native_reflog_rewrite() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("sha256");
    fs::create_dir(&root).unwrap();
    let support = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["init", "-b", "main", "--object-format=sha256"])
        .output()
        .unwrap();
    if !support.status.success() {
        return;
    }
    fs::write(root.join("file.txt"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "SHA256 base"]);
    let app = Application::new(t.path().join("state.json")).unwrap();
    let c = app.add_repository(&root).unwrap();
    let id = c.workspaces[0].selected_id.as_ref().unwrap();
    fs::write(root.join("file.txt"), "saved\n").unwrap();
    apply(&app, id, &review(&app, id, request(StashKind::Push)));
    let e = app.stashes(id).unwrap().entries[0].clone();
    assert_eq!(e.oid.len(), 64);
    let p = review(&app, id, selected(&app, id, 0, StashKind::Drop));
    apply(&app, id, &p);
    assert!(app.stashes(id).unwrap().entries.is_empty());
    assert_eq!(git(&root, &["rev-parse", &p.recovery_ref]), e.oid);
}

#[test]
fn every_stash_lists_its_own_recorded_base_without_including_internal_parents_in_history() {
    let (_t, app, root, id) = setup();
    let first_base = git(&root, &["rev-parse", "HEAD"]);
    fs::write(root.join("file.txt"), "first saved version\n").unwrap();
    git(&root, &["stash", "push", "-m", "first"]);
    fs::write(root.join("new-base.txt"), "another base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "New base"]);
    let second_base = git(&root, &["rev-parse", "HEAD"]);
    fs::write(root.join("file.txt"), "second saved version\n").unwrap();
    git(&root, &["stash", "push", "-m", "second"]);
    let list = app.stashes(&id).unwrap();
    assert_eq!(list.entries.len(), 2);
    assert_eq!(
        list.entries[0].base_oid.as_deref(),
        Some(second_base.as_str())
    );
    assert_eq!(
        list.entries[1].base_oid.as_deref(),
        Some(first_base.as_str())
    );
    let page = app.graph(&id, None).unwrap();
    assert_eq!(page.rows.len(), 2);
    assert!(!page
        .rows
        .iter()
        .any(|row| list.entries.iter().any(|entry| entry.oid == row.commit.oid)));
}

#[test]
fn main_stash_pops_onto_current_develop_branch_without_moving_either_tip() {
    let (_dir, app, root, id) = setup();
    git(&root, &["branch", "develop"]);
    fs::write(root.join("main-only.txt"), "main tip\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Main progress"]);
    let main = git(&root, &["rev-parse", "HEAD"]);
    fs::write(root.join("file.txt"), "saved from main\n").unwrap();
    fs::write(root.join("new.txt"), "new saved file\n").unwrap();
    let p = review(
        &app,
        &id,
        StashRequest {
            include_untracked: true,
            restore_index: false,
            ..request(StashKind::Push)
        },
    );
    apply(&app, &id, &p);
    git(&root, &["switch", "develop"]);
    fs::write(root.join("develop-only.txt"), "develop tip\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Develop progress"]);
    let develop = git(&root, &["rev-parse", "HEAD"]);
    let p = review(
        &app,
        &id,
        StashRequest {
            restore_index: false,
            ..selected(&app, &id, 0, StashKind::Pop)
        },
    );
    assert_eq!(p.branch.as_deref(), Some("develop"));
    apply(&app, &id, &p);
    assert_eq!(git(&root, &["branch", "--show-current"]), "develop");
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), develop);
    assert_eq!(git(&root, &["rev-parse", "main"]), main);
    assert_eq!(
        fs::read_to_string(root.join("file.txt")).unwrap(),
        "saved from main\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("new.txt")).unwrap(),
        "new saved file\n"
    );
    assert!(root.join("develop-only.txt").exists());
    assert!(!root.join("main-only.txt").exists());
    assert!(app.stashes(&id).unwrap().entries.is_empty());
    assert_eq!(git(&root, &["diff", "--cached", "--name-only"]), "");
}

#[test]
fn large_stash_reviews_all_paths_rejects_tail_edits_and_pops_exact_content_on_another_branch() {
    let (_t, app, root, id) = setup();
    let head = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["branch", "develop"]);
    let staged = format!(
        "staged start\n{}staged end\n",
        "staged line\n".repeat(60000)
    );
    let working = format!(
        "working start\n{}working end\n",
        "ข้อความทดสอบ\n".repeat(60000)
    );
    let untracked = format!("new start\n{}new end\n", "new line\n".repeat(80000));
    fs::write(root.join("file.txt"), &staged).unwrap();
    git(&root, &["add", "file.txt"]);
    fs::write(root.join("file.txt"), &working).unwrap();
    fs::write(root.join("new.txt"), &untracked).unwrap();
    fs::write(root.join("zz-note.txt"), "small final file\n").unwrap();
    let index = fs::read(root.join(".git/index")).unwrap();
    let mut r = request(StashKind::Push);
    r.include_untracked = true;
    let p = review(&app, &id, r);
    assert_eq!(p.version, 2);
    assert_eq!(p.files.len(), 3);
    assert!(p.truncated);
    assert!(p.index_truncated);
    assert!(p.blocked_reason.is_none());
    let path = |name: &str| NativePath::new(Path::new(name));
    let small = app
        .stash_patch(&id, &p.request, &p.token, &path("zz-note.txt"), false)
        .unwrap();
    assert!(small.diff.contains("+small final file"));
    assert!(!small.truncated);
    let index_patch = app
        .stash_patch(&id, &p.request, &p.token, &path("file.txt"), true)
        .unwrap();
    assert!(index_patch.diff.contains("+staged start"));
    assert!(index_patch.truncated);
    assert!(app
        .stash_patch(&id, &p.request, &p.token, &path("absent.txt"), false)
        .is_err());
    fs::write(
        root.join("file.txt"),
        format!("{working}changed after the bounded prefix\n"),
    )
    .unwrap();
    assert!(app
        .stash_patch(&id, &p.request, &p.token, &path("zz-note.txt"), false)
        .is_err());
    assert_eq!(
        app.apply_stash(&id, &p.request, &p.token).unwrap().status,
        "failed"
    );
    assert!(app.stashes(&id).unwrap().entries.is_empty());
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    fs::write(root.join("file.txt"), &working).unwrap();
    let p = review(&app, &id, p.request);
    apply(&app, &id, &p);
    assert_eq!(git(&root, &["status", "--porcelain"]), "");
    git(&root, &["switch", "develop"]);
    let p = review(&app, &id, selected(&app, &id, 0, StashKind::Pop));
    assert!(p.truncated);
    assert!(p.index_truncated);
    assert_eq!(p.files.len(), 3);
    assert!(p.blocked_reason.is_none());
    assert!(
        app.stash_patch(&id, &p.request, &p.token, &path("new.txt"), false)
            .unwrap()
            .truncated
    );
    assert!(app
        .stash_patch(&id, &p.request, &p.token, &path("new.txt"), true)
        .unwrap()
        .diff
        .is_empty());
    assert!(app
        .stash_patch(&id, &p.request, &p.token, &path("zz-note.txt"), false)
        .unwrap()
        .diff
        .contains("+small final file"));
    apply(&app, &id, &p);
    assert_eq!(fs::read_to_string(root.join("file.txt")).unwrap(), working);
    assert_eq!(git(&root, &["show", ":file.txt"]), staged.trim());
    assert_eq!(fs::read_to_string(root.join("new.txt")).unwrap(), untracked);
    assert_eq!(
        fs::read_to_string(root.join("zz-note.txt")).unwrap(),
        "small final file\n"
    );
    assert!(app.stashes(&id).unwrap().entries.is_empty());
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&root, &["rev-parse", "main"]), head);
    assert_eq!(git(&root, &["branch", "--show-current"]), "develop");
}
#[test]
fn saved_index_only_paths_are_listed_and_patch_paths_are_literal() {
    let (_t, app, root, id) = setup();
    fs::write(root.join("index-only.txt"), "only in index\n").unwrap();
    git(&root, &["add", "index-only.txt"]);
    fs::remove_file(root.join("index-only.txt")).unwrap();
    git(&root, &["stash", "push", "-m", "index only"]);
    // Some Git versions normalize a staged-added/deleted path into the stash
    // work tree. Construct a valid index-only shape to exercise that distinction.
    let head = git(&root, &["rev-parse", "HEAD"]);
    let index = git(&root, &["rev-parse", "refs/stash^2"]);
    let tree = git(&root, &["rev-parse", "HEAD^{tree}"]);
    let oid = git(
        &root,
        &[
            "commit-tree",
            &tree,
            "-p",
            &head,
            "-p",
            &index,
            "-m",
            "Index-only saved work",
        ],
    );
    git(
        &root,
        &["stash", "store", "-m", "Index-only saved work", &oid],
    );
    let p = review(&app, &id, selected(&app, &id, 0, StashKind::Apply));
    let path = NativePath::new(Path::new("index-only.txt"));
    assert!(p.files.contains(&path));
    assert!(app
        .stash_patch(&id, &p.request, &p.token, &path, true)
        .unwrap()
        .diff
        .contains("+only in index"));
    assert!(app
        .stash_patch(&id, &p.request, &p.token, &path, false)
        .unwrap()
        .diff
        .is_empty());
    #[cfg(unix)]
    {
        let path = NativePath::new(Path::new(":(glob)*.txt"));
        fs::write(root.join(":(glob)*.txt"), "literal untracked\n").unwrap();
        git(
            &root,
            &["stash", "push", "--include-untracked", "-m", "literal path"],
        );
        let p = review(&app, &id, selected(&app, &id, 0, StashKind::Apply));
        let patch = app
            .stash_patch(&id, &p.request, &p.token, &path, false)
            .unwrap();
        assert!(patch.diff.contains("+literal untracked"));
        assert!(!patch.diff.contains("+base"));
    }
}

#[test]
fn saved_index_only_gitlinks_remain_blocked_when_working_delta_has_no_gitlinks() {
    let (_t, app, root, id) = setup();
    let head = git(&root, &["rev-parse", "HEAD"]);
    let tree = git(&root, &["rev-parse", "HEAD^{tree}"]);
    git(
        &root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{head},child"),
        ],
    );
    let index_tree = git(&root, &["write-tree"]);
    let index_commit = git(
        &root,
        &[
            "commit-tree",
            &index_tree,
            "-p",
            &head,
            "-m",
            "Saved index pointer",
        ],
    );
    let saved = git(
        &root,
        &[
            "commit-tree",
            &tree,
            "-p",
            &head,
            "-p",
            &index_commit,
            "-m",
            "Saved pointer",
        ],
    );
    git(&root, &["reset", "--mixed", "HEAD"]);
    git(&root, &["stash", "store", "-m", "Saved pointer", &saved]);
    let index = fs::read(root.join(".git/index")).unwrap();
    let p = review(&app, &id, selected(&app, &id, 0, StashKind::Apply));
    assert!(p
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("submodule pointers"));
    assert_eq!(
        app.apply_stash(&id, &p.request, &p.token).unwrap().status,
        "failed"
    );
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&root, &["rev-parse", "refs/stash"]), saved);
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    assert!(!root.join("child").exists());
}
