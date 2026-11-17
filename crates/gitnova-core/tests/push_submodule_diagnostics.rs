use gitnova_core::Application;
use gitnova_domain::PushRequest;
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
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn init(root: &Path) {
    fs::create_dir(root).unwrap();
    git(root, &["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Fixture"),
        ("user.email", "fixture@example.invalid"),
        ("commit.gpgsign", "false"),
    ] {
        git(root, &["config", key, value]);
    }
}
fn commit(root: &Path, value: &str) -> String {
    fs::write(root.join("file.txt"), value).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", value]);
    git(root, &["rev-parse", "HEAD"])
}
struct Fixture {
    _dir: tempfile::TempDir,
    app: Application,
    id: String,
    parent: PathBuf,
    source: PathBuf,
    old: String,
    current: String,
    middle: String,
    request: PushRequest,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    init(&source);
    commit(&source, "base");
    git(&source, &["checkout", "-b", "unpublished"]);
    let old = commit(&source, "unpublished old history");
    git(&source, &["checkout", "main"]);
    let current = commit(&source, "published current history");
    let remote = dir.path().join("child.git");
    fs::create_dir(&remote).unwrap();
    git(&remote, &["init", "--bare", "-b", "main"]);
    git(
        &source,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&source, &["push", "origin", "main"]);
    let parent = dir.path().join("parent");
    init(&parent);
    git(
        &parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            remote.to_str().unwrap(),
            "dpfunction",
        ],
    );
    git(&parent, &["commit", "-am", "Initial child"]);
    let target = dir.path().join("parent.git");
    fs::create_dir(&target).unwrap();
    git(&target, &["init", "--bare", "-b", "main"]);
    git(
        &parent,
        &["remote", "add", "origin", target.to_str().unwrap()],
    );
    git(&parent, &["push", "origin", "main"]);
    let child = parent.join("dpfunction");
    git(&child, &["fetch", source.to_str().unwrap(), &old]);
    git(&child, &["checkout", "--detach", &old]);
    git(&parent, &["add", "dpfunction"]);
    git(&parent, &["commit", "-m", "Historical child version"]);
    let middle = git(&parent, &["rev-parse", "HEAD"]);
    git(&child, &["checkout", "main"]);
    git(&parent, &["add", "dpfunction"]);
    git(&parent, &["commit", "-m", "Latest child already published"]);
    let request = PushRequest {
        branch: "main".into(),
        oid: git(&parent, &["rev-parse", "HEAD"]),
        remote: "origin".into(),
        remote_branch: "main".into(),
        set_upstream: false,
        buffered_https: false,
    };
    let app = Application::new(dir.path().join("state/catalog.json")).unwrap();
    let catalog = app.add_repository(&parent).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    Fixture {
        _dir: dir,
        app,
        id,
        parent,
        source,
        old,
        current,
        middle,
        request,
    }
}
#[test]
fn identifies_historical_commit_even_when_current_child_is_published_and_recheck_clears_it() {
    let f = fixture();
    let config = fs::read(f.parent.join(".git/config")).unwrap();
    let head = git(&f.parent, &["rev-parse", "HEAD"]);
    let preview = f.app.preview_push(&f.id, &f.request).unwrap();
    let report = preview.submodule_report.unwrap();
    assert!(report.complete);
    assert_eq!(report.issues.len(), 1);
    let issue = &report.issues[0];
    assert_eq!(issue.oid, f.old);
    assert_eq!(issue.parent_oid, f.middle);
    assert_eq!(issue.current_oid.as_deref(), Some(f.current.as_str()));
    assert!(issue.can_open);
    assert!(issue.object_available);
    assert_eq!(fs::read(f.parent.join(".git/config")).unwrap(), config);
    assert_eq!(git(&f.parent, &["rev-parse", "HEAD"]), head);
    git(
        &f.source,
        &[
            "push",
            "origin",
            &format!("{}:refs/heads/recovery/old", f.old),
        ],
    );
    git(&f.parent.join("dpfunction"), &["fetch", "origin"]);
    let report = f
        .app
        .preview_push(&f.id, &f.request)
        .unwrap()
        .submodule_report
        .unwrap();
    assert!(report.complete);
    assert!(report.issues.is_empty());
    // Publishing elsewhere does not update this checkout's cached evidence.
    git(
        &f.parent.join("dpfunction"),
        &["update-ref", "-d", "refs/remotes/origin/recovery/old"],
    );
    let report = f
        .app
        .preview_push(&f.id, &f.request)
        .unwrap()
        .submodule_report
        .unwrap();
    assert_eq!(report.issues[0].oid, f.old);
}
#[test]
fn missing_checkout_is_labelled_unknown_without_initializing_it() {
    let f = fixture();
    let child = f.parent.join("dpfunction");
    let archived = f._dir.path().join("archived-child");
    fs::rename(&child, &archived).unwrap();
    let report = f
        .app
        .preview_push(&f.id, &f.request)
        .unwrap()
        .submodule_report
        .unwrap();
    assert!(!report.complete);
    assert!(!report.issues.is_empty());
    assert!(report.issues.iter().all(|issue| !issue.can_open));
    assert!(!child.exists());
}
