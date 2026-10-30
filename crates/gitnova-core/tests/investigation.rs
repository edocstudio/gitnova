use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=",
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
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim_end().into()
}
fn repo(parent: &Path, name: &str) -> PathBuf {
    let path = parent.join(name);
    fs::create_dir(&path).unwrap();
    git(&path, &["init", "-b", "main"]);
    fs::write(path.join("code.txt"), "base\n").unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "base"]);
    git(&path, &["checkout", "-b", "develop"]);
    fs::write(path.join("code.txt"), "base\nneedle literal [x]\n").unwrap();
    git(&path, &["commit", "-am", "introduce phrase"]);
    git(&path, &["branch", "release", "main"]);
    path
}
fn request(app: &Application, mode: &str) -> InvestigationRequest {
    let c = app.catalog().unwrap();
    let w = &c.workspaces[0];
    InvestigationRequest {
        line_start: None,
        line_end: None,
        selection: CommitSearchRequest {
            workspace_id: w.id.clone(),
            current_membership_id: w.selected_id.clone().unwrap(),
            scope: "workspace".into(),
            selected_ids: vec![],
            query: String::new(),
            cursor: None,
        },
        mode: mode.into(),
        path: "code.txt".into(),
        needle: String::new(),
        source: "develop".into(),
        target: "release".into(),
    }
}
fn snapshot(path: &Path) -> (String, Vec<u8>, String, String) {
    (
        git(path, &["show-ref"]),
        {
            let index = PathBuf::from(git(path, &["rev-parse", "--git-path", "index"]));
            fs::read(if index.is_absolute() {
                index
            } else {
                path.join(index)
            })
            .unwrap()
        },
        git(path, &["status", "--porcelain=v1", "--untracked-files=all"]),
        git(path, &["symbolic-ref", "HEAD"]),
    )
}
#[test]
fn literal_file_code_history_and_cross_repository_comparison_preserve_work() {
    let t = tempfile::tempdir().unwrap();
    let a = repo(t.path(), "one");
    let b = repo(t.path(), "two");
    let app = Application::new(t.path().join("state/catalog.json")).unwrap();
    app.add_repository(&a).unwrap();
    app.add_repository(&b).unwrap();
    fs::write(a.join("code.txt"), "private unstaged content\n").unwrap();
    let before_a = snapshot(&a);
    let before_b = snapshot(&b);
    let mut r = request(&app, "history");
    r.needle = "needle literal [x]".into();
    let page = app.investigate_history(&r).unwrap();
    assert!(!page.partial, "{page:?}");
    assert_eq!(page.evidence.len(), 2);
    assert_eq!(page.hits.len(), 2, "{page:?}");
    assert!(page
        .hits
        .iter()
        .all(|h| h.commit.subject == "introduce phrase"));
    let location = &page.hits[0].locations[0];
    app.focus_commit(
        &r.selection.workspace_id,
        &location.membership_id,
        &page.hits[0].commit.oid,
    )
    .unwrap();
    r.mode = "compare".into();
    let page = app.investigate_history(&r).unwrap();
    assert!(!page.partial, "{page:?}");
    assert_eq!(page.hits.len(), 2);
    for e in page.evidence {
        assert_eq!(e.source_only, Some(1));
        assert_eq!(e.target_only, Some(0));
        assert_eq!(e.changed_files, Some(1));
        assert!(e.source_oid.is_some());
    }
    assert_eq!(snapshot(&a), before_a);
    assert_eq!(snapshot(&b), before_b);
}
#[test]
fn missing_branch_is_unknown_per_repository_and_ref_expressions_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let a = repo(t.path(), "one");
    let b = repo(t.path(), "two");
    git(&b, &["branch", "-D", "release"]);
    let app = Application::new(t.path().join("state/catalog.json")).unwrap();
    app.add_repository(&a).unwrap();
    app.add_repository(&b).unwrap();
    let mut r = request(&app, "compare");
    let p = app.investigate_history(&r).unwrap();
    assert!(p.partial);
    assert!(p.evidence.iter().any(|e| e.issue.is_some()));
    assert!(p.evidence.iter().any(|e| e.source_only == Some(1)));
    for bad in [
        "--all",
        "main~1",
        "main..develop",
        "refs/gitnova/recovery/anything",
        "main^{commit}",
    ] {
        r.source = bad.into();
        assert!(app.investigate_history(&r).is_err(), "{bad}");
    }
    r = request(&app, "history");
    r.path = "../outside".into();
    assert!(app.investigate_history(&r).is_err());
    r.path = ":(glob)**".into();
    assert!(app.investigate_history(&r).is_err());
}
#[test]
fn history_uses_literal_paths_and_excludes_recovery_only_commits() {
    let t = tempfile::tempdir().unwrap();
    let a = repo(t.path(), "one");
    let app = Application::new(t.path().join("state/catalog.json")).unwrap();
    app.add_repository(&a).unwrap();
    fs::write(a.join("[literal].txt"), "exact\n").unwrap();
    git(&a, &["add", "."]);
    git(&a, &["commit", "-m", "literal path"]);
    let mut r = request(&app, "history");
    r.path = "[literal].txt".into();
    let p = app.investigate_history(&r).unwrap();
    assert_eq!(p.hits.len(), 1, "{p:?}");
    assert_eq!(p.hits[0].commit.subject, "literal path");
    git(&a, &["checkout", "--orphan", "private-fixture"]);
    git(&a, &["rm", "-rf", "."]);
    fs::write(a.join("private.txt"), "recovery only\n").unwrap();
    git(&a, &["add", "."]);
    git(&a, &["commit", "-m", "recovery only"]);
    let oid = git(&a, &["rev-parse", "HEAD"]);
    git(&a, &["update-ref", "refs/gitnova/recovery/fixture", &oid]);
    git(&a, &["checkout", "develop"]);
    git(&a, &["branch", "-D", "private-fixture"]);
    r.path = "private.txt".into();
    assert!(app.investigate_history(&r).unwrap().hits.is_empty());
}

