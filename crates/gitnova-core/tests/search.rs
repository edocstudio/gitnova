use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
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
    String::from_utf8_lossy(&out.stdout).trim_end().into()
}
fn repo(path: &Path, subject: &str) -> String {
    fs::create_dir(path).unwrap();
    git(path, &["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Alice Search"),
        ("user.email", "alice@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", ".git/hooks"),
        ("core.attributesFile", ".git/no-attributes"),
    ] {
        git(path, &["config", key, value]);
    }
    fs::write(path.join("file.txt"), "initial\n").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-m", subject]);
    git(path, &["rev-parse", "HEAD"])
}
fn setup() -> (tempfile::TempDir, Application, PathBuf, String) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    let oid = repo(&root, "Initial payment service");
    let app = Application::new(temp.path().join("app/state.json")).unwrap();
    app.add_repository(&root).unwrap();
    (temp, app, root, oid)
}
fn request(
    app: &Application,
    query: &str,
    scope: &str,
    selected_ids: Vec<String>,
) -> CommitSearchRequest {
    let c = app.catalog().unwrap();
    let w = c
        .workspaces
        .iter()
        .find(|w| w.id == c.active_workspace_id)
        .unwrap();
    CommitSearchRequest {
        workspace_id: w.id.clone(),
        current_membership_id: w.selected_id.clone().unwrap(),
        scope: scope.into(),
        selected_ids,
        query: query.into(),
        cursor: None,
    }
}
fn id(app: &Application) -> String {
    request(app, "x", "current", vec![]).current_membership_id
}
fn object_files(path: &Path) -> Vec<PathBuf> {
    fn walk(p: &Path, result: &mut Vec<PathBuf>) {
        for e in fs::read_dir(p).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, result);
            } else {
                result.push(p);
            }
        }
    }
    let mut result = vec![];
    walk(path, &mut result);
    result.sort();
    result
}
#[test]
fn all_scopes_search_real_messages_authors_emails_sha_and_navigation_is_readonly() {
    let (temp, app, first, first_oid) = setup();
    let first_id = id(&app);
    let second = temp.path().join("web");
    let second_oid = repo(
        &second,
        "Fix payment validation\n\nTicket NOVA-842 handles expired tokens.",
    );
    app.add_repository(&second).unwrap();
    let second_id = id(&app);
    assert_eq!(
        app.search_commits(&request(&app, "payment", "workspace", vec![]))
            .unwrap()
            .total_matches,
        2
    );
    let selected = app
        .search_commits(&request(
            &app,
            "NOVA-842",
            "selected",
            vec![first_id.clone()],
        ))
        .unwrap();
    assert!(selected.hits.is_empty());
    assert_eq!(
        app.search_commits(&request(&app, "ALICE expired", "current", vec![]))
            .unwrap()
            .hits[0]
            .commit
            .oid,
        second_oid
    );
    assert_eq!(
        app.search_commits(&request(&app, "alice@example.invalid", "workspace", vec![]))
            .unwrap()
            .total_matches,
        2
    );
    assert_eq!(
        app.search_commits(&request(&app, &first_oid[..12], "workspace", vec![]))
            .unwrap()
            .hits[0]
            .locations[0]
            .membership_id,
        first_id
    );
    let index = fs::read(first.join(".git/index")).unwrap();
    let objects = object_files(&first.join(".git/objects"));
    let selected_before = id(&app);
    let focus = app
        .focus_commit(
            &request(&app, "x", "current", vec![]).workspace_id,
            &first_id,
            &first_oid,
        )
        .unwrap();
    assert_eq!(focus.page.rows[focus.selected_index].commit.oid, first_oid);
    assert_eq!(id(&app), selected_before);
    assert_eq!(selected_before, second_id);
    assert_eq!(fs::read(first.join(".git/index")).unwrap(), index);
    assert_eq!(object_files(&first.join(".git/objects")), objects);
}
#[test]
fn worktrees_share_one_index_and_detached_reachability_remains_location_specific() {
    let (temp, app, root, base) = setup();
    let main_id = id(&app);
    let linked = temp.path().join("linked");
    git(
        &root,
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().unwrap(),
            &base,
        ],
    );
    fs::write(linked.join("detached.txt"), "worktree only\n").unwrap();
    git(&linked, &["add", "."]);
    git(&linked, &["commit", "-m", "Detached workspace needle"]);
    let detached = git(&linked, &["rev-parse", "HEAD"]);
    app.add_repository(&linked).unwrap();
    let linked_id = id(&app);
    let page = app
        .search_commits(&request(&app, "payment", "workspace", vec![]))
        .unwrap();
    assert_eq!(page.total_matches, 1);
    assert_eq!(page.hits[0].locations.len(), 2);
    assert_eq!(page.coverage.len(), 1);
    let page = app
        .search_commits(&request(&app, "Detached workspace", "workspace", vec![]))
        .unwrap();
    assert_eq!(page.hits[0].commit.oid, detached);
    assert_eq!(page.hits[0].locations.len(), 1);
    assert_eq!(page.hits[0].locations[0].membership_id, linked_id);
    app.select(&main_id).unwrap();
    assert_eq!(
        app.search_commits(&request(&app, "Detached workspace", "current", vec![]))
            .unwrap()
            .total_matches,
        0
    );
    let files = fs::read_dir(temp.path().join("app/search"))
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 1);
    let cache: serde_json::Value =
        serde_json::from_slice(&fs::read(files[0].as_ref().unwrap().path()).unwrap()).unwrap();
    assert!(cache["entries"].get(&base).is_some());
    assert!(
        cache["entries"].get(&detached).is_some(),
        "Changing reachability scope must reuse immutable worktree metadata"
    );
}
#[test]
fn cache_is_persisted_revalidated_and_corruption_rebuilds_from_git() {
    let (temp, app, root, _) = setup();
    let page = app
        .search_commits(&request(&app, "payment", "current", vec![]))
        .unwrap();
    assert_eq!(page.total_matches, 1);
    let file = fs::read_dir(temp.path().join("app/search"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut cache: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    let entry = cache["entries"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    entry["message"] = serde_json::json!("forged cache text");
    fs::write(&file, serde_json::to_vec(&cache).unwrap()).unwrap();
    let reopened = Application::new(temp.path().join("app/state.json")).unwrap();
    assert_eq!(
        reopened
            .search_commits(&request(&reopened, "payment", "current", vec![]))
            .unwrap()
            .total_matches,
        1
    );
    fs::write(root.join("file.txt"), "changed\n").unwrap();
    git(&root, &["commit", "-am", "Freshly indexed change"]);
    assert_eq!(
        reopened
            .search_commits(&request(&reopened, "freshly", "current", vec![]))
            .unwrap()
            .total_matches,
        1
    );
}
#[test]
fn absent_repository_is_reported_without_hiding_other_results() {
    let (temp, app, _root, _) = setup();
    let gone = temp.path().join("missing");
    repo(&gone, "missing payment");
    app.add_repository(&gone).unwrap();
    fs::rename(&gone, temp.path().join("moved")).unwrap();
    let page = app
        .search_commits(&request(&app, "payment", "workspace", vec![]))
        .unwrap();
    assert!(page.partial);
    assert_eq!(page.total_matches, 1);
    assert!(page.coverage.iter().any(|c| c.issue.is_some()));
}
fn import_history(path: &Path, count: usize) {
    let mut script = String::new();
    let original = git(path, &["rev-parse", "HEAD"]);
    for i in 0..count {
        let message = if i == 110 {
            "Commit 110\n\nOlder exact navigation needle\n".to_owned()
        } else {
            format!("Commit {i}\n")
        };
        let data = format!("version {i}\n");
        let parent = if i == 0 {
            format!("from {original}\n")
        } else {
            String::new()
        };
        script.push_str(&format!("commit refs/heads/main\ncommitter Alice Search <alice@example.invalid> {} +0000\ndata {}\n{}{parent}M 100644 inline file.txt\ndata {}\n{}\n",1700000000+i,message.len(),message,data.len(),data));
    }
    script.push_str("done\n");
    let mut child = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    git(path, &["reset", "--hard", "HEAD"]);
}
#[test]
fn old_results_open_exact_graph_range_and_continue_with_valid_lane_cursor() {
    let (temp, app, root, _) = setup();
    import_history(&root, 320);
    let r = request(&app, "Older exact navigation", "current", vec![]);
    let hit = app.search_commits(&r).unwrap().hits.remove(0);
    let head = git(&root, &["rev-parse", "HEAD"]);
    let index = fs::read(root.join(".git/index")).unwrap();
    let focused = app
        .focus_commit(&r.workspace_id, &r.current_membership_id, &hit.commit.oid)
        .unwrap();
    assert!(focused.base >= 200);
    assert_eq!(
        focused.page.rows[focused.selected_index].commit.oid,
        hit.commit.oid
    );
    let mut cursor = None;
    let expected = loop {
        let page = app.graph(&r.current_membership_id, cursor).unwrap();
        if page.rows.iter().any(|row| row.commit.oid == hit.commit.oid) {
            break page;
        }
        cursor = page.next;
    };
    assert_eq!(
        serde_json::to_value(&focused.page.rows).unwrap(),
        serde_json::to_value(&expected.rows).unwrap()
    );
    let cursor = focused
        .page
        .next
        .expect("Deep focus must continue into older history");
    assert!(!app
        .graph(&r.current_membership_id, Some(cursor))
        .unwrap()
        .rows
        .is_empty());
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    drop(temp);
}
#[test]
fn pagination_detects_scope_and_ref_changes() {
    let (_temp, app, root, _) = setup();
    import_history(&root, 130);
    let mut r = request(&app, "Commit", "current", vec![]);
    let first = app.search_commits(&r).unwrap();
    assert_eq!(first.hits.len(), 100);
    r.cursor = first.next;
    assert!(r.cursor.is_some());
    assert_eq!(app.search_commits(&r).unwrap().hits.len(), 30);
    fs::write(root.join("file.txt"), "new\n").unwrap();
    git(&root, &["commit", "-am", "Commit newly added"]);
    assert_eq!(app.search_commits(&r).unwrap_err().code, "stale_search");
    r.cursor = None;
    assert_eq!(app.search_commits(&r).unwrap().total_matches, 131);
}

#[test]
fn broad_search_pages_materialize_correctly_after_resident_index_eviction() {
    let temp = tempfile::tempdir().unwrap();
    let app = Application::new(temp.path().join("app/state.json")).unwrap();
    for i in 0..4 {
        let path = temp.path().join(format!("repo-{i}"));
        repo(&path, "Root");
        import_history(&path, 50);
        app.add_repository(&path).unwrap();
    }
    let mut query = request(&app, "Commit", "workspace", vec![]);
    let first = app.search_commits(&query).unwrap();
    assert_eq!(first.total_matches, 200);
    assert_eq!(first.hits.len(), 100);
    assert!(!first.partial);
    query.cursor = first.next.clone();
    let second = app.search_commits(&query).unwrap();
    assert_eq!(second.hits.len(), 100);
    assert!(second.next.is_none());
    let hits = first.hits.iter().chain(&second.hits).collect::<Vec<_>>();
    let keys = hits
        .iter()
        .map(|hit| {
            (
                hit.locations[0].repository_id.clone(),
                hit.commit.oid.clone(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys.len(), 200);
    assert!(hits
        .iter()
        .all(|hit| hit.locations.len() == 1 && hit.excerpt.starts_with("Commit")));
    let warm = app
        .search_commits(&request(&app, "Commit", "workspace", vec![]))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&first.hits).unwrap(),
        serde_json::to_value(&warm.hits).unwrap()
    );
}
#[test]
fn scopes_reject_foreign_workspace_memberships_and_recovery_roots_stay_hidden() {
    let (_temp, app, root, original) = setup();
    let original_id = id(&app);
    let workspace = request(&app, "x", "current", vec![]).workspace_id;
    git(&root, &["switch", "-c", "temporary"]);
    fs::write(root.join("other"), "hidden\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Recovery-only hidden needle"]);
    let hidden = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["switch", "main"]);
    git(
        &root,
        &[
            "update-ref",
            "refs/gitnova/recovery/search-fixture",
            &hidden,
        ],
    );
    git(&root, &["branch", "-D", "temporary"]);
    assert_eq!(
        app.search_commits(&request(&app, "hidden needle", "current", vec![]))
            .unwrap()
            .total_matches,
        0
    );
    assert!(app.focus_commit(&workspace, &original_id, &hidden).is_err());
    let mut r = request(&app, "payment", "selected", vec!["unregistered".into()]);
    assert!(app.search_commits(&r).is_err());
    r = request(&app, "payment", "current", vec![]);
    app.create_workspace("Other").unwrap();
    assert_eq!(app.search_commits(&r).unwrap_err().code, "stale_search");
    assert!(app
        .focus_commit(&workspace, &original_id, &original)
        .is_err());
}
#[test]
fn header_batches_keep_large_eligible_bodies_bounded_and_pages_exact() {
    let (_temp, app, root, _) = setup();
    let original = git(&root, &["rev-parse", "HEAD"]);
    let mut child = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    // The combined bodies exceed one reader's 5 MiB output limit, but each
    // commit is eligible. All must survive indexing and persisted-cache reload.
    for i in 0..130 {
        let message = format!("Batch needle {i}\n\n{}\n", "x".repeat(60_000));
        let parent = if i == 0 {
            format!("from {original}\n")
        } else {
            String::new()
        };
        write!(input, "commit refs/heads/main\ncommitter Batch Test <batch@example.invalid> {} +0000\ndata {}\n{}{parent}\n", 1700000000+i, message.len(), message).unwrap();
    }
    input.write_all(b"done\n").unwrap();
    drop(input);
    assert!(child.wait().unwrap().success());
    git(&root, &["reset", "--hard", "HEAD"]);
    let mut query = request(&app, "batch needle", "current", vec![]);
    let first = app.search_commits(&query).unwrap();
    assert!(!first.partial);
    assert_eq!(first.total_matches, 130);
    assert_eq!(first.hits.len(), 100);
    query.cursor = first.next.clone();
    let second = app.search_commits(&query).unwrap();
    assert_eq!(second.hits.len(), 30);
    assert!(second.next.is_none());
    let unique = first
        .hits
        .iter()
        .chain(&second.hits)
        .map(|hit| &hit.commit.oid)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), 130);
    let reopened = Application::new(root.parent().unwrap().join("app/state.json")).unwrap();
    assert_eq!(
        serde_json::to_value(reopened.search_commits(&query).unwrap().hits).unwrap(),
        serde_json::to_value(second.hits).unwrap()
    );
}
#[test]
fn large_and_shallow_histories_are_explicitly_partial() {
    let (_temp, app, root, base) = setup();
    let file = root.parent().unwrap().join("large-message");
    fs::write(
        &file,
        format!("Oversized\n\n{}", "large-body-needle".repeat(5000)),
    )
    .unwrap();
    git(
        &root,
        &["commit", "--allow-empty", "-F", file.to_str().unwrap()],
    );
    let page = app
        .search_commits(&request(&app, "large-body-needle", "current", vec![]))
        .unwrap();
    assert!(page.partial);
    assert_eq!(page.total_matches, 0);
    fs::write(root.join(".git/shallow"), format!("{base}\n")).unwrap();
    let page = app
        .search_commits(&request(&app, "payment", "current", vec![]))
        .unwrap();
    assert!(page.partial);
    assert_eq!(page.total_matches, 1);
}
#[test]
fn replacement_history_is_not_mistaken_for_immutable_cached_metadata() {
    let (_temp, app, root, base) = setup();
    app.search_commits(&request(&app, "payment", "current", vec![]))
        .unwrap();
    fs::write(root.join("file.txt"), "replacement\n").unwrap();
    git(&root, &["commit", "-am", "Replacement identity"]);
    let replacement = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["replace", &base, &replacement]);
    let page = app
        .search_commits(&request(&app, "payment", "current", vec![]))
        .unwrap();
    assert!(page.partial);
    assert!(page.coverage[0]
        .issue
        .as_ref()
        .unwrap()
        .contains("Replacement"));
}
#[test]
fn literal_control_text_and_unicode_do_not_break_batch_framing() {
    let (_temp, app, root, _) = setup();
    let file = root.parent().unwrap().join("message");
    fs::write(&file, "แก้ไขระบบ\n\nToken\u{001e}body ตัวอักษรไทย\n").unwrap();
    git(
        &root,
        &["commit", "--allow-empty", "-F", file.to_str().unwrap()],
    );
    let page = app
        .search_commits(&request(&app, "ตัวอักษรไทย", "current", vec![]))
        .unwrap();
    assert_eq!(page.total_matches, 1);
    assert_eq!(page.hits[0].commit.subject, "แก้ไขระบบ");
}
#[test]
fn empty_and_bare_repositories_have_truthful_results() {
    let (temp, app, root, _) = setup();
    let bare = temp.path().join("bare.git");
    git(
        &root,
        &[
            "clone",
            "--bare",
            root.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    app.add_repository(&bare).unwrap();
    assert_eq!(
        app.search_commits(&request(&app, "payment", "current", vec![]))
            .unwrap()
            .total_matches,
        1
    );
    let empty = temp.path().join("empty");
    fs::create_dir(&empty).unwrap();
    git(&empty, &["init", "-b", "main"]);
    app.add_repository(&empty).unwrap();
    let page = app
        .search_commits(&request(&app, "anything", "current", vec![]))
        .unwrap();
    assert_eq!(page.total_matches, 0);
    assert!(!page.partial);
    assert!(app
        .search_commits(&request(&app, "", "workspace", vec![]))
        .is_err());
}

#[cfg(unix)]
#[test]
fn indexing_and_focus_do_not_run_signature_monitor_filter_or_diff_helpers() {
    use std::os::unix::fs::PermissionsExt;
    let (temp, app, root, base) = setup();
    let marker = temp.path().join("executed");
    let helper = temp.path().join("untrusted-helper");
    fs::write(
        &helper,
        format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
    let tree = git(&root, &["rev-parse", "HEAD^{tree}"]);
    let commit=format!("tree {tree}\nparent {base}\nauthor Signed Test <signed@example.invalid> 1700000000 +0000\ncommitter Signed Test <signed@example.invalid> 1700000000 +0000\ngpgsig -----BEGIN PGP SIGNATURE-----\n invalid\n -----END PGP SIGNATURE-----\n\nSigned search needle\n");
    let mut child = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["hash-object", "-t", "commit", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(commit.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let signed = String::from_utf8(out.stdout).unwrap().trim().to_owned();
    git(&root, &["update-ref", "refs/heads/signed", &signed]);
    for key in [
        "core.fsmonitor",
        "gpg.program",
        "diff.external",
        "filter.untrusted.clean",
        "filter.untrusted.process",
    ] {
        git(&root, &["config", key, helper.to_str().unwrap()]);
    }
    git(&root, &["config", "log.showSignature", "true"]);
    let r = request(&app, "Signed search needle", "current", vec![]);
    assert_eq!(app.search_commits(&r).unwrap().hits[0].commit.oid, signed);
    app.focus_commit(&r.workspace_id, &r.current_membership_id, &signed)
        .unwrap();
    assert!(!marker.exists());
}
