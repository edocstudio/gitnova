use gitnova_git::GitCli;
use std::{fs, path::Path, process::Command};
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
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
#[test]
fn historical_pipeline_scope_uses_fetch_destination_without_moving_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "CI Test"]);
    git(root, &["config", "user.email", "ci@example.invalid"]);
    git(root, &["config", "core.hooksPath", "/dev/null"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    fs::write(root.join("file"), "first").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "first"]);
    let old = git(root, &["rev-parse", "HEAD"]);
    fs::write(root.join("file"), "second").unwrap();
    git(root, &["commit", "-am", "second"]);
    let head = git(root, &["rev-parse", "HEAD"]);
    git(
        root,
        &[
            "remote",
            "add",
            "origin",
            "git@gitlab.com:team/read-project.git",
        ],
    );
    git(
        root,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "git@gitlab.com:team/write-project.git",
        ],
    );
    fs::write(root.join("file"), "private edit").unwrap();
    let index = fs::read(root.join(".git/index")).unwrap();

    let app = gitnova_core::Application::new(root.join("state/catalog.json")).unwrap();
    let catalog = app.add_repository(root).unwrap();
    let membership = catalog.workspaces[0].selected_id.as_ref().unwrap();
    let config = gitnova_domain::HostingSelection {
        remote: "origin".into(),
        branch: String::new(),
        remote_branch: None,
        oid: old.clone(),
        provider: Some("gitlab".into()),
        origin: None,
    };
    assert_eq!(
        app.commit_link(membership, &config).unwrap(),
        format!("https://gitlab.com/team/read-project/-/commit/{old}")
    );
    git(root, &["tag", "-a", "v1", "-m", "version"]);
    let tag_object = git(root, &["rev-parse", "refs/tags/v1"]);
    let jenkins = gitnova_domain::JenkinsSelection {
        remote: "missing".into(),
        oid: old.clone(),
        controller: "https://ci.example.com/".into(),
        job_url: "https://ci.example.com/job/app/".into(),
        username: "alice".into(),
    };
    assert_eq!(
        app.jenkins_builds(membership, &jenkins, false)
            .unwrap_err()
            .code,
        "remote"
    );
    let mut invalid = jenkins.clone();
    invalid.controller = "http://ci.example.com/".into();
    assert!(app.jenkins_builds(membership, &invalid, false).is_err());
    let reader = GitCli;
    assert!(reader.pipeline_remote(root, "origin", &tag_object).is_err());
    assert_eq!(
        reader.pipeline_remote(root, "origin", &old).unwrap(),
        "git@gitlab.com:team/read-project.git"
    );
    git(root, &["checkout", "--detach", &head]);
    assert!(reader.pipeline_remote(root, "origin", &old).is_ok());
    assert!(reader
        .pipeline_remote(root, "--upload-pack=x", &old)
        .is_err());
    assert!(reader
        .pipeline_remote(root, "origin", &"0".repeat(40))
        .is_err());
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        fs::read_to_string(root.join("file")).unwrap(),
        "private edit"
    );
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
}