#[test]
fn shared_worktrees_keep_locations_and_shallow_coverage_is_never_complete() {
    let t = tempfile::tempdir().unwrap();
    let a = repo(t.path(), "one");
    let linked = t.path().join("linked");
    git(
        &a,
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().unwrap(),
            "HEAD",
        ],
    );
    let app = Application::new(t.path().join("state/catalog.json")).unwrap();
    app.add_repository(&a).unwrap();
    app.add_repository(&linked).unwrap();
    let mut r = request(&app, "history");
    r.needle = "needle literal [x]".into();
    let p = app.investigate_history(&r).unwrap();
    assert_eq!(p.hits.len(), 1);
    assert_eq!(p.hits[0].locations.len(), 2);
    assert!(!p.partial);
    let head = git(&a, &["rev-parse", "HEAD"]);
    fs::write(a.join(".git/shallow"), format!("{head}\n")).unwrap();
    let p = app.investigate_history(&r).unwrap();
    assert!(p.partial);
    assert!(p.evidence.iter().all(|e| e.shallow));
}

#[test]
fn follow_history_carries_each_historical_filename_and_never_changes_checkout() {
    let t = tempfile::tempdir().unwrap();
    let root = repo(t.path(), "follow");
    git(&root, &["mv", "code.txt", "renamed [literal].txt"]);
    git(&root, &["commit", "-m", "rename file"]);
    fs::write(root.join("renamed [literal].txt"), "changed after rename\n").unwrap();
    git(&root, &["commit", "-am", "edit renamed file"]);
    let app = Application::new(t.path().join("catalog.json")).unwrap();
    app.add_repository(&root).unwrap();
    let mut r = request(&app, "follow");
    r.path = "renamed [literal].txt".into();
    r.source = "HEAD".into();
    let before = snapshot(&root);
    let page = app.investigate_history(&r).unwrap();
    assert!(!page.partial, "{:?}", page.evidence);
    assert_eq!(page.hits.len(), 4);
    let old = page
        .hits
        .iter()
        .find(|h| h.commit.subject == "base")
        .unwrap();
    assert_eq!(old.historical_path.as_ref().unwrap().display, "code.txt");
    assert_eq!(
        page.hits[0].historical_path.as_ref().unwrap().display,
        "renamed [literal].txt"
    );
    assert_eq!(snapshot(&root), before);
    r.path = ".".into();
    let p = app.investigate_history(&r).unwrap();
    assert!(p.partial);
    assert!(p.hits.is_empty());
    r.path = "renamed [literal].txt".into();
    r.needle = "phrase".into();
    assert!(app.investigate_history(&r).is_err());
}

