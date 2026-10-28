use gitnova_core::Application;
use gitnova_domain::*;
use gitnova_git::{DiffReader, GitCli, HistoryReader, RepositoryReader};
use std::{path::Path, process::Command};
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=GitNova Test",
            "-c",
            "user.email=test@gitnova.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
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
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .into()
}
fn init(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    git(path, &["init", "-b", "main"]);
}
fn commit(path: &Path, name: &str, content: &str) -> String {
    std::fs::write(path.join(name), content).unwrap();
    git(path, &["add", "--", name]);
    git(path, &["commit", "-m", content]);
    git(path, &["rev-parse", "HEAD"])
}
fn selected(c: &Catalog) -> String {
    c.workspaces
        .iter()
        .find(|w| w.id == c.active_workspace_id)
        .unwrap()
        .selected_id
        .clone()
        .unwrap()
}
fn app(root: &Path) -> Application {
    Application::new(root.join("app/state.json")).unwrap()
}
#[test]
fn real_multi_repo_graph_details_and_persistence() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("a");
    let b = d.path().join("b");
    init(&a);
    init(&b);
    let root = commit(&a, "hello.txt", "hello root");
    let head = commit(&a, "hello.txt", "updated content");
    commit(&b, "b.txt", "second repository");
    let app = app(d.path());
    let first = selected(&app.add_repository(&a).unwrap());
    let second = selected(&app.add_repository(&b).unwrap());
    assert_ne!(first, second);
    let page = app.graph(&first, None).unwrap();
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.rows[0].commit.oid, head);
    assert_eq!(page.rows[1].commit.oid, root);
    let before = git(&a, &["status", "--porcelain=v1"]);
    let index = std::fs::read(a.join(".git/index")).unwrap();
    let refs = git(&a, &["show-ref"]);
    let detail = app.detail(&first, &head).unwrap();
    assert!(detail.diff.contains("+updated content"));
    assert!(app
        .detail(&first, &root)
        .unwrap()
        .diff
        .contains("+hello root"));
    assert_eq!(git(&a, &["status", "--porcelain=v1"]), before);
    assert_eq!(std::fs::read(a.join(".git/index")).unwrap(), index);
    assert_eq!(git(&a, &["show-ref"]), refs);
    app.pin(&first).unwrap();
    app.select(&first).unwrap();
    let reloaded = Application::new(d.path().join("app/state.json")).unwrap();
    let c = reloaded.catalog().unwrap();
    assert_eq!(selected(&c), first);
    assert!(c.workspaces[0]
        .memberships
        .iter()
        .any(|m| m.id == first && m.pinned));
    let new = app.create_workspace("Another project").unwrap();
    assert!(new.workspaces[1].memberships.is_empty());
    let c = app.add_repository(&a).unwrap();
    assert_eq!(c.repositories.len(), 2);
    assert_eq!(c.workspaces[1].memberships.len(), 1);
}
#[test]
fn empty_bare_detached_and_dirty_repositories() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    init(&repo);
    let app = app(d.path());
    let id = selected(&app.add_repository(&repo).unwrap());
    assert!(app.graph(&id, None).unwrap().rows.is_empty());
    let oid = commit(&repo, "file", "root");
    git(&repo, &["checkout", "--detach", &oid]);
    std::fs::write(repo.join("file"), "dirty").unwrap();
    let page = app.graph(&id, None).unwrap();
    assert!(page.status.branch.is_none());
    assert!(page.status.dirty);
    let bare = d.path().join("bare.git");
    git(
        d.path(),
        &[
            "clone",
            "--bare",
            repo.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    let bare_id = selected(&app.add_repository(&bare).unwrap());
    let page = app.graph(&bare_id, None).unwrap();
    assert!(page.status.bare);
    assert_eq!(page.rows.len(), 1);
    assert!(app.submodules(&bare_id).unwrap().is_empty());
}
#[test]
fn worktrees_share_repository_identity_and_can_be_opened() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    let linked = d.path().join("linked");
    init(&repo);
    commit(&repo, "file", "root");
    git(
        &repo,
        &["worktree", "add", "-b", "feature", linked.to_str().unwrap()],
    );
    let app = app(d.path());
    let id = selected(&app.add_repository(&repo).unwrap());
    let trees = app.worktrees(&id).unwrap();
    assert_eq!(trees.len(), 2);
    let tree = trees
        .iter()
        .find(|w| w.branch.as_deref() == Some("feature"))
        .unwrap();
    let c = app.open_related(&id, &tree.path).unwrap();
    assert_eq!(c.repositories.len(), 1);
    assert_eq!(c.worktrees.len(), 2);
    assert_eq!(c.workspaces[0].memberships.len(), 2);
    let linked_id = selected(&c);
    assert_eq!(
        app.status(&linked_id).unwrap().branch.as_deref(),
        Some("feature")
    );
    assert!(app.open_related(&id, &NativePath::new(d.path())).is_err());
}
#[test]
fn pagination_is_topological_and_invalidates_after_ref_changes() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    init(&repo);
    commit(&repo, "file", "root");
    for n in 0..103 {
        git(
            &repo,
            &["commit", "--allow-empty", "-m", &format!("commit {n}")],
        );
    }
    let app = app(d.path());
    let id = selected(&app.add_repository(&repo).unwrap());
    let first = app.graph(&id, None).unwrap();
    assert_eq!(first.rows.len(), 100);
    let cursor = first.next.unwrap();
    let second = app.graph(&id, Some(cursor.clone())).unwrap();
    assert_eq!(second.rows.len(), 4);
    assert!(second.next.is_none());
    assert_eq!(
        first.rows.last().unwrap().commit.parents[0],
        second.rows[0].commit.oid
    );
    git(&repo, &["commit", "--allow-empty", "-m", "external change"]);
    assert_eq!(
        app.graph(&id, Some(cursor)).unwrap_err().code,
        "stale_graph"
    );
}
#[test]
fn merge_topology_and_first_parent_diff() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    init(&repo);
    commit(&repo, "root", "root");
    git(&repo, &["checkout", "-b", "feature"]);
    let feature = commit(&repo, "feature", "feature change");
    git(&repo, &["checkout", "main"]);
    commit(&repo, "main", "main change");
    git(
        &repo,
        &["merge", "--no-ff", "feature", "-m", "merge feature"],
    );
    let history = GitCli.history(&repo, 0, 100).unwrap();
    assert_eq!(history[0].parents.len(), 2);
    let mut seen = std::collections::HashSet::new();
    for c in &history {
        for parent in &c.parents {
            assert!(!seen.contains(parent));
        }
        seen.insert(c.oid.clone());
    }
    let detail = GitCli.detail(&repo, &history[0].oid).unwrap();
    assert!(detail.diff.contains("+feature change"));
    assert!(!detail.diff.contains("+main change"));
    assert!(history.iter().any(|c| c.oid == feature));
}
#[test]
fn submodule_tracks_head_index_checkout_and_uninitialized_state() {
    let d = tempfile::tempdir().unwrap();
    let child = d.path().join("child");
    let parent = d.path().join("parent");
    init(&child);
    let old = commit(&child, "child", "old");
    let newer = commit(&child, "child", "new");
    init(&parent);
    commit(&parent, "root", "parent");
    git(
        &parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child.to_str().unwrap(),
            "module",
        ],
    );
    git(&parent, &["commit", "-am", "add submodule"]);
    let app = app(d.path());
    let id = selected(&app.add_repository(&parent).unwrap());
    let module = parent.join("module");
    git(&module, &["checkout", &old]);
    let sub = app.submodules(&id).unwrap().remove(0);
    assert_eq!(sub.head_oid.as_deref(), Some(newer.as_str()));
    assert_eq!(sub.index_oid, newer);
    assert_eq!(sub.checked_out_oid.as_deref(), Some(old.as_str()));
    assert!(app.status(&id).unwrap().dirty);
    git(&parent, &["add", "module"]);
    let sub = app.submodules(&id).unwrap().remove(0);
    assert_eq!(sub.index_oid, old);
    assert_ne!(sub.head_oid.as_deref(), Some(old.as_str()));
    std::fs::write(module.join("child"), "dirty child").unwrap();
    assert_eq!(app.submodules(&id).unwrap()[0].dirty, Some(true));
    let clone = d.path().join("uninitialized");
    git(
        d.path(),
        &["clone", parent.to_str().unwrap(), clone.to_str().unwrap()],
    );
    let clone_id = selected(&app.add_repository(&clone).unwrap());
    let sub = app.submodules(&clone_id).unwrap().remove(0);
    assert!(!sub.initialized);
    assert!(sub.checked_out_oid.is_none());
}
#[test]
fn nested_independent_repository_is_not_a_submodule() {
    let d = tempfile::tempdir().unwrap();
    let parent = d.path().join("parent");
    init(&parent);
    commit(&parent, "file", "parent");
    let nested = parent.join("nested");
    init(&nested);
    commit(&nested, "file", "nested");
    let app = app(d.path());
    let id = selected(&app.add_repository(&parent).unwrap());
    assert!(app.submodules(&id).unwrap().is_empty());
    let c = app.add_repository(&nested).unwrap();
    assert_eq!(c.repositories.len(), 2);
}
#[test]
fn sha256_and_shallow_history_use_native_oid_lengths() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("sha256");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "--object-format=sha256", "-b", "main"]);
    let oid = commit(&repo, "file", "root");
    assert_eq!(oid.len(), 64);
    assert_eq!(GitCli.discover(&repo).unwrap().object_format, "sha256");
    assert_eq!(GitCli.history(&repo, 0, 100).unwrap()[0].oid, oid);
    assert!(GitCli.detail(&repo, &oid).unwrap().diff.contains("+root"));
    let source = d.path().join("source");
    init(&source);
    commit(&source, "file", "first");
    commit(&source, "file", "second");
    let clone = d.path().join("shallow");
    git(
        d.path(),
        &[
            "clone",
            "--depth=1",
            &format!("file://{}", source.display()),
            clone.to_str().unwrap(),
        ],
    );
    assert_eq!(GitCli.history(&clone, 0, 100).unwrap().len(), 1);
}
#[test]
fn external_diff_and_textconv_are_not_executed() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    init(&repo);
    let oid = commit(&repo, "file", "root");
    git(
        &repo,
        &[
            "config",
            "diff.external",
            "this-command-must-not-exist-gitnova",
        ],
    );
    git(
        &repo,
        &[
            "config",
            "diff.test.textconv",
            "this-command-must-not-exist-gitnova",
        ],
    );
    std::fs::write(repo.join(".gitattributes"), "file diff=test\n").unwrap();
    assert!(GitCli.detail(&repo, &oid).unwrap().diff.contains("+root"));
    assert!(GitCli.detail(&repo, "--output=/tmp/not-allowed").is_err());
}
#[cfg(unix)]
#[test]
fn symlinks_preserve_identity() {
    use std::os::unix::fs::symlink;
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    init(&repo);
    commit(&repo, "file", "root");
    let alias = d.path().join("alias");
    symlink(&repo, &alias).unwrap();
    let app = app(d.path());
    let c = app.add_repository(&repo).unwrap();
    let id = selected(&c);
    let c = app.add_repository(&alias).unwrap();
    assert_eq!(c.repositories.len(), 1);
    assert_eq!(c.worktrees.len(), 1);
    assert_eq!(selected(&c), id);
    assert_eq!(app.graph(&id, None).unwrap().rows.len(), 1);
    assert_eq!(NativePath::new(&repo).to_path().unwrap(), repo);
}
#[test]
fn missing_repository_and_corrupt_objects_return_errors() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    init(&repo);
    let oid = commit(&repo, "file", "root");
    let app = app(d.path());
    let id = selected(&app.add_repository(&repo).unwrap());
    std::fs::remove_file(repo.join(".git/objects").join(&oid[..2]).join(&oid[2..])).unwrap();
    assert!(app.graph(&id, None).is_err());
    assert!(app.add_repository(&d.path().join("missing")).is_err());
    assert!(app.graph("unregistered", None).is_err());
}
#[test]
fn annotated_tags_point_to_commits_in_the_graph() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    init(&repo);
    let oid = commit(&repo, "file", "root");
    git(&repo, &["tag", "-a", "v1", "-m", "release"]);
    let app = app(d.path());
    let id = selected(&app.add_repository(&repo).unwrap());
    let page = app.graph(&id, None).unwrap();
    assert!(page
        .refs
        .iter()
        .any(|r| r.name == "refs/tags/v1" && r.oid == oid));
}

