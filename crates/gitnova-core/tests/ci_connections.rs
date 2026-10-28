use gitnova_core::Application;
use gitnova_domain::*;
use std::{fs, path::Path, process::Command};
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=CI Test",
            "-c",
            "user.email=ci@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
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
fn global_connections_follow_fetch_hosts_across_repositories_without_git_writes() {
    let dir = tempfile::tempdir().unwrap();
    let app = Application::new(dir.path().join("state.json")).unwrap();
    let profiles = vec![
        HostProfile {
            provider: "gitlab".into(),
            origin: "https://git.a.example".into(),
        },
        HostProfile {
            provider: "github".into(),
            origin: "https://git.b.example".into(),
        },
    ];
    for (name, url, provider, origin) in [
        (
            "one",
            "git@git.a.example:group/one.git",
            "gitlab",
            "https://git.a.example",
        ),
        (
            "two",
            "ssh://git@git.a.example/group/two.git",
            "gitlab",
            "https://git.a.example",
        ),
        (
            "three",
            "https://git.b.example/org/three.git",
            "github",
            "https://git.b.example",
        ),
    ] {
        let path = dir.path().join(name);
        fs::create_dir(&path).unwrap();
        git(&path, &["init", "-b", "main"]);
        git(&path, &["commit", "--allow-empty", "-m", "CI base"]);
        git(&path, &["remote", "add", "origin", url]);
        git(
            &path,
            &[
                "remote",
                "set-url",
                "--push",
                "origin",
                "git@different.example:other/project.git",
            ],
        );
        let oid = git(&path, &["rev-parse", "HEAD"]);
        let before = fs::read(path.join(".git/config")).unwrap();
        let id = app.add_repository(&path).unwrap().workspaces[0]
            .selected_id
            .clone()
            .unwrap();
        let scope = app.ci_connection(&id, &oid, &profiles).unwrap().unwrap();
        assert_eq!(scope.provider.as_deref(), Some(provider));
        assert_eq!(scope.origin.as_deref(), Some(origin));
        assert_eq!(scope.remote, "origin");
        assert_eq!(scope.oid, oid);
        if provider == "gitlab" {
            let ambiguous = vec![
                profiles[0].clone(),
                HostProfile {
                    provider: "gitlab".into(),
                    origin: "https://git.a.example/another-instance".into(),
                },
            ];
            assert!(app.ci_connection(&id, &oid, &ambiguous).is_err());
        }

        git(
            &path,
            &[
                "remote",
                "set-url",
                "origin",
                "git@git.a.example.evil.invalid:group/project.git",
            ],
        );
        assert!(app.ci_connection(&id, &oid, &profiles).unwrap().is_none());
        assert!(app.ci_summary(&id, &scope).is_err()); // reject changed fetch host before credentials/network
        git(&path, &["remote", "set-url", "origin", url]);
        assert_eq!(fs::read(path.join(".git/config")).unwrap(), before);
        assert_eq!(git(&path, &["rev-parse", "HEAD"]), oid);
        assert!(git(&path, &["status", "--porcelain"]).is_empty());
    }
}
