use gitnova_core::Application;
use serde_json::to_value;
use std::{fs, process::Command};

#[test]
fn rename_preserves_workspace_identity_and_repositories_and_persists_case_changes() {
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("app/state.json");
    let app = Application::new(state.clone()).unwrap();
    let initial = app.create_workspace("code").unwrap();
    let id = initial.active_workspace_id;
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    assert!(Command::new("git")
        .args(["init", "-b", "main"])
        .arg(&repo)
        .output()
        .unwrap()
        .status
        .success());
    let before = app.add_repository(&repo).unwrap();
    let config = fs::read(repo.join(".git/config")).unwrap();
    let mut expected = before.clone();
    expected
        .workspaces
        .iter_mut()
        .find(|w| w.id == id)
        .unwrap()
        .name = "Code".into();
    let renamed = app.rename_workspace(&id, "  Code  ").unwrap();
    assert_eq!(to_value(&renamed).unwrap(), to_value(&expected).unwrap());
    assert_eq!(
        to_value(Application::new(state).unwrap().catalog().unwrap()).unwrap(),
        to_value(&expected).unwrap()
    );
    let other = app.create_workspace("Other").unwrap().active_workspace_id;
    let renamed = app.rename_workspace(&id, "โค้ด").unwrap();
    assert_eq!(renamed.active_workspace_id, other);
    assert_eq!(fs::read(repo.join(".git/config")).unwrap(), config);
}

#[test]
fn invalid_names_or_unknown_identity_leave_the_catalog_and_disk_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("state.json");
    let app = Application::new(state.clone()).unwrap();
    let before = app.catalog().unwrap();
    let original = fs::read(&state).unwrap();
    for name in ["".to_owned(), " \t\n".into(), "ก".repeat(81)] {
        assert!(app
            .rename_workspace(&before.active_workspace_id, &name)
            .is_err());
    }
    assert!(app.rename_workspace("missing", "Valid name").is_err());
    assert_eq!(
        to_value(app.catalog().unwrap()).unwrap(),
        to_value(&before).unwrap()
    );
    assert_eq!(fs::read(&state).unwrap(), original);
    let unicode = "ก".repeat(80);
    let renamed = app
        .rename_workspace(&before.active_workspace_id, &unicode)
        .unwrap();
    assert_eq!(
        renamed
            .workspaces
            .iter()
            .find(|w| w.id == before.active_workspace_id)
            .unwrap()
            .name,
        unicode
    );
}

#[test]
fn failed_persistence_keeps_the_old_name_in_memory_and_on_disk() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("app");
    let state = parent.join("state.json");
    let app = Application::new(state).unwrap();
    let before = app.catalog().unwrap();
    let backup = temp.path().join("app-backup");
    fs::rename(&parent, &backup).unwrap();
    fs::write(&parent, "blocks the catalog directory").unwrap();
    assert!(app
        .rename_workspace(&before.active_workspace_id, "New label")
        .is_err());
    assert_eq!(
        to_value(app.catalog().unwrap()).unwrap(),
        to_value(&before).unwrap()
    );
    assert_eq!(
        to_value(
            Application::new(backup.join("state.json"))
                .unwrap()
                .catalog()
                .unwrap()
        )
        .unwrap(),
        to_value(&before).unwrap()
    );
}
