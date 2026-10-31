use gitnova_core::Application;
use gitnova_domain::CloneRequest;
use std::{fs, path::Path, process::Command};
fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Clone Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8_lossy(&result.stdout).trim().into()
}
fn source(parent: &Path) -> std::path::PathBuf {
    let source = parent.join("source ' ไทย");
    fs::create_dir(&source).unwrap();
    git(&source, &["init", "-b", "main"]);
    fs::write(source.join("file.txt"), "source content\n").unwrap();
    git(&source, &["add", "."]);
    git(&source, &["commit", "-m", "Clone fixture"]);
    source
}
fn request(app: &Application, source: &Path, destination: &Path) -> CloneRequest {
    CloneRequest {
        workspace_id: app.catalog().unwrap().active_workspace_id,
        source: source.to_str().unwrap().into(),
        destination: destination.to_str().unwrap().into(),
    }
}
#[test]
fn clone_opens_real_history_and_upstream_without_initializing_submodules() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = source(root);
    let child = root.join("child");
    fs::create_dir(&child).unwrap();
    git(&child, &["init", "-b", "main"]);
    git(&child, &["commit", "--allow-empty", "-m", "Child"]);
    git(
        &source,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child.to_str().unwrap(),
            "modules/child",
        ],
    );
    git(&source, &["commit", "-am", "Recorded submodule"]);
    let expected = git(&source, &["rev-parse", "HEAD"]);
    let app = Application::new(root.join("state/catalog.json")).unwrap();
    let dest = root.join("cloned project");
    let request = request(&app, &source, &dest);
    let preview = app.preview_clone(&request).unwrap();
    assert!(!dest.exists());
    assert!(app.catalog().unwrap().repositories.is_empty());
    let outcome = app
        .clone_repository(&preview.request, &preview.token)
        .unwrap();
    assert_eq!(outcome.status, "succeeded", "{}", outcome.summary);
    // A clone remains usable after its local source disappears: no alternates/shared objects.
    assert!(!dest.join(".git/objects/info/alternates").exists());
    fs::remove_dir_all(&source).unwrap();
    let catalog = outcome.catalog.unwrap();
    let membership = catalog.workspaces[0].selected_id.as_deref().unwrap();
    let page = app.graph(membership, None).unwrap();
    assert_eq!(page.rows[0].commit.oid, expected);
    assert_eq!(page.status.branch.as_deref(), Some("main"));
    assert!(!page.status.dirty);
    assert_eq!(
        fs::read_to_string(dest.join("file.txt")).unwrap(),
        "source content\n"
    );
    assert_eq!(
        git(&dest, &["rev-parse", "--symbolic-full-name", "@{upstream}"]),
        "refs/remotes/origin/main"
    );
    assert!(!dest.join("modules/child/.git").exists());
    assert!(!fs::read_to_string(dest.join(".git/config"))
        .unwrap()
        .contains("hooksPath"));
    let journal = app.recent_clones(&request.workspace_id).unwrap();
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0].status, "succeeded");
    assert!(journal[0].catalog.is_none());
}
#[test]
fn existing_destinations_stale_tokens_and_unsafe_sources_never_start_clone() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = source(root);
    let app = Application::new(root.join("state/catalog.json")).unwrap();
    let dest = root.join("destination");
    let mut request = request(&app, &source, &dest);
    let preview = app.preview_clone(&request).unwrap();
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("keep.txt"), "keep").unwrap();
    assert!(app.clone_repository(&request, &preview.token).is_err());
    assert_eq!(fs::read_to_string(dest.join("keep.txt")).unwrap(), "keep");
    assert!(!dest.join(".git").exists());
    request.destination = root.join("other").to_str().unwrap().into();
    assert!(app.clone_repository(&request, &preview.token).is_err());
    assert!(!root.join("other").exists());
    for source in [
        "ext::touch marker",
        "https://user:secret@example.invalid/repo",
        "https://example.invalid/repo?token=secret",
        "relative/repository",
        "-option",
    ] {
        request.source = source.into();
        assert!(app.preview_clone(&request).is_err());
    }
    assert!(app.recent_clones(&request.workspace_id).unwrap().is_empty());
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        request.source = source.to_str().unwrap().into();
        let link = root.join("link");
        symlink(root.join("missing"), &link).unwrap();
        request.destination = link.to_str().unwrap().into();
        assert!(app.preview_clone(&request).is_err());
        assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    }
}
#[test]
fn failed_and_interrupted_attempts_survive_restart_without_automatic_retry_or_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = root.join("not-a-repository");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("keep.txt"), "keep").unwrap();
    let state = root.join("state/catalog.json");
    let app = Application::new(state.clone()).unwrap();
    let dest = root.join("partial");
    let request = request(&app, &source, &dest);
    let preview = app.preview_clone(&request).unwrap();
    let outcome = app
        .clone_repository(&preview.request, &preview.token)
        .unwrap();
    assert_eq!(outcome.status, "failed");
    assert!(app.catalog().unwrap().repositories.is_empty());
    assert_eq!(fs::read_to_string(source.join("keep.txt")).unwrap(), "keep");
    let mut saved = outcome.clone();
    saved.status = "pending".into();
    fs::write(
        saved.journal.to_path().unwrap(),
        serde_json::to_vec(&saved).unwrap(),
    )
    .unwrap();
    drop(app);
    let restored = Application::new(state).unwrap();
    let attempts = restored.recent_clones(&request.workspace_id).unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].status, "pending");
    assert_eq!(attempts[0].destination, outcome.destination);
    assert!(restored.catalog().unwrap().repositories.is_empty());
    assert!(!dest.join(".git").exists());
}
#[test]
fn registration_uses_reviewed_workspace_even_if_active_workspace_changes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = source(root);
    let app = Application::new(root.join("state/catalog.json")).unwrap();
    let request = request(&app, &source, &root.join("workspace-clone"));
    let preview = app.preview_clone(&request).unwrap();
    let next = app
        .create_workspace("Other project")
        .unwrap()
        .active_workspace_id;
    let outcome = app
        .clone_repository(&preview.request, &preview.token)
        .unwrap();
    assert_eq!(outcome.status, "succeeded");
    let catalog = outcome.catalog.unwrap();
    assert_eq!(catalog.active_workspace_id, next);
    assert_eq!(
        catalog
            .workspaces
            .iter()
            .find(|w| w.id == request.workspace_id)
            .unwrap()
            .memberships
            .len(),
        1
    );
    assert!(catalog
        .workspaces
        .iter()
        .find(|w| w.id == next)
        .unwrap()
        .memberships
        .is_empty());
}
#[test]
fn empty_repository_clones_into_a_real_unborn_worktree() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = root.join("empty.git");
    git(
        root,
        &["init", "--bare", "-b", "main", source.to_str().unwrap()],
    );
    let app = Application::new(root.join("state/catalog.json")).unwrap();
    let request = request(&app, &source, &root.join("empty-checkout"));
    let preview = app.preview_clone(&request).unwrap();
    let outcome = app
        .clone_repository(&preview.request, &preview.token)
        .unwrap();
    assert_eq!(outcome.status, "succeeded", "{}", outcome.summary);
    let catalog = outcome.catalog.unwrap();
    let id = catalog.workspaces[0].selected_id.as_deref().unwrap();
    assert!(app.graph(id, None).unwrap().rows.is_empty());
    assert!(app.working_changes(id).unwrap().files.is_empty());
}
