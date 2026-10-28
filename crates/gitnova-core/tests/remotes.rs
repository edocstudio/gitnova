use gitnova_core::Application;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
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
    String::from_utf8_lossy(&out.stdout).trim_end().into()
}
fn commit(repo: &Path, file: &str, contents: &str) -> String {
    fs::write(repo.join(file), contents).unwrap();
    git(repo, &["add", "--", file]);
    git(repo, &["commit", "-m", contents]);
    git(repo, &["rev-parse", "HEAD"])
}
struct Fixture {
    _dir: tempfile::TempDir,
    writer: PathBuf,
    repo: PathBuf,
    app: Application,
    id: String,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let writer = dir.path().join("writer");
    let repo = dir.path().join("local");
    fs::create_dir(&writer).unwrap();
    git(&writer, &["init", "-b", "main"]);
    commit(&writer, "file.txt", "initial\n");
    git(
        dir.path(),
        &[
            "clone",
            "--",
            writer.to_str().unwrap(),
            repo.to_str().unwrap(),
        ],
    );
    for (key, value) in [
        ("user.name", "Fixture"),
        ("user.email", "fixture@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", ".git/hooks"),
        ("core.attributesFile", ".git/empty-attributes"),
    ] {
        git(&repo, &["config", key, value]);
    }
    let app = Application::new(dir.path().join("app/state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    Fixture {
        _dir: dir,
        writer,
        repo,
        app,
        id,
    }
}
fn fetch(f: &Fixture, branch: Option<&str>) {
    let r = f.app.fetch_remote(&f.id, "origin", branch).unwrap();
    assert_eq!(r.status, "succeeded", "{}", r.summary);
}
#[test]
fn fetch_discovers_heads_only_without_writing_local_branches_or_fetch_head() {
    let f = fixture();
    git(&f.writer, &["switch", "-c", "feature/api"]);
    let oid = commit(&f.writer, "api.txt", "new API\n");
    git(&f.writer, &["tag", "not-requested"]);
    let head = git(&f.repo, &["rev-parse", "HEAD"]);
    let index = fs::read(f.repo.join(".git/index")).unwrap();
    fs::write(f.repo.join(".git/FETCH_HEAD"), "preserved sentinel\n").unwrap();
    git(
        &f.repo,
        &[
            "config",
            "--add",
            "remote.origin.fetch",
            "+refs/heads/*:refs/heads/unsafe/*",
        ],
    );
    git(&f.repo, &["config", "remote.origin.pruneTags", "true"]);
    git(&f.repo, &["config", "remote.origin.tagOpt", "--tags"]);
    fetch(&f, None);
    let listing = f.app.remote_branches(&f.id).unwrap();
    assert_eq!(listing.remotes, vec!["origin"]);
    assert!(listing
        .branches
        .iter()
        .any(|b| b.name == "feature/api" && b.oid == oid));
    assert!(!listing.branches.iter().any(|b| b.name == "HEAD"));
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(f.repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(f.repo.join(".git/FETCH_HEAD")).unwrap(),
        "preserved sentinel\n"
    );
    assert!(git(
        &f.repo,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads/unsafe",
            "refs/tags"
        ]
    )
    .is_empty());
}
#[test]
fn remote_only_branch_creates_a_local_tracking_branch_at_reviewed_revision() {
    let f = fixture();
    git(&f.writer, &["switch", "-c", "feature/api"]);
    let oid = commit(&f.writer, "api.txt", "latest API\n");
    fetch(&f, None);
    let before = git(&f.repo, &["show-ref"]);
    let preview = f
        .app
        .preview_remote_switch(&f.id, "origin", "feature/api")
        .unwrap();
    assert!(preview.create);
    assert!(preview.switch.blocked_reason.is_none());
    assert_eq!(preview.switch.target_oid, oid);
    assert_eq!(git(&f.repo, &["show-ref"]), before);
    let receipt = f
        .app
        .switch_remote_branch(&f.id, "origin", "feature/api", &preview.switch.token)
        .unwrap();
    assert_eq!(
        receipt.status, "succeeded",
        "{} {:?}",
        receipt.summary, receipt.recovery
    );
    assert_eq!(git(&f.repo, &["branch", "--show-current"]), "feature/api");
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), oid);
    assert_eq!(
        git(
            &f.repo,
            &["rev-parse", "--symbolic-full-name", "@{upstream}"]
        ),
        "refs/remotes/origin/feature/api"
    );
    assert_eq!(
        fs::read_to_string(f.repo.join("api.txt")).unwrap(),
        "latest API\n"
    );
}
#[test]
fn current_local_branch_pulls_by_fast_forward_and_keeps_ahead_commits() {
    let f = fixture();
    let remote = commit(&f.writer, "file.txt", "remote update\n");
    fetch(&f, Some("main"));
    let preview = f
        .app
        .preview_remote_switch(&f.id, "origin", "main")
        .unwrap();
    assert_eq!((preview.ahead, preview.behind), (0, 1));
    assert!(!preview.create);
    assert!(preview.switch.blocked_reason.is_none());
    let receipt = f
        .app
        .switch_remote_branch(&f.id, "origin", "main", &preview.switch.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), remote);
    assert_eq!(
        fs::read_to_string(f.repo.join("file.txt")).unwrap(),
        "remote update\n"
    );
    let local = commit(&f.repo, "local.txt", "unpushed local\n");
    let preview = f
        .app
        .preview_remote_switch(&f.id, "origin", "main")
        .unwrap();
    assert_eq!((preview.ahead, preview.behind), (1, 0));
    assert_eq!(
        f.app
            .switch_remote_branch(&f.id, "origin", "main", &preview.switch.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), local);
}
#[test]
fn divergence_dirty_work_and_other_worktree_never_get_overwritten() {
    let f = fixture();
    let local = commit(&f.repo, "local.txt", "local work\n");
    commit(&f.writer, "remote.txt", "remote work\n");
    fetch(&f, Some("main"));
    let preview = f
        .app
        .preview_remote_switch(&f.id, "origin", "main")
        .unwrap();
    assert!(preview
        .switch
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("diverged"));
    assert_ne!(
        f.app
            .switch_remote_branch(&f.id, "origin", "main", &preview.switch.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), local);
    git(&f.writer, &["branch", "feature"]);
    fetch(&f, None);
    let preview = f
        .app
        .preview_remote_switch(&f.id, "origin", "feature")
        .unwrap();
    fs::write(f.repo.join("file.txt"), "unsaved work\n").unwrap();
    assert_ne!(
        f.app
            .switch_remote_branch(&f.id, "origin", "feature", &preview.switch.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        fs::read_to_string(f.repo.join("file.txt")).unwrap(),
        "unsaved work\n"
    );
    git(&f.repo, &["restore", "file.txt"]);
    git(&f.repo, &["branch", "feature", "origin/feature"]);
    let linked = f._dir.path().join("linked");
    git(
        &f.repo,
        &["worktree", "add", linked.to_str().unwrap(), "feature"],
    );
    assert!(f
        .app
        .preview_remote_switch(&f.id, "origin", "feature")
        .unwrap()
        .switch
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("another worktree"));
}
#[test]
fn moved_remote_or_tracking_config_invalidates_review_and_upstream_collisions_block() {
    let f = fixture();
    git(&f.writer, &["switch", "-c", "feature"]);
    commit(&f.writer, "file.txt", "feature one\n");
    fetch(&f, None);
    let preview = f
        .app
        .preview_remote_switch(&f.id, "origin", "feature")
        .unwrap();
    commit(&f.writer, "file.txt", "feature two\n");
    fetch(&f, Some("feature"));
    assert_ne!(
        f.app
            .switch_remote_branch(&f.id, "origin", "feature", &preview.switch.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.repo, &["branch", "--show-current"]), "main");
    let preview = f
        .app
        .preview_remote_switch(&f.id, "origin", "feature")
        .unwrap();
    git(
        &f.repo,
        &[
            "config",
            "remote.origin.pushurl",
            "https://example.invalid/changed",
        ],
    );
    assert_ne!(
        f.app
            .switch_remote_branch(&f.id, "origin", "feature", &preview.switch.token)
            .unwrap()
            .status,
        "succeeded"
    );
    git(&f.repo, &["branch", "feature", "main"]);
    git(
        &f.repo,
        &["branch", "--set-upstream-to=origin/main", "feature"],
    );
    assert!(f
        .app
        .preview_remote_switch(&f.id, "origin", "feature")
        .unwrap()
        .switch
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("already tracks"));
}
#[test]
fn failed_fetch_reports_recovery_and_does_not_integrate_or_erase_work() {
    let f = fixture();
    let before = git(&f.repo, &["rev-parse", "HEAD"]);
    fs::write(f.repo.join("file.txt"), "unsaved\n").unwrap();
    git(
        &f.repo,
        &[
            "remote",
            "set-url",
            "origin",
            f._dir.path().join("missing").to_str().unwrap(),
        ],
    );
    let receipt = f.app.fetch_remote(&f.id, "origin", Some("main")).unwrap();
    assert_ne!(receipt.status, "succeeded");
    assert!(receipt.recovery.unwrap().contains("partially"));
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        fs::read_to_string(f.repo.join("file.txt")).unwrap(),
        "unsaved\n"
    );
    assert!(f
        .app
        .preview_remote_switch(&f.id, "origin", "--discard-changes")
        .is_err());
    assert_ne!(
        f.app
            .fetch_remote(&f.id, "https://example.invalid/arbitrary", None)
            .unwrap()
            .status,
        "succeeded"
    );
}