#[test]
fn release_dependency_evidence_uses_registered_child_identity_and_exact_gitlink_tips() {
    let t = tempfile::tempdir().unwrap();
    let child = repo(t.path(), "child-source");
    let parent = repo(t.path(), "parent");
    git(
        &parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child.to_str().unwrap(),
            "library",
        ],
    );
    git(&parent, &["commit", "-m", "record child"]);
    git(&parent, &["branch", "-f", "release", "HEAD"]);
    let checkout = parent.join("library");
    git(&checkout, &["checkout", "main"]);
    git(&parent, &["add", "library"]);
    git(&parent, &["commit", "-m", "pin older child"]);
    // source develop now intentionally rolls back the release pin.
    let app = Application::new(t.path().join("catalog.json")).unwrap();
    app.add_repository(&parent).unwrap();
    let mut r = request(&app, "compare");
    r.selection.scope = "current".into();
    let unknown = app.investigate_history(&r).unwrap();
    assert!(unknown.partial);
    assert_eq!(unknown.evidence[0].dependencies[0].relation, "unknown");
    app.add_repository(&checkout).unwrap();
    app.add_repository(&parent).unwrap();
    r = request(&app, "compare");
    r.selection.scope = "current".into();
    let before = snapshot(&parent);
    let child_before = snapshot(&checkout);
    let page = app.investigate_history(&r).unwrap();
    assert!(!page.partial, "{:?}", page.evidence);
    let dep = &page.evidence[0].dependencies[0];
    assert_eq!(dep.relation, "rollback");
    assert_eq!(dep.target_only, Some(1));
    assert!(dep.location.is_some());
    assert_eq!(snapshot(&parent), before);
    assert_eq!(snapshot(&checkout), child_before);
    r.source = "release".into();
    r.target = "develop".into();
    let page = app.investigate_history(&r).unwrap();
    assert_eq!(page.evidence[0].dependencies[0].relation, "forward");
    git(
        &parent,
        &[
            "config",
            "--file",
            ".gitmodules",
            "submodule.library.url",
            "https://example.invalid/another.git",
        ],
    );
    let page = app.investigate_history(&r).unwrap();
    assert!(page.partial);
    assert_eq!(page.evidence[0].dependencies[0].relation, "unknown");
}

#[test]
fn committed_line_blame_follows_original_filenames_and_ignores_dirty_content() {
    let t = tempfile::tempdir().unwrap();
    let root = repo(t.path(), "blame");
    let base = git(&root, &["rev-parse", "main"]);
    let phrase = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["mv", "code.txt", "renamed file.txt"]);
    git(&root, &["commit", "-m", "rename"]);
    let tip = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["config", "blame.ignoreRevsFile", "missing-ignore-file"],
    );
    fs::write(root.join("renamed file.txt"), "private working edit\n").unwrap();
    let app = Application::new(t.path().join("state/catalog.json")).unwrap();
    app.add_repository(&root).unwrap();
    let mut r = request(&app, "blame");
    r.selection.scope = "current".into();
    r.source = "HEAD".into();
    r.path = "renamed file.txt".into();
    r.line_start = Some(1);
    r.line_end = Some(200);
    let blocked = app.investigate_history(&r).unwrap();
    assert!(blocked.partial);
    assert!(blocked.evidence[0]
        .issue
        .as_deref()
        .unwrap()
        .contains("Configured blame ignore files"));
    git(&root, &["config", "--unset-all", "blame.ignoreRevsFile"]);
    let before = snapshot(&root);
    let page = app.investigate_history(&r).unwrap();
    assert!(!page.partial, "{page:?}");
    let e = &page.evidence[0];
    assert_eq!(e.source_oid.as_deref(), Some(tip.as_str()));
    assert_eq!(e.dirty, Some(true));
    assert_eq!(e.line_attribution.len(), 2);
    let lines = &e.line_attribution;
    assert_eq!(lines[0].oid, base);
    assert_eq!(lines[1].oid, phrase);
    assert_eq!(lines[0].path.display, "code.txt");
    assert_eq!(lines[1].line, 2);
    assert_eq!(lines[1].original_line, 2);
    assert_eq!(lines[1].content, "needle literal [x]");
    assert_eq!(snapshot(&root), before);
    for (start, end) in [(0, 1), (1, 201), (2, 1), (1, 1_000_001)] {
        r.line_start = Some(start);
        r.line_end = Some(end);
        assert!(app.investigate_history(&r).is_err());
    }
    r.line_start = Some(1);
    r.line_end = Some(2);
    r.selection.scope = "workspace".into();
    assert!(app.investigate_history(&r).is_err());
}

