use gitnova_core::Application;
use gitnova_git::GitCli;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Tracking Test",
            "-c",
            "user.email=tracking@example.invalid",
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
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}
fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    let local = temp.path().join("local");
    std::fs::create_dir(&remote).unwrap();
    std::fs::create_dir(&local).unwrap();
    git(&remote, &["init", "--bare"]);
    git(&local, &["init", "-b", "main"]);
    git(&local, &["commit", "--allow-empty", "-m", "Base"]);
    git(
        &local,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&local, &["push", "-u", "origin", "main:release"]);
    (temp, local, remote)
}
#[test]
fn graph_and_snapshots_count_real_merge_commits_and_configured_upstream_divergence() {
    let (temp, local, remote) = setup();
    git(&local, &["switch", "-c", "feature"]);
    for message in ["Feature one", "Feature two"] {
        git(&local, &["commit", "--allow-empty", "-m", message]);
    }
    git(&local, &["switch", "main"]);
    git(
        &local,
        &["merge", "--no-ff", "-m", "Merge feature", "feature"],
    );
    let head = git(&local, &["rev-parse", "HEAD"]);
    let app = Application::new(temp.path().join("app/state.json")).unwrap();
    let c = app.add_repository(&local).unwrap();
    let id = c.workspaces[0].selected_id.as_ref().unwrap();
    let page = app.graph(id, None).unwrap();
    let main = page.tracking.iter().find(|b| b.branch == "main").unwrap();
    assert_eq!((main.ahead, main.behind), (Some(3), Some(0)));
    assert_eq!(
        main.upstream.as_deref(),
        Some("refs/remotes/origin/release")
    );
    assert_eq!(main.remote_branch.as_deref(), Some("release"));
    assert!(!page.status.dirty);
    assert_eq!(page.status.changed_entries, 0);
    let other = temp.path().join("other");
    git(
        temp.path(),
        &[
            "clone",
            "--branch",
            "release",
            remote.to_str().unwrap(),
            other.to_str().unwrap(),
        ],
    );
    for message in ["Remote one", "Remote two"] {
        git(&other, &["commit", "--allow-empty", "-m", message]);
    }
    git(&other, &["push", "origin", "release"]);
    let cached = app.snapshot(id).unwrap();
    let main = cached.tracking.iter().find(|b| b.branch == "main").unwrap();
    assert_eq!(
        (main.ahead, main.behind),
        (Some(3), Some(0)),
        "No implicit network fetch"
    );
    git(&local, &["fetch", "origin"]);
    let snapshot = app.snapshot(id).unwrap();
    let main = snapshot
        .tracking
        .iter()
        .find(|b| b.branch == "main")
        .unwrap();
    assert_eq!((main.ahead, main.behind), (Some(3), Some(2)));
    assert_eq!(git(&local, &["rev-parse", "HEAD"]), head);
    git(
        &local,
        &["remote", "add", "publish", remote.to_str().unwrap()],
    );
    git(&local, &["config", "branch.main.pushRemote", "publish"]);
    git(&local, &["config", "push.default", "current"]);
    let rows = GitCli.branch_tracking(&local).unwrap();
    let main = rows.iter().find(|b| b.branch == "main").unwrap();
    assert_eq!(main.push_remote.as_deref(), Some("publish"));
    assert_eq!(main.push_branch.as_deref(), Some("main"));
    assert_eq!((main.ahead, main.behind), (Some(3), Some(2)));
}
#[test]
fn untracked_and_missing_upstreams_are_unknown_instead_of_zero() {
    let (_temp, local, _remote) = setup();
    git(&local, &["branch", "untracked"]);
    git(&local, &["update-ref", "-d", "refs/remotes/origin/release"]);
    let rows = GitCli.branch_tracking(&local).unwrap();
    let main = rows.iter().find(|b| b.branch == "main").unwrap();
    assert_eq!(main.state, "gone");
    assert_eq!(main.ahead, None);
    assert_eq!(main.behind, None);
    let untracked = rows.iter().find(|b| b.branch == "untracked").unwrap();
    assert_eq!(untracked.state, "noUpstream");
    assert_eq!(untracked.ahead, None);
}
#[test]
fn shallow_tracking_does_not_claim_complete_counts() {
    let (temp, local, remote) = setup();
    git(&local, &["commit", "--allow-empty", "-m", "Second"]);
    git(&local, &["push", "origin", "main:release"]);
    let shallow = temp.path().join("shallow");
    git(
        temp.path(),
        &[
            "clone",
            "--depth=1",
            "--branch",
            "release",
            &format!("file://{}", remote.display()),
            shallow.to_str().unwrap(),
        ],
    );
    let rows = GitCli.branch_tracking(&shallow).unwrap();
    let branch = rows.iter().find(|b| b.branch == "release").unwrap();
    assert_eq!(branch.state, "shallow");
    assert_eq!(branch.ahead, None);
    assert_eq!(branch.behind, None);
}