#[cfg(unix)]
#[test]
fn configured_custom_transport_is_not_executed_by_fetch() {
    let f = fixture();
    let marker = f._dir.path().join("helper-must-not-run");
    git(&f.repo, &["config", "protocol.ext.allow", "always"]);
    git(
        &f.repo,
        &[
            "remote",
            "set-url",
            "origin",
            &format!("ext::touch {}", marker.display()),
        ],
    );
    let receipt = f.app.fetch_remote(&f.id, "origin", None).unwrap();
    assert_ne!(receipt.status, "succeeded");
    assert!(!marker.exists());
    assert!(receipt.summary.contains("not allowed"));
}

#[cfg(unix)]
#[test]
fn checkout_hook_moving_head_stops_before_fast_forward() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    git(&f.writer, &["switch", "-c", "feature"]);
    commit(&f.writer, "feature.txt", "first\n");
    fetch(&f, None);
    git(&f.repo, &["branch", "feature", "origin/feature"]);
    commit(&f.writer, "feature.txt", "second\n");
    fetch(&f, Some("feature"));
    let preview = f
        .app
        .preview_remote_switch(&f.id, "origin", "feature")
        .unwrap();
    let main = git(&f.repo, &["rev-parse", "main"]);
    let hook = f.repo.join(".git/hooks/post-checkout");
    fs::write(
        &hook,
        format!("#!/bin/sh\ngit update-ref refs/heads/feature {main}\n"),
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let receipt = f
        .app
        .switch_remote_branch(&f.id, "origin", "feature", &preview.switch.token)
        .unwrap();
    assert_eq!(receipt.status, "unknown");
    assert!(receipt.summary.contains("HEAD unexpectedly"));
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), main);
    assert_eq!(git(&f.repo, &["show", ":feature.txt"]), "first");
}