#[cfg(unix)]
#[test]
fn native_path_roundtrips_non_utf8_bytes_without_using_display() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let path = std::path::PathBuf::from(OsString::from_vec(b"/repo-\xff/file-\xfe".to_vec()));
    let mut stored = NativePath::new(&path);
    stored.display = "not a real path".into();
    assert_eq!(stored.to_path().unwrap(), path);
}
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_repository_is_readable_on_linux() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join(OsString::from_vec(b"repo-\xff".to_vec()));
    init(&repo);
    commit(&repo, "file", "root");
    let app = app(d.path());
    let id = selected(&app.add_repository(&repo).unwrap());
    assert_eq!(app.graph(&id, None).unwrap().rows.len(), 1);
}

#[test]
fn real_octopus_criss_cross_and_timestamp_skew_remain_topological() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    init(&repo);
    let tree = git(&repo, &["mktree"]);
    let create = |message: &str, parents: &[&str], timestamp: &str| {
        let mut command = Command::new("git");
        command.arg("-C").arg(&repo).args([
            "-c",
            "user.name=GitNova Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit-tree",
            &tree,
            "-m",
            message,
        ]);
        for parent in parents {
            command.args(["-p", parent]);
        }
        let output = command
            .env("GIT_AUTHOR_DATE", timestamp)
            .env("GIT_COMMITTER_DATE", timestamp)
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
    let root = create("root in the future", &[], "2000000000 +0000");
    let a = create("a", &[&root], "1500000000 +0000");
    let b = create("b", &[&root], "1500000100 +0000");
    let c = create("c", &[&root], "1500000200 +0000");
    let x = create("cross a b", &[&a, &b], "1400000000 +0000");
    let y = create("cross b a", &[&b, &a], "1400000100 +0000");
    let merge = create("octopus", &[&x, &y, &c], "1300000000 +0000");
    git(&repo, &["update-ref", "refs/heads/main", &merge]);
    let app = app(d.path());
    let id = selected(&app.add_repository(&repo).unwrap());
    let page = app.graph(&id, None).unwrap();
    assert_eq!(page.rows.len(), 7);
    assert_eq!(page.rows[0].commit.oid, merge);
    assert_eq!(page.rows[0].edges.len(), 3);
    assert_eq!(page.rows.last().unwrap().commit.oid, root);
    let mut seen = std::collections::HashSet::new();
    for row in &page.rows {
        for parent in &row.commit.parents {
            assert!(!seen.contains(parent));
        }
        seen.insert(row.commit.oid.clone());
    }
}