#[test]
fn untracked_current_branch_compares_origin_main_for_display_without_configuring_it() {
    let (temp, local, remote) = setup();
    git(&local, &["push", "origin", "main:main"]);
    git(&local, &["switch", "-c", "fix/display-only"]);
    git(&local, &["commit", "--allow-empty", "-m", "Local one"]);
    let head = git(&local, &["rev-parse", "HEAD"]);
    std::fs::write(local.join("dirty.txt"), "keep working files").unwrap();
    let writer = temp.path().join("writer");
    git(
        temp.path(),
        &[
            "clone",
            "--branch",
            "main",
            remote.to_str().unwrap(),
            writer.to_str().unwrap(),
        ],
    );
    for message in ["Remote one", "Remote two"] {
        git(&writer, &["commit", "--allow-empty", "-m", message]);
    }
    git(&writer, &["push", "origin", "main"]);
    git(&local, &["fetch", "origin"]);
    let app = Application::new(temp.path().join("display-state.json")).unwrap();
    let catalog = app.add_repository(&local).unwrap();
    let id = catalog.workspaces[0].selected_id.as_ref().unwrap();
    let snapshot = app.snapshot(id).unwrap();
    let current = snapshot
        .tracking
        .iter()
        .find(|row| row.branch == "fix/display-only")
        .unwrap();
    assert_eq!(current.state, "noUpstream");
    assert!(current.upstream.is_none());
    assert!(current.ahead.is_none());
    let comparison = current.comparison.as_ref().unwrap();
    assert_eq!(comparison.reference, "refs/remotes/origin/main");
    assert_eq!((comparison.ahead, comparison.behind), (1, 2));
    assert!(
        GitCli
            .branch_tracking(&local)
            .unwrap()
            .iter()
            .all(|row| row.comparison.is_none()),
        "Write metadata must never use the fallback"
    );
    let (_, metrics) = gitnova_git::measure_git(|| app.snapshot(id).unwrap());
    assert!(
        !metrics.commands.contains_key("rev-list"),
        "Repeated display reads should use the bounded cache"
    );
    git(&writer, &["commit", "--allow-empty", "-m", "Remote three"]);
    git(&writer, &["push", "origin", "main"]);
    git(&local, &["fetch", "origin"]);
    let snapshot = app.snapshot(id).unwrap();
    assert_eq!(
        snapshot
            .tracking
            .iter()
            .find(|row| row.branch == "fix/display-only")
            .unwrap()
            .comparison
            .as_ref()
            .unwrap()
            .behind,
        3
    );
    git(&local, &["update-ref", "-d", "refs/remotes/origin/main"]);
    git(
        &local,
        &["update-ref", "refs/remotes/origin/main/child", &head],
    );
    assert!(
        app.snapshot(id)
            .unwrap()
            .tracking
            .iter()
            .find(|row| row.branch == "fix/display-only")
            .unwrap()
            .comparison
            .is_none(),
        "A prefix match is not origin/main"
    );
    assert_eq!(git(&local, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        git(
            &local,
            &[
                "for-each-ref",
                "--format=%(upstream)",
                "refs/heads/fix/display-only"
            ]
        ),
        ""
    );
    assert_eq!(
        std::fs::read_to_string(local.join("dirty.txt")).unwrap(),
        "keep working files"
    );
}

#[test]
fn published_branch_without_upstream_compares_its_own_remote_and_invalidates_display_cache() {
    let (temp, local, _) = setup();
    git(&local, &["push", "origin", "main:main"]);
    git(&local, &["switch", "-c", "feature/published"]);
    for message in ["Feature one", "Feature two"] {
        git(&local, &["commit", "--allow-empty", "-m", message]);
    }
    let app = Application::new(temp.path().join("published-state.json")).unwrap();
    let catalog = app.add_repository(&local).unwrap();
    let id = catalog.workspaces[0].selected_id.as_ref().unwrap();
    let comparison = || {
        app.snapshot(id)
            .unwrap()
            .tracking
            .into_iter()
            .find(|row| row.branch == "feature/published")
            .unwrap()
            .comparison
            .unwrap()
    };
    assert_eq!(comparison().ahead, 2);
    git(&local, &["push", "origin", "feature/published"]);
    let published = comparison();
    assert_eq!(published.reference, "refs/remotes/origin/feature/published");
    assert_eq!((published.ahead, published.behind), (0, 0));
    assert_eq!(
        git(
            &local,
            &[
                "for-each-ref",
                "--format=%(upstream)",
                "refs/heads/feature/published"
            ]
        ),
        ""
    );
    git(
        &local,
        &["commit", "--allow-empty", "-m", "New unpublished work"],
    );
    assert_eq!(comparison().ahead, 1);
    git(&local, &["push", "origin", "feature/published"]);
    assert_eq!(comparison().ahead, 0);
    // An explicit upstream always takes priority, even if another ref has the same tip.
    git(&local, &["branch", "--set-upstream-to=origin/main"]);
    let snapshot = app.snapshot(id).unwrap();
    let tracked = snapshot
        .tracking
        .iter()
        .find(|row| row.branch == "feature/published")
        .unwrap();
    assert_eq!(tracked.state, "tracked");
    assert_eq!(tracked.ahead, Some(3));
    assert!(tracked.comparison.is_none());
}
