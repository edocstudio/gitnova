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
        fs::read(path.join(".git/index")).unwrap(),
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
