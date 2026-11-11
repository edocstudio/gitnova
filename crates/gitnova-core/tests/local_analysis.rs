use gitnova_core::local_analysis::{authorize, revoke, LocalAnalysis};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    process::Command,
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Synthetic")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "Synthetic")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim_end().into()
}
fn call(core: &LocalAnalysis, tool: &str, args: Value) -> Value {
    core.call(
        tool,
        args,
        Instant::now() + Duration::from_secs(15),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}
#[test]
fn permissions_snapshots_bounds_and_real_history() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    let config = dir.path().join("policy.json");
    let core = LocalAnalysis::new(config.clone()).unwrap();
    assert_eq!(
        call(&core, "list_repositories", json!({}))["data"]["repositories"],
        json!([])
    );
    assert!(core
        .call(
            "get_status",
            json!({"repository_id":"other"}),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
    let id = authorize(&config, &root).unwrap();
    assert!(call(&core, "get_status", json!({"repository_id":id}))["head"].is_null());
    fs::write(root.join("file with space.txt"), "one\ntwo\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    let before_head = git(&root, &["rev-parse", "HEAD"]);
    let before_refs = git(&root, &["show-ref"]);
    let before_index = fs::read(root.join(".git/index")).unwrap();
    fs::write(
        root.join("file with space.txt"),
        "one\nthree\napi_key = secret-value\n",
    )
    .unwrap();
    fs::write(root.join("untracked.txt"), "NEVER_DISCLOSE_THIS").unwrap();
    assert_eq!(
        call(&core, "get_status", json!({"repository_id":id}))["data"]["untracked"],
        1
    );
    let diff = call(&core, "get_diff", json!({"repository_id":id})).to_string();
    assert!(diff.contains("three"));
    assert!(!diff.contains("secret-value"));
    assert!(!diff.contains("NEVER_DISCLOSE_THIS"));
    let blame = call(
        &core,
        "get_line_history",
        json!({"repository_id":id,"path":"file with space.txt","line_start":1,"line_end":2}),
    );
    assert_eq!(blame["data"]["hasLocalChanges"], true);
    assert_eq!(blame["data"]["lines"][1]["content"], "two");
    assert_eq!(
        call(
            &core,
            "search_commits",
            json!({"repository_id":id,"message":"initial"})
        )["data"]["commits"][0]["subject"],
        "initial"
    );
    for args in [
        json!({"repository_id":id,"path":"../outside"}),
        json!({"repository_id":id,"path":"C:\\secret"}),
        json!({"repository_id":id,"path":"/etc/passwd"}),
        json!({"repository_id":id,"path":".git/config"}),
        json!({"repository_id":id,"limit":101}),
        json!({"repository_id":id,"command":"push"}),
    ] {
        assert!(core
            .call(
                "get_diff",
                args,
                Instant::now() + Duration::from_secs(5),
                Arc::new(AtomicBool::new(false))
            )
            .is_err());
    }
    assert!(core
        .call(
            "search_commits",
            json!({"repository_id":id,"reference":"HEAD~1"}),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), before_head);
    assert_eq!(git(&root, &["show-ref"]), before_refs);
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), before_index);
    assert!(fs::read_to_string(root.join("file with space.txt"))
        .unwrap()
        .contains("three"));
    let cancelled = Arc::new(AtomicBool::new(true));
    assert!(core
        .call(
            "get_status",
            json!({"repository_id":id}),
            Instant::now() + Duration::from_secs(5),
            cancelled
        )
        .is_err());
    assert!(core
        .call(
            "get_status",
            json!({"repository_id":id}),
            Instant::now() - Duration::from_secs(1),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
    revoke(&config, &id).unwrap();
    assert!(core
        .call(
            "get_status",
            json!({"repository_id":id}),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
}
#[test]
fn worktree_submodule_nested_repo_conflicts_and_detached() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("parent");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    fs::write(root.join("f.txt"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);
    let cfg = dir.path().join("policy.json");
    let id = authorize(&cfg, &root).unwrap();
    let core = LocalAnalysis::new(cfg.clone()).unwrap();
    let work = dir.path().join("linked");
    git(
        &root,
        &["worktree", "add", "-b", "side", work.to_str().unwrap()],
    );
    let wid = authorize(&cfg, &work).unwrap();
    assert_ne!(id, wid);
    assert_eq!(
        call(&core, "get_status", json!({"repository_id":wid}))["data"]["branch"],
        "side"
    );
    git(&root, &["switch", "-c", "incoming"]);
    fs::write(root.join("f.txt"), "incoming\n").unwrap();
    git(&root, &["commit", "-am", "incoming"]);
    git(&root, &["switch", "main"]);
    fs::write(root.join("f.txt"), "current\n").unwrap();
    git(&root, &["commit", "-am", "current"]);
    let merge = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "merge",
            "incoming",
        ])
        .output()
        .unwrap();
    assert!(!merge.status.success());
    assert_eq!(
        call(&core, "get_conflict_summary", json!({"repository_id":id}))["data"]["paths"][0]
            ["path"],
        "f.txt"
    );
    git(&root, &["merge", "--abort"]);
    git(&root, &["checkout", "--detach"]);
    assert!(call(&core, "get_status", json!({"repository_id":id}))["data"]["branch"].is_null());
    let child = root.join("child");
    fs::create_dir(&child).unwrap();
    git(&child, &["init", "-b", "main"]);
    fs::write(child.join("code.txt"), "child\n").unwrap();
    git(&child, &["add", "."]);
    git(&child, &["commit", "-m", "child"]);
    let child_head = git(&child, &["rev-parse", "HEAD"]);
    git(
        &root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{child_head},child"),
        ],
    );
    assert!(core
        .call(
            "get_line_history",
            json!({"repository_id":id,"path":"child/code.txt"}),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
    assert_eq!(
        call(&core, "list_repositories", json!({}))["data"]["repositories"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path(), root.join("escape")).unwrap();
        assert!(core
            .call(
                "get_diff",
                json!({"repository_id":id,"path":"escape/policy.json"}),
                Instant::now() + Duration::from_secs(5),
                Arc::new(AtomicBool::new(false))
            )
            .is_err());
    }
    let cid = authorize(&cfg, &child).unwrap();
    assert_eq!(
        call(&core, "get_status", json!({"repository_id":cid}))["data"]["staged"],
        0
    );
}
#[test]
fn binary_and_large_patch_are_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    fs::write(root.join("large.txt"), "base\n").unwrap();
    fs::write(root.join("image.bin"), [0, 1, 2]).unwrap();
    fs::write(root.join("z-last.txt"), "before\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    fs::write(root.join("large.txt"), "long text content\n".repeat(20000)).unwrap();
    fs::write(root.join("image.bin"), [0, 3, 4]).unwrap();
    fs::write(root.join("z-last.txt"), "after\n").unwrap();
    let cfg = root.join(".git/policy.json");
    let id = authorize(&cfg, root).unwrap();
    let core = LocalAnalysis::new(cfg).unwrap();
    let diff = call(
        &core,
        "get_diff",
        json!({"repository_id":id,"path":"large.txt","max_bytes":1024}),
    );
    assert_eq!(diff["truncated"], true);
    assert!(diff.to_string().len() < 5000);
    let binary = call(
        &core,
        "get_diff",
        json!({"repository_id":id,"path":"image.bin"}),
    );
    assert!(binary.to_string().contains("Binary files"));
    let page = call(
        &core,
        "get_diff",
        json!({"repository_id":id,"max_bytes":1024}),
    );
    assert_eq!(page["truncated"], true);
    assert_eq!(page["nextOffset"], 2);
    let next = call(
        &core,
        "get_diff",
        json!({"repository_id":id,"offset":page["nextOffset"],"max_bytes":1024}),
    );
    assert_eq!(next["data"]["files"][0]["path"], "z-last.txt");
    assert!(next["nextOffset"].is_null());
}
#[test]
fn staged_pagination_special_names_and_read_filters() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    fs::write(root.join("-special [file].txt"), "old\n").unwrap();
    git(&root, &["add", "--", "-special [file].txt"]);
    git(&root, &["commit", "-m", "first"]);
    fs::write(root.join("-special [file].txt"), "staged\n").unwrap();
    git(&root, &["add", "--", "-special [file].txt"]);
    git(&root, &["commit", "-m", "second"]);
    let config = dir.path().join("policy.json");
    let id = authorize(&config, &root).unwrap();
    let core = LocalAnalysis::new(config.clone()).unwrap();
    let first = call(
        &core,
        "search_commits",
        json!({"repository_id":id,"reference":"refs/heads/main","limit":1}),
    );
    assert_eq!(first["nextOffset"], 1);
    assert_eq!(
        call(
            &core,
            "search_commits",
            json!({"repository_id":id,"limit":1,"offset":1})
        )["data"]["commits"][0]["subject"],
        "first"
    );
    fs::write(root.join("-special [file].txt"), "staged change\n").unwrap();
    git(&root, &["add", "--", "-special [file].txt"]);
    fs::write(root.join("-special [file].txt"), "local change\n").unwrap();
    assert!(call(
        &core,
        "get_diff",
        json!({"repository_id":id,"staged":true,"path":"-special [file].txt"})
    )
    .to_string()
    .contains("staged change"));
    // These commands would leave a marker if read inspection invoked configured code.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let trap = dir.path().join("trap");
        let marker = dir.path().join("UNSAFE");
        fs::write(
            &trap,
            format!("#!/bin/sh\ntouch '{}'\ncat\n", marker.display()),
        )
        .unwrap();
        fs::set_permissions(&trap, fs::Permissions::from_mode(0o700)).unwrap();
        git(
            &root,
            &["config", "filter.test.clean", trap.to_str().unwrap()],
        );
        git(&root, &["config", "filter.test.required", "true"]);
        git(
            &root,
            &["config", "diff.test.command", trap.to_str().unwrap()],
        );
        git(
            &root,
            &["config", "diff.test.textconv", trap.to_str().unwrap()],
        );
        git(&root, &["config", "core.fsmonitor", trap.to_str().unwrap()]);
        git(&root, &["config", "core.pager", trap.to_str().unwrap()]);
        fs::write(root.join(".gitattributes"), "*.txt filter=test diff=test\n").unwrap();
        call(&core, "get_status", json!({"repository_id":id}));
        call(&core, "get_diff", json!({"repository_id":id}));
        call(
            &core,
            "get_line_history",
            json!({"repository_id":id,"path":"-special [file].txt","line_start":1,"line_end":1}),
        );
        assert!(!marker.exists());
    }
    // Replacing an approved root with another Git repository invalidates its grant.
    fs::rename(&root, dir.path().join("old-root")).unwrap();
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    assert!(core
        .call(
            "get_status",
            json!({"repository_id":id}),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
}
#[test]
fn policy_limits_symlink_config_and_uninitialized_submodule() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    fs::write(root.join("f"), "base").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);
    let config = dir.path().join("policy.json");
    let id = authorize(&config, &root).unwrap();
    let core = LocalAnalysis::new(config.clone()).unwrap();
    let oid = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{oid},missing"),
        ],
    );
    assert!(core
        .call(
            "get_line_history",
            json!({"repository_id":id,"path":"missing/source.rs"}),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
    #[cfg(unix)]
    {
        let alias = dir.path().join("alias.json");
        std::os::unix::fs::symlink(&config, &alias).unwrap();
        let unsafe_core = LocalAnalysis::new(alias).unwrap();
        assert!(unsafe_core
            .call(
                "list_repositories",
                json!({}),
                Instant::now() + Duration::from_secs(5),
                Arc::new(AtomicBool::new(false))
            )
            .is_err());
    }
    fs::write(&config, vec![b'x'; 65537]).unwrap();
    assert!(core
        .call(
            "get_status",
            json!({"repository_id":id}),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
}
#[test]
fn private_key_ranges_are_not_exposed_and_dates_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    fs::write(
        root.join("document.txt"),
        "-----BEGIN PRIVATE KEY-----\nSENSITIVE-BODY\n-----END PRIVATE KEY-----\n",
    )
    .unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "first"]);
    let config = dir.path().join("policy.json");
    let id = authorize(&config, &root).unwrap();
    let core = LocalAnalysis::new(config).unwrap();
    assert!(core
        .call(
            "get_line_history",
            json!({"repository_id":id,"path":"document.txt","line_start":2,"line_end":2}),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
    fs::write(
        root.join("document.txt"),
        "-----BEGIN PRIVATE KEY-----\nCHANGED-SENSITIVE-BODY\n-----END PRIVATE KEY-----\n",
    )
    .unwrap();
    let d = call(&core, "get_diff", json!({"repository_id":id}));
    assert_eq!(d["truncated"], true);
    assert!(!d.to_string().contains("SENSITIVE-BODY"));
    assert!(core
        .call(
            "search_commits",
            json!({"repository_id":id,"since":50,"until":1}),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
    assert_eq!(
        call(
            &core,
            "search_commits",
            json!({"repository_id":id,"since":1,"author":"Synthetic"})
        )["data"]["commits"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
