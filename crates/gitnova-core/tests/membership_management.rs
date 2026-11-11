use gitnova_core::Application;
use std::{fs, path::Path, process::Command};

fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Workspace Test",
            "-c",
            "user.email=workspace@example.invalid",
            "-c",
            "commit.gpgsign=false",
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
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn repository(path: &Path) {
    fs::create_dir(path).unwrap();
    git(path, &["init", "-b", "main"]);
    fs::write(path.join("file.txt"), "base\n").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-m", "Base"]);
}
#[test]
fn rename_and_remove_are_workspace_scoped_and_persist_without_touching_git() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    repository(&repo);
    fs::write(repo.join("file.txt"), "staged\n").unwrap();
    git(&repo, &["add", "."]);
    fs::write(repo.join("file.txt"), "unstaged\n").unwrap();
    let state = temp.path().join("state.json");
    let app = Application::new(state.clone()).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let first = catalog.active_workspace_id;
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    let second = app.create_workspace("Second").unwrap().active_workspace_id;
    let shared = app.add_repository(&repo).unwrap();
    let other_id = shared.workspaces[1].selected_id.clone().unwrap();
    app.switch_workspace(&first).unwrap();
    let changed = app
        .update_membership(&id, " Friendly name ", repo.to_str().unwrap())
        .unwrap();
    assert_eq!(changed.workspaces[0].memberships[0].id, id);
    assert_eq!(changed.workspaces[0].memberships[0].name, "Friendly name");
    assert_eq!(changed.workspaces[1].memberships[0].name, "repo");
    assert!(app.remove_membership(&other_id).is_err());
    let removed = app.remove_membership(&id).unwrap();
    assert!(removed.workspaces[0].memberships.is_empty());
    assert!(removed.workspaces[0].selected_id.is_none());
    assert_eq!(removed.workspaces[1].memberships.len(), 1);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "unstaged\n"
    );
    let restored = Application::new(state).unwrap();
    restored.switch_workspace(&second).unwrap();
    assert_eq!(
        restored.path(&other_id).unwrap(),
        fs::canonicalize(repo).unwrap()
    );
}
#[test]
fn moved_entry_rebinds_fresh_context_and_invalid_destinations_do_not_change_catalog() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    repository(&repo);
    let app = Application::new(temp.path().join("state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let old_id = catalog.workspaces[0].selected_id.clone().unwrap();
    app.pin(&old_id).unwrap();
    let old_path = app.path(&old_id).unwrap();
    let moved = temp.path().join("moved");
    fs::rename(&repo, &moved).unwrap();
    let before = serde_json::to_value(app.catalog().unwrap()).unwrap();
    assert!(app
        .update_membership(&old_id, "Name", temp.path().to_str().unwrap())
        .is_err());
    assert_eq!(
        serde_json::to_value(app.catalog().unwrap()).unwrap(),
        before
    );
    let renamed = app
        .update_membership(&old_id, "Offline label", old_path.to_str().unwrap())
        .unwrap();
    assert_eq!(renamed.workspaces[0].memberships[0].name, "Offline label");
    fs::create_dir(moved.join("nested")).unwrap();
    assert!(app
        .update_membership(&old_id, "Name", moved.join("nested").to_str().unwrap())
        .is_err());
    let updated = app
        .update_membership(&old_id, "Moved repo", moved.to_str().unwrap())
        .unwrap();
    let entry = &updated.workspaces[0].memberships[0];
    assert_ne!(entry.id, old_id);
    assert!(entry.pinned);
    assert_eq!(
        updated.workspaces[0].selected_id.as_deref(),
        Some(entry.id.as_str())
    );
    assert_eq!(
        app.path(&entry.id).unwrap(),
        fs::canonicalize(&moved).unwrap()
    );
    assert!(app.path(&old_id).is_err());
    assert_eq!(git(&moved, &["status", "--porcelain"]), "");
}
#[test]
fn duplicate_rebinding_is_rejected_and_missing_entries_can_be_removed() {
    let temp = tempfile::tempdir().unwrap();
    let one = temp.path().join("one");
    repository(&one);
    let two = temp.path().join("two");
    repository(&two);
    let app = Application::new(temp.path().join("state.json")).unwrap();
    let first = app.add_repository(&one).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    let second = app.add_repository(&two).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    assert!(app
        .update_membership(&first, "Duplicate", two.to_str().unwrap())
        .is_err());
    assert!(app
        .update_membership(&first, "", one.to_str().unwrap())
        .is_err());
    assert!(app
        .update_membership(&first, "bad\nname", one.to_str().unwrap())
        .is_err());
    fs::remove_dir_all(&two).unwrap();
    let removed = app.remove_membership(&second).unwrap();
    assert_eq!(
        removed.workspaces[0].selected_id.as_deref(),
        Some(first.as_str())
    );
    assert!(app.remove_membership(&second).is_err());
}

#[test]
fn linked_worktree_relocation_and_bare_entries_keep_other_registrations_separate() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    repository(&repo);
    let linked = temp.path().join("linked");
    git(
        &repo,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    let app = Application::new(temp.path().join("state.json")).unwrap();
    let first = app.add_repository(&linked).unwrap();
    let first_workspace = first.active_workspace_id;
    let first_id = first.workspaces[0].selected_id.clone().unwrap();
    let old_path = app.path(&first_id).unwrap();
    app.create_workspace("Other").unwrap();
    let second = app.add_repository(&linked).unwrap();
    let other_id = second.workspaces[1].selected_id.clone().unwrap();
    app.switch_workspace(&first_workspace).unwrap();
    let moved = temp.path().join("moved-linked");
    git(
        &repo,
        &[
            "worktree",
            "move",
            linked.to_str().unwrap(),
            moved.to_str().unwrap(),
        ],
    );
    let updated = app
        .update_membership(&first_id, "Moved linked", moved.to_str().unwrap())
        .unwrap();
    let new_id = updated.workspaces[0].selected_id.as_ref().unwrap();
    assert_eq!(app.path(new_id).unwrap(), fs::canonicalize(&moved).unwrap());
    assert_eq!(app.path(&other_id).unwrap(), old_path);
    assert_eq!(updated.repositories.len(), 1);
    let parent = app.add_repository(&repo).unwrap();
    let parent_id = parent.workspaces[0].selected_id.as_ref().unwrap();
    assert!(app
        .update_membership(parent_id, "Duplicate", moved.to_str().unwrap())
        .is_err());
    let worktree_id = updated.workspaces[0].memberships[0].worktree_id.clone();
    let other_workspace = updated.workspaces[1].id.clone();
    app.switch_workspace(&other_workspace).unwrap();
    let reconnected = app
        .update_membership(&other_id, "Other linked", moved.to_str().unwrap())
        .unwrap();
    assert_eq!(
        reconnected.workspaces[1].memberships[0].worktree_id,
        worktree_id
    );
    app.create_workspace("Fresh registration").unwrap();
    let added_again = app.add_repository(&moved).unwrap();
    let fresh_id = added_again.workspaces[2].selected_id.as_ref().unwrap();
    assert_eq!(
        app.path(fresh_id).unwrap(),
        fs::canonicalize(&moved).unwrap()
    );
    assert_eq!(
        added_again.workspaces[2].memberships[0].worktree_id,
        worktree_id
    );
    assert_eq!(added_again.repositories.len(), 1);
    app.switch_workspace(&first_workspace).unwrap();
    let bare = temp.path().join("bare.git");
    fs::create_dir(&bare).unwrap();
    git(&bare, &["init", "--bare"]);
    let added = app.add_repository(&bare).unwrap();
    let bare_id = added.workspaces[0].selected_id.as_ref().unwrap();
    assert!(added.workspaces[0]
        .memberships
        .iter()
        .find(|entry| &entry.id == bare_id)
        .unwrap()
        .worktree_id
        .is_none());
    app.update_membership(bare_id, "Bare storage", bare.to_str().unwrap())
        .unwrap();
    app.remove_membership(bare_id).unwrap();
    assert!(bare.join("HEAD").exists());
    assert_eq!(git(&moved, &["branch", "--show-current"]), "side");
}