#[test]
fn line_blame_refuses_binary_and_oversize_files_without_changing_the_checkout() {
    let t = tempfile::tempdir().unwrap();
    let root = repo(t.path(), "blame-bounds");
    fs::write(root.join("binary"), b"binary\0data").unwrap();
    fs::write(root.join("large"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "bounded inputs"]);
    let app = Application::new(t.path().join("state/catalog.json")).unwrap();
    app.add_repository(&root).unwrap();
    let mut r = request(&app, "blame");
    r.selection.scope = "current".into();
    r.source = "HEAD".into();
    let before = snapshot(&root);
    for path in ["binary", "large", "missing"] {
        r.path = path.into();
        let p = app.investigate_history(&r).unwrap();
        assert!(p.partial);
        assert!(p.evidence[0].issue.is_some());
        assert!(p.hits.is_empty());
    }
    assert_eq!(snapshot(&root), before);
}

#[test]
fn package_comparison_matches_committed_names_and_reports_ambiguous_repositories() {
    let t = tempfile::tempdir().unwrap();
    let root = repo(t.path(), "application");
    let lib = repo(t.path(), "unrelated-folder-name");
    git(&root, &["checkout", "main"]);
    fs::write(
        root.join("package.json"),
        r#"{"name":"@fixture/app","version":"1.0.0","dependencies":{"@fixture/lib":"^1"}}"#,
    )
    .unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "old package"]);
    git(&root, &["branch", "-f", "release", "HEAD"]);
    git(&root, &["checkout", "develop"]);
    fs::write(
        root.join("package.json"),
        r#"{"name":"@fixture/app","version":"2.0.0","dependencies":{"@fixture/lib":"^2"}}"#,
    )
    .unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "new requirement"]);
    fs::write(
        lib.join("package.json"),
        r#"{"name":"@fixture/lib","version":"2.1.0"}"#,
    )
    .unwrap();
    git(&lib, &["add", "."]);
    git(&lib, &["commit", "-m", "library package"]);
    let lib_oid = git(&lib, &["rev-parse", "HEAD"]);
    fs::write(
        lib.join("package.json"),
        r#"{"name":"private-working-name"}"#,
    )
    .unwrap();
    let app = Application::new(t.path().join("state/catalog.json")).unwrap();
    app.add_repository(&lib).unwrap();
    app.add_repository(&root).unwrap();
    let mut r = request(&app, "compare");
    r.selection.scope = "current".into();
    let before = snapshot(&root);
    let lib_before = snapshot(&lib);
    let page = app.investigate_history(&r).unwrap();
    assert!(!page.partial, "{page:?}");
    let package = page.evidence[0].package_manifest.as_ref().unwrap();
    assert_eq!(package.source_version.as_deref(), Some("2.0.0"));
    assert_eq!(package.target_version.as_deref(), Some("1.0.0"));
    let row = &package.requirements[0];
    assert_eq!(row.source.as_deref(), Some("^2"));
    assert_eq!(row.target.as_deref(), Some("^1"));
    assert_eq!(row.locations.len(), 1);
    assert_eq!(row.locations[0].oid, lib_oid);
    assert_eq!(row.locations[0].version.as_deref(), Some("2.1.0"));
    assert!(row.issue.is_none());
    assert_eq!(snapshot(&root), before);
    assert_eq!(snapshot(&lib), lib_before);
    let duplicate = repo(t.path(), "ambiguous");
    fs::write(
        duplicate.join("package.json"),
        r#"{"name":"@fixture/lib","version":"3.0.0"}"#,
    )
    .unwrap();
    git(&duplicate, &["add", "."]);
    git(&duplicate, &["commit", "-m", "same declared name"]);
    app.add_repository(&duplicate).unwrap();
    app.select(&r.selection.current_membership_id).unwrap();
    let page = app.investigate_history(&r).unwrap();
    assert!(page.partial);
    let row = &page.evidence[0]
        .package_manifest
        .as_ref()
        .unwrap()
        .requirements[0];
    assert_eq!(row.locations.len(), 2);
    assert!(row
        .issue
        .as_deref()
        .unwrap()
        .contains("No unique registered repository"));
    fs::write(root.join("package.json"),r#"{"name":"@fixture/app","version":"2.0.0","workspaces":["packages/*"],"dependencies":{"@fixture/lib":"npm:another-package@2"}}"#).unwrap();
    git(&root, &["add", "package.json"]);
    git(&root, &["commit", "-m", "alias remains unresolved"]);
    let page = app.investigate_history(&r).unwrap();
    assert!(page.partial);
    let package = page.evidence[0].package_manifest.as_ref().unwrap();
    assert!(package
        .issue
        .as_deref()
        .unwrap()
        .contains("Only root package.json"));
    let row = &package.requirements[0];
    assert!(row.locations.is_empty());
    assert!(row.issue.as_deref().unwrap().contains("not resolved"));
}
