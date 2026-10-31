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
            "user.name=Worktree Test",
            "-c",
            "user.email=worktree@example.invalid",
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
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
fn setup() -> (tempfile::TempDir, Application, PathBuf, String) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("source space ' ไทย");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "core.autocrlf", "false"]);
    fs::write(repo.join("file.txt"), "base\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Base"]);
    // Isolate native confirmed hooks from the developer's global configuration too.
    git(&repo, &["config", "core.hooksPath", "/dev/null"]);
    let app = Application::new(dir.path().join("app/state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    (dir, app, repo, id)
}
fn record(dir: &Path, member: &Membership, serial: u64, status: &str) -> PathBuf {
    let id = format!("00000000-0000-4000-8000-{serial:012x}");
    let receipt = serde_json::json!({
        "id":id,"kind":"stage","status":status,"summary":"Recorded test",
        "repositoryId":member.repository_id,"worktreeId":member.worktree_id,
        "membershipId":member.id,"startedAt":serial,"finishedAt":null,
        "headBefore":null,"headAfter":null,"recovery":null,"submodule":null
    });
    let folder = dir.join("app/operations");
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join(format!("{id}.json"));
    fs::write(&path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    path
}
#[test]
fn discarded_content_and_recorded_location_survive_application_restart() {
    let (dir, app, repo, id) = setup();
    fs::write(repo.join("file.txt"), "valuable draft\n").unwrap();
    let request = DiscardRequest {
        paths: vec![NativePath::new(Path::new("file.txt"))],
        scope: DiscardScope::Unstaged,
        id: String::new(),
    };
    let preview = app.preview_discard(&id, &request).unwrap();
    let receipt = app
        .discard_changes(&id, &preview.request, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{receipt:?}");
    drop(app);
    let app = Application::new(dir.path().join("app/state.json")).unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    let catalog = fs::read(dir.path().join("app/state.json")).unwrap();
    let history = app.recorded_activity(&id).unwrap();
    assert!(!history.incomplete);
    assert_eq!(history.entries.len(), 1);
    let entry = &history.entries[0];
    assert_eq!(entry.id, receipt.id);
    assert!(entry.current_checkout);
    assert_eq!(
        entry.recovery_dir.as_ref().unwrap().display,
        preview.recovery_dir.display
    );
    assert_eq!(
        fs::read_to_string(preview.recovery_dir.to_path().unwrap().join("work-0")).unwrap(),
        "valuable draft\n"
    );
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read(dir.path().join("app/state.json")).unwrap(),
        catalog
    );
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "base\n");
}
#[test]
fn history_scopes_shared_repository_and_leaves_unconfirmed_records_and_locks_untouched() {
    let (dir, app, repo, id) = setup();
    let linked = dir.path().join("linked");
    git(
        &repo,
        &["worktree", "add", "-b", "dev", linked.to_str().unwrap()],
    );
    app.add_repository(&linked).unwrap();
    let catalog = app.catalog().unwrap();
    let members = &catalog.workspaces[0].memberships;
    let current = members.iter().find(|m| m.id == id).unwrap();
    let other = members.iter().find(|m| m.id != id).unwrap();
    let current_path = record(dir.path(), current, 10, "started");
    record(dir.path(), other, 20, "unknown");
    let mut unrelated = current.clone();
    unrelated.repository_id = "unrelated-repository".into();
    record(dir.path(), &unrelated, 30, "succeeded");
    let lock = repo.join(".git/index.lock");
    fs::write(&lock, "another operation").unwrap();
    let original = fs::read(&current_path).unwrap();
    let history = app.recorded_activity(&id).unwrap();
    assert_eq!(history.entries.len(), 2);
    assert_eq!(history.entries[0].status, "unknown");
    assert!(!history.entries[0].current_checkout);
    assert!(history.entries[1].current_checkout);
    assert_eq!(fs::read(current_path).unwrap(), original);
    assert_eq!(fs::read_to_string(lock).unwrap(), "another operation");
    assert!(app.recorded_activity("not-registered").is_err());
}
#[test]
fn unreadable_and_bounded_history_is_explicitly_incomplete() {
    let (dir, app, _repo, id) = setup();
    let catalog = app.catalog().unwrap();
    let member = &catalog.workspaces[0].memberships[0];
    assert!(app.recorded_activity(&id).unwrap().entries.is_empty());
    for serial in 1..=129 {
        record(dir.path(), member, serial, "succeeded");
    }
    let history = app.recorded_activity(&id).unwrap();
    assert!(history.incomplete);
    assert_eq!(history.entries.len(), 10);
    assert!(history
        .entries
        .windows(2)
        .all(|pair| pair[0].started_at >= pair[1].started_at));
    fs::remove_dir_all(dir.path().join("app/operations")).unwrap();
    let path = record(dir.path(), member, 1, "succeeded");
    fs::write(path, "invalid json").unwrap();
    let path = record(dir.path(), member, 2, "succeeded");
    fs::write(path, vec![b' '; 2 * 1024 * 1024 + 1]).unwrap();
    let history = app.recorded_activity(&id).unwrap();
    assert!(history.incomplete);
    assert!(history.entries.is_empty());
}
#[cfg(unix)]
#[test]
fn symlink_records_and_directory_are_not_followed() {
    use std::os::unix::fs::symlink;
    let (dir, app, _repo, id) = setup();
    let catalog = app.catalog().unwrap();
    let member = &catalog.workspaces[0].memberships[0];
    let path = record(dir.path(), member, 1, "succeeded");
    let outside = dir.path().join("outside.json");
    fs::rename(&path, &outside).unwrap();
    symlink(&outside, &path).unwrap();
    let history = app.recorded_activity(&id).unwrap();
    assert!(history.incomplete);
    assert!(history.entries.is_empty());
    fs::remove_dir_all(dir.path().join("app/operations")).unwrap();
    symlink(dir.path(), dir.path().join("app/operations")).unwrap();
    assert!(app.recorded_activity(&id).is_err());
    assert!(outside.exists());
}
