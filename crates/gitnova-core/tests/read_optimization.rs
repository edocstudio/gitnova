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

#[test]
fn submodule_discovery_without_gitlinks_skips_status_and_head_tree_scans() {
    let (_dir, app, id, root) = fixture();
    fs::write(root.join("file"), "dirty\n").unwrap();
    let index = fs::read(root.join(".git/index")).unwrap();
    let (rows, metrics) = measure_git(|| app.submodules(&id).unwrap());
    assert!(rows.is_empty());
    assert!(!metrics.commands.contains_key("status"));
    assert!(!metrics.commands.contains_key("ls-tree"));
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    eprintln!(
        "Submodule discovery without gitlinks: {:?}",
        metrics.commands
    );
}

#[test]
fn graph_reuses_request_metadata_but_rejects_external_ref_changes() {
    let (_dir, app, id, root) = fixture();
    let first = app.graph(&id, None).unwrap();
    let (warm, metrics) = measure_git(|| app.graph(&id, None).unwrap());
    assert_eq!(warm.fingerprint, first.fingerprint);
    assert_eq!(
        serde_json::to_value(&warm.refs).unwrap(),
        serde_json::to_value(&first.refs).unwrap()
    );
    assert_eq!(metrics.commands.get("rev-parse"), Some(&5));
    assert_eq!(metrics.commands.get("show-ref"), Some(&2));
    assert_eq!(metrics.commands.get("status"), Some(&1));
    // Create a valid page cursor, then change refs outside the application.
    let cursor = gitnova_domain::GraphCursor {
        version: 2,
        fingerprint: first.fingerprint,
        ..Default::default()
    };
    git(&root, &["commit", "--allow-empty", "-m", "external"]);
    assert_eq!(
        app.graph(&id, Some(cursor)).unwrap_err().code,
        "stale_graph"
    );
    fs::write(root.join("file"), "new working edit\n").unwrap();
    let fresh = app.graph(&id, None).unwrap();
    assert!(fresh.status.dirty);
    assert_eq!(fresh.rows[0].commit.subject, "external");
    assert_eq!(fresh.tracking[0].ahead, Some(1));
}

#[test]
fn graph_observations_keep_linked_worktrees_and_repositories_separate() {
    let (dir, app, id, root) = fixture();
    let original = app.graph(&id, None).unwrap();
    let linked = dir.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    git(&linked, &["commit", "--allow-empty", "-m", "side only"]);
    let other = app.add_repository(&linked).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    let side = app.graph(&other, None).unwrap();
    assert_eq!(side.status.branch.as_deref(), Some("side"));
    assert_ne!(side.status.head, original.status.head);
    let main = app.graph(&id, None).unwrap();
    assert_eq!(main.status.branch.as_deref(), Some("main"));
    assert_eq!(main.status.head, original.status.head);
    assert_ne!(main.fingerprint, side.fingerprint);
    git(&linked, &["checkout", "--detach"]);
    assert!(app.graph(&other, None).unwrap().status.branch.is_none());
}

#[test]
fn graph_reads_unborn_bare_and_unusual_native_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("unborn");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    let app = Application::new(dir.path().join("catalog.json")).unwrap();
    let id = app.add_repository(&root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    assert!(app.graph(&id, None).unwrap().rows.is_empty());
    let bare = dir.path().join("bare.git");
    fs::create_dir(&bare).unwrap();
    git(&bare, &["init", "--bare"]);
    let id = app.add_repository(&bare).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    assert!(app.graph(&id, None).unwrap().rows.is_empty());
    #[cfg(unix)]
    {
        let unusual = dir.path().join("native\npath");
        fs::rename(root, &unusual).unwrap();
        let id = app.add_repository(&unusual).unwrap().workspaces[0]
            .selected_id
            .clone()
            .unwrap();
        assert!(app.graph(&id, None).unwrap().rows.is_empty());
    }
}

#[test]
fn shared_graph_keys_match_uncached_keys_and_shallow_changes_expire_cursors() {
    let (_dir, app, id, root) = fixture();
    let cli = gitnova_git::GitCli;
    let (fingerprint, tracking) = cli.graph_read_keys(&root).unwrap();
    assert_eq!(fingerprint, cli.fingerprint(&root).unwrap());
    assert_eq!(
        tracking.unwrap(),
        cli.tracking_display_key(&root, &fingerprint).unwrap()
    );
    let cursor = gitnova_domain::GraphCursor {
        version: 2,
        fingerprint,
        ..Default::default()
    };
    let head = git(&root, &["rev-parse", "HEAD"]);
    fs::write(root.join(".git/shallow"), format!("{head}\n")).unwrap();
    assert_eq!(
        app.graph(&id, Some(cursor)).unwrap_err().code,
        "stale_graph"
    );
    assert_eq!(app.graph(&id, None).unwrap().rows.len(), 1);
}
