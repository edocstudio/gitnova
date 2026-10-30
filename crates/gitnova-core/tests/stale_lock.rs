#![cfg(target_os = "macos")]
use gitnova_core::Application;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(p: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(p)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().into()
}
fn fixture() -> (tempfile::TempDir, PathBuf, Application, String) {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Fixture"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    fs::write(root.join("file"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(
        &root,
        &["-c", "commit.gpgsign=false", "commit", "-m", "base"],
    );
    let a = Application::new(d.path().join("catalog.json")).unwrap();
    let id = a.add_repository(&root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    (d, root, a, id)
}
fn dead_pid() -> u32 {
    let mut p = Command::new("/usr/bin/true").spawn().unwrap();
    let id = p.id();
    p.wait().unwrap();
    id
}
fn stale(root: &Path) -> Vec<u8> {
    let bytes = format!("GitNova pid={}\n", dead_pid()).into_bytes();
    fs::write(root.join(".git/gitnova-operation.lock"), &bytes).unwrap();
    bytes
}
#[test]
fn archives_dead_owner_only_and_preserves_dirty_staging_refs_and_journals() {
    let (_d, root, a, id) = fixture();
    fs::write(root.join("file"), "staged\n").unwrap();
    git(&root, &["add", "file"]);
    fs::write(root.join("file"), "working\n").unwrap();
    let bytes = stale(&root);
    let index = fs::read(root.join(".git/index")).unwrap();
    let refs = git(&root, &["show-ref"]);
    let journal = root.join(".git/gitnova-merge-retained");
    fs::create_dir(&journal).unwrap();
    fs::write(journal.join("session.json"), "retained diagnostic").unwrap();
    let p = a.preview_stale_lock(&id).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    let archive = a
        .recover_stale_lock(&id, &p.token)
        .unwrap()
        .to_path()
        .unwrap();
    assert_eq!(fs::read(archive).unwrap(), bytes);
    assert!(!root.join(".git/gitnova-operation.lock").exists());
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    assert_eq!(git(&root, &["show-ref"]), refs);
    assert_eq!(fs::read_to_string(root.join("file")).unwrap(), "working\n");
    assert_eq!(
        fs::read_to_string(journal.join("session.json")).unwrap(),
        "retained diagnostic"
    );
    assert!(!a.preview_stale_lock(&id).unwrap().exists);
    assert!(a.recover_stale_lock(&id, &p.token).is_err());
}
#[test]
fn alive_owner_malformed_symlink_and_native_state_are_never_removed() {
    let (_d, root, a, id) = fixture();
    let lock = root.join(".git/gitnova-operation.lock");
    fs::write(&lock, format!("GitNova pid={}\n", std::process::id())).unwrap();
    let p = a.preview_stale_lock(&id).unwrap();
    assert!(p.blocked_reason.unwrap().contains("still running"));
    assert!(a.recover_stale_lock(&id, &"a".repeat(64)).is_err());
    fs::write(&lock, "foreign lock").unwrap();
    assert!(a.preview_stale_lock(&id).unwrap().blocked_reason.is_some());
    fs::remove_file(&lock).unwrap();
    std::os::unix::fs::symlink(root.join("file"), &lock).unwrap();
    assert!(a.preview_stale_lock(&id).unwrap().blocked_reason.is_some());
    assert!(a.recover_stale_lock(&id, &"a".repeat(64)).is_err());
    assert_eq!(fs::read_to_string(root.join("file")).unwrap(), "base\n");
    fs::remove_file(&lock).unwrap();
    stale(&root);
    for marker in [
        "index.lock",
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
    ] {
        fs::write(root.join(".git").join(marker), "native").unwrap();
        assert!(a.preview_stale_lock(&id).unwrap().blocked_reason.is_some());
        assert!(lock.exists());
        fs::remove_file(root.join(".git").join(marker)).unwrap();
    }
}
#[test]
fn changed_inode_or_content_invalidates_confirmation() {
    let (_d, root, a, id) = fixture();
    let bytes = stale(&root);
    let p = a.preview_stale_lock(&id).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    let lock = root.join(".git/gitnova-operation.lock");
    fs::rename(&lock, root.join("old-lock")).unwrap();
    fs::write(&lock, &bytes).unwrap();
    assert!(a.recover_stale_lock(&id, &p.token).is_err());
    let p = a.preview_stale_lock(&id).unwrap();
    fs::write(root.join("file"), "new edit\n").unwrap();
    git(&root, &["add", "file"]);
    assert!(a.recover_stale_lock(&id, &p.token).is_err());
    assert_eq!(fs::read(&lock).unwrap(), bytes);
}
#[test]
fn common_lock_is_visible_from_linked_worktree_and_other_native_state_blocks() {
    let (d, root, a, id) = fixture();
    let linked = d.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    let linked_id = a.add_repository(&linked).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    let bytes = stale(&root);
    let admin = PathBuf::from(git(&linked, &["rev-parse", "--absolute-git-dir"]));
    fs::write(admin.join("index.lock"), "native").unwrap();
    assert!(a.preview_stale_lock(&id).unwrap().blocked_reason.is_some());
    fs::remove_file(admin.join("index.lock")).unwrap();
    let p = a.preview_stale_lock(&linked_id).unwrap();
    assert_eq!(p.path, a.preview_stale_lock(&id).unwrap().path);
    assert!(p.blocked_reason.is_none(), "{p:?}");
    let archive = a.recover_stale_lock(&linked_id, &p.token).unwrap();
    assert_eq!(fs::read(archive.to_path().unwrap()).unwrap(), bytes);
    assert!(linked.join("file").exists());
}
#[test]
fn orphan_git_process_blocks_recovery_until_it_exits() {
    let (_d, root, a, id) = fixture();
    stale(&root);
    let p = a.preview_stale_lock(&id).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    let sleeper = root.join("git-upload-pack");
    fs::copy("/bin/sleep", &sleeper).unwrap();
    let mut child = Command::new(&sleeper).arg("10").spawn().unwrap();
    let blocked = a.preview_stale_lock(&id).unwrap();
    let result = a.recover_stale_lock(&id, &p.token);
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(blocked.blocked_reason.is_some());
    assert!(result.is_err());
    assert!(root.join(".git/gitnova-operation.lock").exists());
    assert!(a.preview_stale_lock(&id).unwrap().blocked_reason.is_none());
}
