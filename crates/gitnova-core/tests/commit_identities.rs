use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
fn git(root: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
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
        o.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap().trim_end().into()
}
fn profile(provider: &str, host: &str) -> HostProfile {
    HostProfile {
        provider: provider.into(),
        origin: format!("https://{host}"),
    }
}
fn identity(name: &str) -> GitCommitIdentity {
    GitCommitIdentity {
        name: name.into(),
        email: format!("{}@example.invalid", name.to_lowercase()),
    }
}
fn repo(dir: &Path, app: &Application, name: &str, url: &str) -> (PathBuf, String) {
    let root = dir.join(name);
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    for (k, v) in [
        ("user.name", "Fallback"),
        ("user.email", "fallback@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", ".git/no-hooks"),
    ] {
        git(&root, &["config", k, v]);
    }
    fs::write(root.join("file"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);
    git(&root, &["remote", "add", "origin", url]);
    let c = app.add_repository(&root).unwrap();
    (root, c.workspaces[0].selected_id.clone().unwrap())
}
fn commit(app: &Application, id: &str, root: &Path) {
    fs::write(root.join("file"), "new content\n").unwrap();
    git(root, &["add", "file"]);
    let p = app.commit_preview(id).unwrap();
    let r = app.create_commit(id, "Scoped author", &p.token).unwrap();
    assert_eq!(r.status, "succeeded", "{r:?}");
}
#[test]
fn independent_hosts_apply_to_author_and_committer_without_touching_git_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app/state.json");
    let app = Application::new(path.clone()).unwrap();
    app.set_connection_commit_identity(
        &profile("github", "public.example"),
        Some(identity("Personal")),
    )
    .unwrap();
    app.set_connection_commit_identity(&profile("gitlab", "work.example"), Some(identity("Work")))
        .unwrap();
    for (name, url, expected) in [
        (
            "personal",
            "git@public.example:owner/project.git",
            "Personal",
        ),
        ("work", "https://work.example/group/project.git", "Work"),
        (
            "unknown",
            "https://public.example.attacker.invalid/owner/project.git",
            "Fallback",
        ),
    ] {
        let (root, id) = repo(dir.path(), &app, name, url);
        if expected != "Fallback" {
            git(&root, &["config", "author.name", "Old Author"]);
            git(
                &root,
                &["config", "author.email", "old-author@example.invalid"],
            );
            git(&root, &["config", "committer.name", "Old Committer"]);
            git(
                &root,
                &["config", "committer.email", "old-committer@example.invalid"],
            );
        }
        let config = fs::read(root.join(".git/config")).unwrap();
        assert_eq!(
            app.commit_identity(&id).unwrap().identity,
            identity(expected)
        );
        commit(&app, &id, &root);
        let want = identity(expected);
        assert_eq!(
            git(&root, &["show", "-s", "--format=%an|%ae|%cn|%ce", "HEAD"]),
            format!("{}|{}|{}|{}", want.name, want.email, want.name, want.email)
        );
        assert_eq!(fs::read(root.join(".git/config")).unwrap(), config);
    }
    let restarted = Application::new(path).unwrap();
    assert_eq!(
        restarted
            .connection_commit_identity(&profile("github", "public.example"))
            .unwrap(),
        Some(identity("Personal"))
    );
}
#[test]
fn identity_and_remote_changes_invalidate_commit_reviews() {
    let dir = tempfile::tempdir().unwrap();
    let app = Application::new(dir.path().join("app/state.json")).unwrap();
    let p = profile("github", "public.example");
    app.set_connection_commit_identity(&p, Some(identity("One")))
        .unwrap();
    let (root, id) = repo(dir.path(), &app, "repo", "https://public.example/o/r.git");
    fs::write(root.join("file"), "stage me\n").unwrap();
    git(&root, &["add", "file"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    let review = app.commit_preview(&id).unwrap();
    app.set_connection_commit_identity(&p, Some(identity("Two")))
        .unwrap();
    assert_ne!(
        app.create_commit(&id, "Stale identity", &review.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    let review = app.commit_preview(&id).unwrap();
    git(
        &root,
        &[
            "remote",
            "set-url",
            "origin",
            "https://elsewhere.example/o/r.git",
        ],
    );
    assert_ne!(
        app.create_commit(&id, "Stale host", &review.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read_to_string(root.join("file")).unwrap(), "stage me\n");
}
#[test]
fn amend_retains_original_author_and_uses_connection_committer() {
    let dir = tempfile::tempdir().unwrap();
    let app = Application::new(dir.path().join("state.json")).unwrap();
    let (root, id) = repo(dir.path(), &app, "repo", "https://public.example/o/r.git");
    app.set_connection_commit_identity(
        &profile("github", "public.example"),
        Some(identity("Personal")),
    )
    .unwrap();
    let request = AmendRequest {
        oid: git(&root, &["rev-parse", "HEAD"]),
        message: "Correct message".into(),
    };
    let p = app.preview_amend(&id, &request).unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    let out = app.amend_commit(&id, &request, &p.token, false).unwrap();
    assert_eq!(out.status, "succeeded", "{out:?}");
    assert_eq!(
        git(&root, &["show", "-s", "--format=%an|%cn|%ce", "HEAD"]),
        "Fallback|Personal|personal@example.invalid"
    );
}
#[test]
fn malformed_profiles_are_rejected_and_clearing_identity_restores_git_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let app = Application::new(dir.path().join("state.json")).unwrap();
    let p = profile("github", "public.example");
    for bad in [
        GitCommitIdentity {
            name: "Injected\nname".into(),
            email: "test@example.invalid".into(),
        },
        GitCommitIdentity {
            name: "Valid".into(),
            email: "a@example.invalid\nuser.name=Other".into(),
        },
    ] {
        assert!(app.set_connection_commit_identity(&p, Some(bad)).is_err());
    }
    assert!(app
        .set_connection_commit_identity(
            &profile("github", "public.example/path?token=secret"),
            Some(identity("Personal"))
        )
        .is_err());
    let (root, id) = repo(dir.path(), &app, "repo", "https://public.example/o/r.git");
    app.set_connection_commit_identity(&p, Some(identity("Personal")))
        .unwrap();
    app.set_connection_commit_identity(&p, None).unwrap();
    assert_eq!(
        app.commit_identity(&id).unwrap().identity,
        identity("Fallback")
    );
    assert_eq!(git(&root, &["config", "user.name"]), "Fallback");
}
#[test]
fn simultaneous_connections_do_not_share_thread_identity() {
    let dir = tempfile::tempdir().unwrap();
    let app = Arc::new(Application::new(dir.path().join("state.json")).unwrap());
    let mut fixtures = Vec::new();
    for (name, host) in [("One", "one.example"), ("Two", "two.example")] {
        app.set_connection_commit_identity(&profile("github", host), Some(identity(name)))
            .unwrap();
        let (root, id) = repo(dir.path(), &app, name, &format!("https://{host}/o/r.git"));
        fixtures.push((root, id, name));
    }
    let threads: Vec<_> = fixtures
        .into_iter()
        .map(|(root, id, name)| {
            let app = app.clone();
            std::thread::spawn(move || {
                commit(&app, &id, &root);
                assert_eq!(git(&root, &["show", "-s", "--format=%an", "HEAD"]), name);
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn merge_cherry_pick_and_rebase_share_committer_policy_and_preserve_replayed_authors() {
    for operation in ["merge", "cherryPick", "rebase"] {
        let dir = tempfile::tempdir().unwrap();
        let app = Application::new(dir.path().join("state.json")).unwrap();
        let (root, id) = repo(dir.path(), &app, "repo", "https://public.example/o/r.git");
        git(&root, &["checkout", "-b", "feature"]);
        fs::write(root.join("feature"), "incoming\n").unwrap();
        git(&root, &["add", "feature"]);
        git(
            &root,
            &[
                "commit",
                "--author",
                "Original <original@example.invalid>",
                "-m",
                "feature",
            ],
        );
        let feature = git(&root, &["rev-parse", "HEAD"]);
        git(&root, &["checkout", "main"]);
        fs::write(root.join("main-only"), "target\n").unwrap();
        git(&root, &["add", "main-only"]);
        git(&root, &["commit", "-m", "target"]);
        let main = git(&root, &["rev-parse", "HEAD"]);
        app.set_connection_commit_identity(
            &profile("github", "public.example"),
            Some(identity("Personal")),
        )
        .unwrap();
        let source = if operation == "rebase" {
            git(&root, &["checkout", "feature"]);
            main
        } else {
            feature
        };
        let request = MergeRequest {
            rebase_steps: vec![],
            acknowledge_rewrite: operation == "rebase",
            delete_source_branch: false,
            method: if operation == "merge" {
                MergeMethod::MergeCommit
            } else {
                MergeMethod::Auto
            },
            operation: operation.into(),
            mainline: None,
            source_oid: source,
            source_ref: Some(
                if operation == "rebase" {
                    "refs/heads/main"
                } else {
                    "refs/heads/feature"
                }
                .into(),
            ),
            session_id: String::new(),
        };
        let p = app.preview_merge(&id, &request).unwrap();
        assert!(p.blocked_reason.is_none(), "{operation}: {p:?}");
        let r = app.begin_merge(&id, &p.request, &p.token).unwrap();
        assert_eq!(
            r.status,
            if operation == "rebase" {
                "succeeded"
            } else {
                "pending"
            },
            "{operation}: {r:?}"
        );
        assert_eq!(
            app.snapshot(&id).unwrap().merge.token,
            app.merge_state(&id).unwrap().token
        );
        if operation != "rebase" {
            let finish = MergeAction {
                kind: "finish".into(),
                path: None,
                choice: None,
                content: None,
                message: "Finish scoped operation".into(),
            };
            let review = app.preview_merge_action(&id, &finish).unwrap();
            assert!(review.blocked_reason.is_none(), "{review:?}");
            let r = app.apply_merge_action(&id, &finish, &review.token).unwrap();
            assert_eq!(r.status, "succeeded", "{r:?}");
        }
        assert_eq!(
            git(&root, &["show", "-s", "--format=%cn|%ce", "HEAD"]),
            "Personal|personal@example.invalid",
            "{operation}"
        );
        assert_eq!(
            git(&root, &["show", "-s", "--format=%an", "HEAD"]),
            if operation == "merge" {
                "Personal"
            } else {
                "Original"
            },
            "{operation}"
        );
        assert_eq!(git(&root, &["config", "user.name"]), "Fallback");
    }
}

#[test]
fn linked_worktree_uses_its_own_scoped_identity_without_moving_main() {
    let dir = tempfile::tempdir().unwrap();
    let app = Application::new(dir.path().join("state.json")).unwrap();
    let (root, _) = repo(dir.path(), &app, "repo", "https://public.example/o/r.git");
    let original = git(&root, &["rev-parse", "HEAD"]);
    let linked = dir.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );
    let id = app.add_repository(&linked).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    app.set_connection_commit_identity(
        &profile("github", "public.example"),
        Some(identity("Personal")),
    )
    .unwrap();
    commit(&app, &id, &linked);
    assert_eq!(
        git(&linked, &["show", "-s", "--format=%an|%cn", "HEAD"]),
        "Personal|Personal"
    );
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), original);
    assert_eq!(git(&root, &["config", "user.name"]), "Fallback");
}

#[test]
fn identity_storage_bounds_reads_and_rejects_oversized_saves_before_replacing_good_data() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("state.json");
    let app = Application::new(data.clone()).unwrap();
    let mut saved = Vec::new();
    let mut refused = false;
    for i in 0..20 {
        let p = HostProfile {
            provider: "gitlab".into(),
            origin: format!("https://host{i}.example/{}", "x".repeat(3000)),
        };
        if app
            .set_connection_commit_identity(&p, Some(identity("Personal")))
            .is_err()
        {
            assert!(app.connection_commit_identity(&p).unwrap().is_none());
            refused = true;
            break;
        }
        saved.push(p);
    }
    assert!(refused && !saved.is_empty());
    let file = dir.path().join("commit-identities.json");
    assert!(fs::metadata(&file).unwrap().len() <= 32768);
    let restarted = Application::new(data.clone()).unwrap();
    for p in saved {
        assert_eq!(
            restarted.connection_commit_identity(&p).unwrap(),
            Some(identity("Personal"))
        );
    }
    fs::write(file, vec![b' '; 32769]).unwrap();
    assert!(Application::new(data).is_err());
}