#[test]
fn parallel_branch_tips_interleave_and_keep_lanes_across_pages() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("parallel");
    init(&repo);
    let root = commit(&repo, "file", "root");
    let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);
    let mut tips = vec![root.clone(); 3];
    let mut expected = std::collections::HashMap::new();
    for generation in 0..40 {
        for (branch, tip) in tips.iter_mut().enumerate() {
            let time = format!("{} +0000", 2_000_000_000 + generation * 60 + branch);
            let output = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args([
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "commit-tree",
                    &tree,
                    "-p",
                    tip,
                    "-m",
                    &format!("branch {branch} generation {generation}"),
                ])
                .env("GIT_AUTHOR_DATE", &time)
                .env("GIT_COMMITTER_DATE", &time)
                .output()
                .unwrap();
            assert!(output.status.success());
            *tip = String::from_utf8(output.stdout).unwrap().trim().to_string();
            expected.insert(tip.clone(), branch);
        }
    }
    for (branch, tip) in tips.iter().enumerate() {
        let name = if branch == 0 {
            "refs/heads/main".to_string()
        } else {
            format!("refs/heads/feature/{branch}")
        };
        git(&repo, &["update-ref", &name, tip]);
    }
    let app = app(d.path());
    let id = selected(&app.add_repository(&repo).unwrap());
    let first = app.graph(&id, None).unwrap();
    assert_eq!(first.rows.len(), 100);
    assert!(tips
        .iter()
        .all(|tip| first.rows[..3].iter().any(|row| &row.commit.oid == tip)));
    assert_eq!(
        first
            .rows
            .iter()
            .find(|row| row.commit.oid == tips[0])
            .unwrap()
            .lane,
        0
    );
    let cursor = first.next.clone().unwrap();
    let second = app.graph(&id, Some(cursor.clone())).unwrap();
    assert_eq!(second.rows.len(), 21);
    assert!(second.next.is_none());
    let mut seen = std::collections::HashSet::new();
    let mut lanes = std::collections::HashMap::new();
    for row in first.rows.iter().chain(&second.rows) {
        for parent in &row.commit.parents {
            assert!(!seen.contains(parent));
        }
        seen.insert(row.commit.oid.clone());
        if let Some(branch) = expected.get(&row.commit.oid) {
            let lane = lanes.entry(branch).or_insert((row.lane, row.color));
            assert_eq!(*lane, (row.lane, row.color));
        }
    }
    assert_eq!(seen.len(), 121);
    let mut stale = cursor;
    stale.version = 1;
    assert_eq!(app.graph(&id, Some(stale)).unwrap_err().code, "stale_graph");
}

#[cfg(unix)]
#[test]
fn newline_repository_paths_keep_exact_identity_with_batched_discovery() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("project\nwith newline");
    std::fs::create_dir(&root).unwrap();
    init(&root);
    commit(&root, "file.txt", "content\n");
    let app = app(d.path());
    let catalog = app.add_repository(&root).unwrap();
    let id = selected(&catalog);
    let page = app.graph(&id, None).unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(app.path(&id).unwrap(), root.canonicalize().unwrap());
}
