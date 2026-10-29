use gitnova_core::Application;
use gitnova_domain::InitRequest;
use std::{fs, path::Path, process::Command};
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
#[test]
fn initializes_new_and_existing_folders_without_staging_or_committing() {
    for existing in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let app = Application::new(temp.path().join("app/catalog.json")).unwrap();
        let dest = temp.path().join("project");
        if existing {
            fs::create_dir(&dest).unwrap();
            fs::write(dest.join("keep.txt"), "my work").unwrap();
        }
        let request = InitRequest {
            workspace_id: app.catalog().unwrap().active_workspace_id,
            destination: dest.to_str().unwrap().into(),
            branch: "develop".into(),
        };
        let preview = app.preview_init(&request).unwrap();
        assert_eq!(preview.existing_folder, existing);
        assert!(!dest.join(".git").exists());
        let catalog = app.init_repository(&request, &preview.token).unwrap();
        assert!(catalog.workspaces.iter().any(|w| w.selected_id.is_some()));
        assert_eq!(git(&dest, &["symbolic-ref", "--short", "HEAD"]), "develop");
        assert!(git(&dest, &["ls-files"]).is_empty());
        assert!(git(&dest, &["remote"]).is_empty());
        assert!(!dest.join(".git/hooks").exists());
        assert!(app.preview_init(&request).is_err());
        if existing {
            assert_eq!(
                fs::read_to_string(dest.join("keep.txt")).unwrap(),
                "my work"
            );
        }
    }
}
#[test]
fn rejects_invalid_branches_nested_repositories_and_stale_destinations() {
    let temp = tempfile::tempdir().unwrap();
    let app = Application::new(temp.path().join("app/catalog.json")).unwrap();
    let dest = temp.path().join("project");
    let mut request = InitRequest {
        workspace_id: app.catalog().unwrap().active_workspace_id,
        destination: dest.to_str().unwrap().into(),
        branch: "bad..branch".into(),
    };
    assert!(app.preview_init(&request).is_err());
    request.branch = "HEAD".into();
    assert!(app.preview_init(&request).is_err());
    request.branch = "main".into();
    let preview = app.preview_init(&request).unwrap();
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("keep.txt"), "preserved").unwrap();
    assert!(app.init_repository(&request, &preview.token).is_err());
    assert!(!dest.join(".git").exists());
    let preview = app.preview_init(&request).unwrap();
    app.init_repository(&request, &preview.token).unwrap();
    request.destination = dest.join("nested").to_str().unwrap().into();
    assert!(app.preview_init(&request).is_err());
    request.destination = dest.to_str().unwrap().into();
    assert!(app.init_repository(&request, &preview.token).is_err());
    assert_eq!(
        fs::read_to_string(dest.join("keep.txt")).unwrap(),
        "preserved"
    );
}
#[test]
fn rejects_gitfiles_symlinks_and_unknown_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let app = Application::new(temp.path().join("app/catalog.json")).unwrap();
    let dest = temp.path().join("project");
    fs::create_dir(&dest).unwrap();
    let mut request = InitRequest {
        workspace_id: app.catalog().unwrap().active_workspace_id,
        destination: dest.to_str().unwrap().into(),
        branch: "main".into(),
    };
    fs::write(dest.join(".git"), "gitdir: missing").unwrap();
    assert!(app.preview_init(&request).is_err());
    fs::remove_file(dest.join(".git")).unwrap();
    #[cfg(unix)]
    {
        let link = temp.path().join("link");
        std::os::unix::fs::symlink(&dest, &link).unwrap();
        request.destination = link.to_str().unwrap().into();
        assert!(app.preview_init(&request).is_err());
    }
    request.destination = dest.to_str().unwrap().into();
    request.workspace_id = "missing".into();
    assert!(app.preview_init(&request).is_err());
}
