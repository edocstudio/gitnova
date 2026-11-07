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
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    remote: PathBuf,
    app: Application,
    id: String,
    first: String,
    last: String,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let remote = dir.path().join("remote.git");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Tag Test"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    git(&root, &["config", "core.hooksPath", "/dev/null"]);
    fs::write(root.join("file.txt"), "first\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "First"]);
    let first = git(&root, &["rev-parse", "HEAD"]);
    fs::write(root.join("file.txt"), "second\n").unwrap();
    git(&root, &["commit", "-am", "Second"]);
    let last = git(&root, &["rev-parse", "HEAD"]);
    git(dir.path(), &["init", "--bare", remote.to_str().unwrap()]);
    git(
        &root,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    let app = Application::new(dir.path().join("state.json")).unwrap();
    let catalog = app.add_repository(&root).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    Fixture {
        _dir: dir,
        root,
        remote,
        app,
        id,
        first,
        last,
    }
}
fn apply(f: &Fixture, request: &TagRequest) -> OperationReceipt {
    let p = f.app.preview_tag(&f.id, request).unwrap();
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    f.app.change_tag(&f.id, request, &p.token).unwrap()
}
#[test]
fn annotated_tag_at_non_head_and_publication_preserve_annotation_index_and_other_refs() {
    let f = fixture();
    fs::write(f.root.join("file.txt"), "staged\n").unwrap();
    git(&f.root, &["add", "file.txt"]);
    fs::write(f.root.join("file.txt"), "unstaged\n").unwrap();
    let index = fs::read(f.root.join(".git/index")).unwrap();
    let create = TagRequest::Create {
        name: "v1.2.0".into(),
        oid: f.first.clone(),
        annotation: Some("Release ไทย\nSecond line".into()),
    };
    let p = f.app.preview_tag(&f.id, &create).unwrap();
    assert_eq!(git(&f.root, &["tag", "--list"]), "");
    assert_eq!(p.annotation.as_deref(), Some("Release ไทย\nSecond line"));
    assert_eq!(
        f.app.change_tag(&f.id, &create, &p.token).unwrap().status,
        "succeeded"
    );
    let object = git(&f.root, &["rev-parse", "refs/tags/v1.2.0"]);
    assert_ne!(object, f.first);
    assert_eq!(git(&f.root, &["rev-parse", "v1.2.0^{commit}"]), f.first);
    assert!(git(&f.root, &["cat-file", "-p", "v1.2.0"]).contains("Release ไทย\nSecond line"));
    git(&f.root, &["tag", "unpublished", &f.last]);
    let push = TagRequest::Push {
        name: "v1.2.0".into(),
        oid: f.first.clone(),
        remote: "origin".into(),
    };
    assert_eq!(apply(&f, &push).status, "succeeded");
    assert_eq!(git(&f.remote, &["rev-parse", "refs/tags/v1.2.0"]), object);
    assert_eq!(
        git(&f.remote, &["for-each-ref", "--format=%(refname)"]),
        "refs/tags/v1.2.0"
    );
    assert!(f.app.preview_tag(&f.id, &push).unwrap().already_published);
    assert_eq!(apply(&f, &push).status, "succeeded");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.last);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(f.root.join("file.txt")).unwrap(),
        "unstaged\n"
    );
}
#[test]
fn tag_duplicates_moved_sources_and_remote_conflicts_do_not_overwrite() {
    let f = fixture();
    let create = TagRequest::Create {
        name: "v1".into(),
        oid: f.first.clone(),
        annotation: None,
    };
    assert_eq!(apply(&f, &create).status, "succeeded");
    assert!(f
        .app
        .preview_tag(&f.id, &create)
        .unwrap()
        .blocked_reason
        .is_some());
    let push = TagRequest::Push {
        name: "v1".into(),
        oid: f.first.clone(),
        remote: "origin".into(),
    };
    let preview = f.app.preview_tag(&f.id, &push).unwrap();
    git(&f.root, &["tag", "-f", "v1", &f.last]);
    assert!(f.app.change_tag(&f.id, &push, &preview.token).is_err());
    assert_eq!(git(&f.remote, &["tag", "--list"]), "");
    git(&f.root, &["push", "origin", "refs/tags/v1"]);
    git(&f.root, &["tag", "-f", "v1", &f.first]);
    let conflict = f.app.preview_tag(&f.id, &push).unwrap();
    assert!(conflict
        .blocked_reason
        .unwrap()
        .contains("different object"));
    assert_eq!(git(&f.remote, &["rev-parse", "v1"]), f.last);
}

