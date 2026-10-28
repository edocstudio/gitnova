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
