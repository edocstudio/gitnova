use gitnova_core::Application;
use gitnova_git::measure_git;
use std::{fs, path::Path, process::Command};
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
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn fixture() -> (tempfile::TempDir, Application, String, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Fixture"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    fs::write(root.join("file"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);
    git(
        &root,
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/project.git",
        ],
    );
    let head = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["update-ref", "refs/remotes/origin/main", &head]);
    git(&root, &["branch", "--set-upstream-to=origin/main", "main"]);
    let app = Application::new(dir.path().join("state.json")).unwrap();
    let id = app.add_repository(&root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    (dir, app, id, root)
}
#[test]
fn file_list_and_count_share_one_status_scan_and_keep_the_index() {
    let (_dir, app, id, root) = fixture();
    let before = fs::read(root.join(".git/index")).unwrap();
    fs::write(root.join("file"), "changed\n").unwrap();
    fs::create_dir(root.join("new")).unwrap();
    fs::write(root.join("new/one"), "new\n").unwrap();
    let (result, metrics) = measure_git(|| app.working_changes(&id).unwrap());
    assert_eq!(result.status.changed_entries, result.files.len());
    assert_eq!(result.files.len(), 2);
    assert_eq!(metrics.commands.get("status"), Some(&1));
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), before);
}
#[test]
fn display_tracking_cache_never_hides_working_changes_refs_or_config_changes() {
    let (_dir, app, id, root) = fixture();
    let (first, cold) = measure_git(|| app.snapshot(&id).unwrap());
    assert_eq!(cold.commands.get("for-each-ref"), Some(&1));
    assert_eq!(first.tracking[0].ahead, Some(0));
    fs::write(root.join("file"), "dirty\n").unwrap();
    let (warm, metrics) = measure_git(|| app.snapshot(&id).unwrap());
    assert!(warm.changes.status.dirty);
    assert!(!metrics.commands.contains_key("for-each-ref"));
    assert!(metrics.commands.contains_key("status"));
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "external advance"]);
    let (changed, metrics) = measure_git(|| app.snapshot(&id).unwrap());
    assert_eq!(changed.tracking[0].ahead, Some(1));
    assert_eq!(metrics.commands.get("for-each-ref"), Some(&1));
    git(
        &root,
        &["config", "branch.main.merge", "refs/heads/missing"],
    );
    let (changed, metrics) = measure_git(|| app.snapshot(&id).unwrap());
    assert_eq!(changed.tracking[0].state, "gone");
    assert_eq!(metrics.commands.get("for-each-ref"), Some(&1));
}