#[test]
fn tag_inventory_includes_packed_local_and_remote_only_tags_without_fetching() {
    let f = fixture();
    for index in 0..27 {
        git(&f.root, &["tag", &format!("v1.0.{index}"), &f.first]);
    }
    git(&f.root, &["tag", "-a", "v2", "-m", "Version two", &f.first]);
    git(&f.root, &["push", "origin", "refs/tags/v2"]);
    let remote_object = git(&f.remote, &["rev-parse", "refs/tags/v2"]);
    git(&f.root, &["tag", "-d", "v2"]);
    git(&f.root, &["pack-refs", "--all"]);
    let index = fs::read(f.root.join(".git/index")).unwrap();
    let local = f.app.tag_refs(&f.id, None).unwrap();
    assert_eq!(local.len(), 27);
    assert!(local.iter().all(|tag| tag.oid == f.first));
    let remote = f.app.tag_refs(&f.id, Some("origin")).unwrap();
    assert_eq!(remote.len(), 1);
    assert_eq!(remote[0].name, "refs/tags/v2");
    assert_eq!(remote[0].oid, remote_object);
    assert_eq!(git(&f.root, &["tag", "--list", "v2"]), "");
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.last);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert!(f.app.tag_refs(&f.id, Some("unknown")).is_err());
    // Publication names must be checked at the push URL, not the fetch URL.
    git(
        &f.root,
        &["remote", "set-url", "origin", "missing-fetch.git"],
    );
    git(
        &f.root,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            f.remote.to_str().unwrap(),
        ],
    );
    assert_eq!(
        f.app.tag_refs(&f.id, Some("origin")).unwrap()[0].name,
        "refs/tags/v2"
    );
}

#[test]
fn fetch_imports_remote_annotated_tag_into_warm_graph_without_checkout_changes() {
    let f = fixture();
    git(&f.root, &["push", "origin", "main"]);
    git(&f.root, &["tag", "-a", "0.0.35", "-m", "Release", &f.last]);
    git(&f.root, &["push", "origin", "refs/tags/0.0.35"]);
    git(&f.root, &["tag", "-d", "0.0.35"]);
    git(&f.root, &["config", "remote.origin.tagOpt", "--no-tags"]);
    fs::write(f.root.join("file.txt"), "staged\n").unwrap();
    git(&f.root, &["add", "file.txt"]);
    fs::write(f.root.join("file.txt"), "draft\n").unwrap();
    let index = fs::read(f.root.join(".git/index")).unwrap();
    let before = f.app.graph(&f.id, None).unwrap();
    assert!(!before.refs.iter().any(|r| r.name == "refs/tags/0.0.35"));
    assert_eq!(f.app.tag_refs(&f.id, Some("origin")).unwrap().len(), 1);
    assert_eq!(
        f.app.fetch_remote(&f.id, "origin", None).unwrap().status,
        "succeeded"
    );
    let after = f.app.graph(&f.id, None).unwrap();
    assert_ne!(before.fingerprint, after.fingerprint);
    assert!(after
        .refs
        .iter()
        .any(|r| r.name == "refs/tags/0.0.35" && r.oid == f.last));
    assert_eq!(after.status.head.as_deref(), Some(f.last.as_str()));
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(f.root.join("file.txt")).unwrap(),
        "draft\n"
    );
    assert_eq!(git(&f.root, &["cat-file", "-t", "0.0.35"]), "tag");
}

#[test]
fn conflicting_remote_tag_never_overwrites_local_tag_or_partially_updates_refs() {
    let f = fixture();
    git(&f.root, &["push", "origin", "main"]);
    git(&f.root, &["tag", "0.0.35", &f.last]);
    git(&f.root, &["push", "origin", "refs/tags/0.0.35"]);
    git(&f.root, &["tag", "-f", "0.0.35", &f.first]);
    git(&f.root, &["tag", "new-remote-tag", &f.last]);
    git(&f.root, &["push", "origin", "refs/tags/new-remote-tag"]);
    git(&f.root, &["tag", "-d", "new-remote-tag"]);
    let before = git(&f.root, &["show-ref"]);
    assert!(f.app.auto_fetch(&f.id).is_err());
    assert_eq!(git(&f.root, &["show-ref"]), before);
    assert_eq!(git(&f.root, &["rev-parse", "0.0.35"]), f.first);
    assert_eq!(git(&f.root, &["rev-parse", "HEAD"]), f.last);
}
