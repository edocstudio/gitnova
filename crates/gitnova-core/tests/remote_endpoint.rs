use gitnova_core::Application;
use gitnova_domain::{NativePath, RemoteEndpointRequest};
use std::{fs, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim_end().into()
}
fn fixture() -> (tempfile::TempDir, Application, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "Fixture"]);
    git(root, &["config", "user.email", "fixture@example.invalid"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    fs::write(root.join("file.txt"), "original\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "Initial"]);
    git(
        root,
        &[
            "remote",
            "add",
            "origin",
            "ssh://git@old.example:2424/team/project.git",
        ],
    );
    let app = Application::new(root.join("state/catalog.json")).unwrap();
    let id = app.add_repository(root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    (dir, app, id)
}
fn changed() -> RemoteEndpointRequest {
    RemoteEndpointRequest {
        name: "origin".into(),
        urls: vec!["https://new.example/team/project.git".into()],
        push_urls: vec![],
    }
}

#[test]
fn migration_preserves_files_index_refs_upstream_other_remotes_and_records_recovery() {
    let (dir, app, id) = fixture();
    let root = dir.path();
    git(
        root,
        &[
            "remote",
            "add",
            "other",
            "https://other.example/team/project.git",
        ],
    );
    git(
        root,
        &[
            "config",
            "remote.origin.pushurl",
            "ssh://git@old-push.example/team/project.git",
        ],
    );
    git(
        root,
        &[
            "config",
            "remote.origin.push",
            "refs/heads/main:refs/heads/release",
        ],
    );
    git(root, &["config", "branch.main.remote", "origin"]);
    git(root, &["config", "branch.main.merge", "refs/heads/main"]);
    git(root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    fs::write(root.join("file.txt"), "staged\n").unwrap();
    git(root, &["add", "file.txt"]);
    fs::write(root.join("file.txt"), "working\n").unwrap();
    let config = fs::read(root.join(".git/config")).unwrap();
    let index = fs::read(root.join(".git/index")).unwrap();
    let refs = git(root, &["show-ref"]);
    let settings = app.remote_endpoint(&id, "origin").unwrap();
    let preview = app
        .preview_remote_endpoint(&id, &changed(), &settings.token)
        .unwrap();
    assert_eq!(fs::read(root.join(".git/config")).unwrap(), config);
    let receipt = app
        .change_remote_endpoint(&id, &preview.request, &settings.token, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(
        git(root, &["remote", "get-url", "origin"]),
        changed().urls[0]
    );
    assert_eq!(
        git(root, &["remote", "get-url", "--push", "origin"]),
        changed().urls[0]
    );
    assert_eq!(
        git(root, &["remote", "get-url", "other"]),
        "https://other.example/team/project.git"
    );
    assert_eq!(git(root, &["config", "branch.main.remote"]), "origin");
    assert_eq!(
        git(root, &["config", "branch.main.merge"]),
        "refs/heads/main"
    );
    assert_eq!(
        git(root, &["config", "remote.origin.push"]),
        "refs/heads/main:refs/heads/release"
    );
    assert_eq!(
        git(root, &["config", "remote.origin.fetch"]),
        "+refs/heads/*:refs/remotes/origin/*"
    );
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(root.join("file.txt")).unwrap(),
        "working\n"
    );
    assert_eq!(git(root, &["show-ref"]), refs);
    assert_eq!(
        fs::read(preview.recovery_path.to_path().unwrap()).unwrap(),
        config
    );
    let activity = app.recorded_activity(&id).unwrap();
    assert!(activity
        .entries
        .iter()
        .any(|entry| entry.kind == "change_remote_endpoint"
            && entry.recovery_dir == Some(preview.recovery_path.clone())));
}

#[test]
fn separate_push_urls_and_multiple_endpoints_keep_reviewed_order() {
    let (dir, app, id) = fixture();
    let root = dir.path();
    let request = RemoteEndpointRequest {
        name: "origin".into(),
        urls: vec![
            "https://new.example/a.git".into(),
            "ssh://git@mirror.example/a.git".into(),
        ],
        push_urls: vec![
            "ssh://git@write.example/a.git".into(),
            "ssh://git@backup.example/a.git".into(),
        ],
    };
    let settings = app.remote_endpoint(&id, "origin").unwrap();
    let preview = app
        .preview_remote_endpoint(&id, &request, &settings.token)
        .unwrap();
    assert_eq!(
        app.change_remote_endpoint(&id, &request, &settings.token, &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(app.remote_endpoint(&id, "origin").unwrap().request, request);
    assert_eq!(
        git(root, &["remote", "get-url", "--push", "--all", "origin"]),
        request.push_urls.join("\n")
    );
}

#[test]
fn external_config_edits_invalidate_both_form_and_confirmation() {
    let (dir, app, id) = fixture();
    let root = dir.path();
    let settings = app.remote_endpoint(&id, "origin").unwrap();
    let preview = app
        .preview_remote_endpoint(&id, &changed(), &settings.token)
        .unwrap();
    git(
        root,
        &[
            "remote",
            "set-url",
            "origin",
            "https://external.example/a.git",
        ],
    );
    let original = fs::read(root.join(".git/config")).unwrap();
    assert_eq!(
        app.preview_remote_endpoint(&id, &changed(), &settings.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    assert_eq!(
        app.change_remote_endpoint(&id, &changed(), &settings.token, &preview.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    assert_eq!(fs::read(root.join(".git/config")).unwrap(), original);
}

#[test]
fn existing_config_lock_prevents_migration_without_removing_the_lock() {
    let (dir, app, id) = fixture();
    let root = dir.path();
    let settings = app.remote_endpoint(&id, "origin").unwrap();
    let preview = app
        .preview_remote_endpoint(&id, &changed(), &settings.token)
        .unwrap();
    let original = fs::read(root.join(".git/config")).unwrap();
    fs::write(root.join(".git/config.lock"), "external lock").unwrap();
    let receipt = app
        .change_remote_endpoint(&id, &changed(), &settings.token, &preview.token)
        .unwrap();
    assert_ne!(receipt.status, "succeeded");
    assert_eq!(fs::read(root.join(".git/config")).unwrap(), original);
    assert_eq!(
        fs::read_to_string(root.join(".git/config.lock")).unwrap(),
        "external lock"
    );
}

#[test]
fn rejects_unsafe_urls_and_inherited_endpoint_configuration() {
    let (dir, app, id) = fixture();
    let root = dir.path();
    let settings = app.remote_endpoint(&id, "origin").unwrap();
    for url in [
        "ext::touch bad",
        "https://token@host.example/a.git",
        "-x",
        "https://host.example/a.git\ninvalid",
    ] {
        let mut request = changed();
        request.urls = vec![url.into()];
        assert!(app
            .preview_remote_endpoint(&id, &request, &settings.token)
            .is_err());
    }
    let included = root.join("extra.config");
    fs::write(
        &included,
        "[remote \"origin\"]\nurl = https://included.example/a.git\n",
    )
    .unwrap();
    git(
        root,
        &["config", "include.path", included.to_str().unwrap()],
    );
    assert_eq!(
        app.remote_endpoint(&id, "origin").unwrap_err().code,
        "remote_config"
    );
}

#[test]
fn linked_worktrees_share_endpoint_changes_and_repository_identity() {
    let (dir, app, id) = fixture();
    let root = dir.path();
    let parent = tempfile::tempdir().unwrap();
    let child = parent.path().join("linked");
    git(
        root,
        &["worktree", "add", "-b", "feature", child.to_str().unwrap()],
    );
    let catalog = app.add_repository(&child).unwrap();
    let child_id = catalog.workspaces[0].selected_id.clone().unwrap();
    let settings = app.remote_endpoint(&child_id, "origin").unwrap();
    let preview = app
        .preview_remote_endpoint(&child_id, &changed(), &settings.token)
        .unwrap();
    assert_eq!(
        app.change_remote_endpoint(&child_id, &changed(), &settings.token, &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        app.remote_endpoint(&id, "origin").unwrap().request,
        changed()
    );
    assert_eq!(git(&child, &["branch", "--show-current"]), "feature");
    assert_eq!(git(root, &["branch", "--show-current"]), "main");
    assert_eq!(
        preview.recovery_path.encoding,
        NativePath::new(root).encoding
    );
}

#[test]
fn existing_credential_urls_are_not_returned_to_the_dialog() {
    let (dir, app, id) = fixture();
    for url in [
        "ssh://git:password@host.example/a.git",
        "https://host.example/a.git?token=private",
    ] {
        git(dir.path(), &["remote", "set-url", "origin", url]);
        let error = app.remote_endpoint(&id, "origin").unwrap_err();
        assert_eq!(error.code, "remote_config");
        assert!(!error.message.contains("password"));
        assert!(!error.message.contains("private"));
    }
}