#[test]
fn direct_pull_preserves_current_branch_when_upstream_has_a_different_name() {
    let f = fixture();
    let before = git(&f.repo, &["rev-parse", "HEAD"]);
    git(&f.repo, &["branch", "release"]);
    git(&f.writer, &["switch", "-c", "release"]);
    let target = commit(&f.writer, "file.txt", "release update\n");
    git(
        &f.repo,
        &["config", "branch.main.merge", "refs/heads/release"],
    );
    // Source resolution works before the upstream has ever been fetched.
    let source = f.app.pull_upstream(&f.id).unwrap();
    assert_eq!(source.branch, "main");
    assert_eq!(source.remote_branch.as_deref(), Some("release"));
    fetch(&f, Some("release"));
    let review = f.app.preview_pull_current(&f.id).unwrap();
    assert_eq!(review.switch.branch, "main");
    assert_eq!(review.remote_branch, "release");
    assert_eq!((review.ahead, review.behind), (0, 1));
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), before);
    git(&f.repo, &["config", "branch.main.merge", "refs/heads/main"]);
    assert_eq!(
        f.app
            .pull_current(&f.id, &review.switch.token)
            .unwrap()
            .status,
        "failed"
    );
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), before);
    git(
        &f.repo,
        &["config", "branch.main.merge", "refs/heads/release"],
    );
    let receipt = f.app.pull_current(&f.id, &review.switch.token).unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(git(&f.repo, &["branch", "--show-current"]), "main");
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), target);
    assert_eq!(git(&f.repo, &["rev-parse", "release"]), before);
    assert_eq!(
        git(
            &f.repo,
            &["rev-parse", "--symbolic-full-name", "@{upstream}"]
        ),
        "refs/remotes/origin/release"
    );
}

#[test]
fn direct_pull_revalidates_dirty_state_and_blocks_divergence_and_missing_upstream() {
    let f = fixture();
    commit(&f.writer, "file.txt", "remote update\n");
    fetch(&f, Some("main"));
    let review = f.app.preview_pull_current(&f.id).unwrap();
    let original = git(&f.repo, &["rev-parse", "HEAD"]);
    fs::write(f.repo.join("file.txt"), "uncommitted\n").unwrap();
    let receipt = f.app.pull_current(&f.id, &review.switch.token).unwrap();
    assert_eq!(receipt.status, "failed");
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(
        fs::read_to_string(f.repo.join("file.txt")).unwrap(),
        "uncommitted\n"
    );
    commit(&f.repo, "file.txt", "local commit\n");
    let local = git(&f.repo, &["rev-parse", "HEAD"]);
    let diverged = f.app.preview_pull_current(&f.id).unwrap();
    assert_eq!((diverged.ahead, diverged.behind), (1, 1));
    assert_eq!(diverged.remote_oid, git(&f.writer, &["rev-parse", "HEAD"]));
    assert_ne!(diverged.remote_oid, local);
    assert_eq!(diverged.switch.target_oid, local);
    assert!(diverged
        .switch
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("diverged"));
    assert_eq!(
        f.app
            .pull_current(&f.id, &diverged.switch.token)
            .unwrap()
            .status,
        "failed"
    );
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), local);
    git(&f.repo, &["branch", "--unset-upstream"]);
    assert!(f.app.pull_upstream(&f.id).is_err());
    assert!(f.app.preview_pull_current(&f.id).is_err());
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), local);
}
