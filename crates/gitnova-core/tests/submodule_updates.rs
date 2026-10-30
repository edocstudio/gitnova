use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.file.allow=always",
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
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim_end().into()
}
fn init(path: &Path) {
    fs::create_dir(path).unwrap();
    git(path, &["init", "-b", "main"]);
}
fn commit(path: &Path, name: &str, contents: &str) -> String {
    fs::write(path.join(name), contents).unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-m", contents]);
    git(path, &["rev-parse", "HEAD"])
}
struct Fixture {
    _dir: tempfile::TempDir,
    app: Application,
    root: PathBuf,
    id: String,
    first: String,
    second: String,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    let parent = dir.path().join("parent");
    let root = dir.path().join("local");
    init(&source);
    let first = commit(&source, "file.txt", "first\n");
    let second = commit(&source, "file.txt", "second\n");
    init(&parent);
    git(
        &parent,
        &["submodule", "add", "--", source.to_str().unwrap(), "module"],
    );
    git(&parent.join("module"), &["switch", "--detach", &first]);
    git(&parent, &["add", "."]);
    git(&parent, &["commit", "-m", "Record module"]);
    git(
        dir.path(),
        &[
            "clone",
            "--",
            parent.to_str().unwrap(),
            root.to_str().unwrap(),
        ],
    );
    git(&root, &["config", "core.hooksPath", ".git/hooks"]);
    git(
        &root,
        &["config", "core.attributesFile", ".git/empty-attributes"],
    );
    let app = Application::new(dir.path().join("app/state.json")).unwrap();
    let catalog = app.add_repository(&root).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    Fixture {
        _dir: dir,
        app,
        root,
        id,
        first,
        second,
    }
}
fn module() -> NativePath {
    NativePath::new(Path::new("module"))
}
fn initialize(f: &Fixture) {
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "initialize")
        .unwrap();
    assert!(
        preview.blocked_reason.is_none(),
        "{:?}",
        preview.blocked_reason
    );
    let receipt = f
        .app
        .update_submodule(&f.id, &module(), "initialize", &preview.token)
        .unwrap();
    assert_eq!(
        receipt.status, "succeeded",
        "{} {:?}",
        receipt.summary, receipt.recovery
    );
}
#[test]
fn initialization_is_previewed_nonrecursive_and_preserves_parent_index() {
    let f = fixture();
    fs::write(f.root.join("pending.txt"), "keep\n").unwrap();
    fs::write(f.root.join("staged.txt"), "prepared\n").unwrap();
    git(&f.root, &["add", "staged.txt"]);
    git(&f.root, &["config", "submodule.recurse", "true"]);
    git(
        &f.root,
        &[
            "config",
            "submodule.module.update",
            "!touch custom-must-not-run",
        ],
    );
    let index = fs::read(f.root.join(".git/index")).unwrap();
    let config = fs::read(f.root.join(".git/config")).unwrap();
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "initialize")
        .unwrap();
    assert!(preview.network_required);
    assert_eq!(preview.submodule.index_oid, f.first);
    assert!(!f.root.join("module/.git").exists());
    assert_eq!(fs::read(f.root.join(".git/config")).unwrap(), config);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    initialize(&f);
    assert_eq!(git(&f.root.join("module"), &["rev-parse", "HEAD"]), f.first);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert_eq!(git(&f.root, &["show", ":staged.txt"]), "prepared");
    assert_eq!(
        fs::read_to_string(f.root.join("pending.txt")).unwrap(),
        "keep\n"
    );
    assert!(!f.root.join("custom-must-not-run").exists());
    let sub = f.app.submodules(&f.id).unwrap().remove(0);
    assert!(sub.initialized);
    assert!(sub.detached.unwrap());
    assert!(sub.repository_id.is_some());
    assert!(sub.worktree_id.is_some());
}
#[test]
fn restore_uses_index_revision_and_keeps_parent_staged_gitlink() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    git(
        &f.root,
        &[
            "update-index",
            "--cacheinfo",
            &format!("160000,{},module", f.second),
        ],
    );
    let index = fs::read(f.root.join(".git/index")).unwrap();
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "restore")
        .unwrap();
    assert!(
        preview.blocked_reason.is_none(),
        "{:?}",
        preview.blocked_reason
    );
    assert_eq!(
        preview.submodule.head_oid.as_deref(),
        Some(f.first.as_str())
    );
    assert_eq!(preview.submodule.index_oid, f.second);
    assert!(!preview.network_required);
    let receipt = f
        .app
        .update_submodule(&f.id, &module(), "restore", &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    let details = receipt.submodule.as_ref().unwrap();
    assert_eq!(details.target_oid, f.second);
    assert_eq!(details.head_after.as_deref(), Some(f.second.as_str()));
    assert!(details.repository_id.is_some());
    assert_eq!(details.path.display, "module");
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), f.second);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert!(git(&f.root, &["status", "--porcelain"]).starts_with("M  module"));
}
#[test]
fn dirty_and_unretained_detached_commits_are_not_discarded() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    git(&child, &["switch", "--detach", &f.second]);
    let reviewed = f
        .app
        .preview_submodule_update(&f.id, &module(), "restore")
        .unwrap();
    fs::write(child.join("file.txt"), "uncommitted\n").unwrap();
    assert!(f
        .app
        .preview_submodule_update(&f.id, &module(), "restore")
        .unwrap()
        .blocked_reason
        .unwrap()
        .contains("uncommitted"));
    assert_ne!(
        f.app
            .update_submodule(&f.id, &module(), "restore", &reviewed.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        fs::read_to_string(child.join("file.txt")).unwrap(),
        "uncommitted\n"
    );
    git(&child, &["restore", "file.txt"]);
    git(
        &child,
        &["commit", "--allow-empty", "-m", "Unpublished detached work"],
    );
    let unretained = git(&child, &["rev-parse", "HEAD"]);
    let blocked = f
        .app
        .preview_submodule_update(&f.id, &module(), "restore")
        .unwrap();
    assert!(blocked.blocked_reason.unwrap().contains("not retained"));
    git(&child, &["branch", "keep-unpublished", &unretained]);
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "restore")
        .unwrap();
    assert!(preview.blocked_reason.is_none());
    assert_eq!(
        f.app
            .update_submodule(&f.id, &module(), "restore", &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&child, &["rev-parse", "keep-unpublished"]), unretained);
}
#[test]
fn native_locks_changed_gitlinks_and_destination_contents_are_protected() {
    let f = fixture();
    fs::write(f.root.join("module/keep.txt"), "existing content\n").unwrap();
    let blocked = f
        .app
        .preview_submodule_update(&f.id, &module(), "initialize")
        .unwrap();
    assert!(blocked.blocked_reason.unwrap().contains("not empty"));
    fs::remove_file(f.root.join("module/keep.txt")).unwrap();
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "initialize")
        .unwrap();
    fs::write(f.root.join(".git/index.lock"), "external").unwrap();
    assert_ne!(
        f.app
            .update_submodule(&f.id, &module(), "initialize", &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        fs::read_to_string(f.root.join(".git/index.lock")).unwrap(),
        "external"
    );
    fs::remove_file(f.root.join(".git/index.lock")).unwrap();
    git(
        &f.root,
        &[
            "update-index",
            "--cacheinfo",
            &format!("160000,{},module", f.second),
        ],
    );
    assert_ne!(
        f.app
            .update_submodule(&f.id, &module(), "initialize", &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert!(!f.root.join("module/.git").exists());
}
#[test]
fn recursive_discovery_has_stable_parent_child_identities_without_workspace_memberships() {
    let f = fixture();
    initialize(&f);
    let leaf = f._dir.path().join("leaf");
    init(&leaf);
    commit(&leaf, "leaf.txt", "nested\n");
    let child = f.root.join("module");
    git(&child, &["switch", "-c", "nested"]);
    git(
        &child,
        &[
            "submodule",
            "add",
            "--",
            leaf.to_str().unwrap(),
            "nested/leaf",
        ],
    );
    git(&child, &["commit", "-am", "Record nested module"]);
    let first = f.app.submodules(&f.id).unwrap();
    assert_eq!(first.len(), 2);
    let parent = &first[0];
    let nested = &first[1];
    assert_eq!(
        nested.parent_relation_id.as_deref(),
        Some(parent.relation_id.as_str())
    );
    assert_eq!(
        nested.parent_repository_id,
        parent.repository_id.as_ref().unwrap().clone()
    );
    assert_ne!(nested.repository_id, parent.repository_id);
    assert_eq!(
        nested.path.to_path().unwrap(),
        Path::new("module").join("nested").join("leaf")
    );
    assert_eq!(
        nested.relative_path.to_path().unwrap(),
        Path::new("nested").join("leaf")
    );
    assert_eq!(f.app.catalog().unwrap().workspaces[0].memberships.len(), 1);
    let restarted = Application::new(f._dir.path().join("app/state.json")).unwrap();
    let second = restarted.submodules(&f.id).unwrap();
    assert_eq!(first[1].relation_id, second[1].relation_id);
    assert_eq!(first[1].repository_id, second[1].repository_id);
    let before = git(&leaf, &["rev-parse", "HEAD"]);
    let catalog = f.app.open_related(&f.id, &nested.path).unwrap();
    assert_eq!(catalog.workspaces[0].memberships.len(), 2);
    assert_eq!(before, git(&leaf, &["rev-parse", "HEAD"]));
}

#[test]
fn relative_sources_are_previewed_without_network_and_initialized_by_native_git() {
    let f = fixture();
    git(
        &f.root,
        &[
            "config",
            "--file",
            ".gitmodules",
            "submodule.module.url",
            "../source",
        ],
    );
    git(&f.root, &["add", ".gitmodules"]);
    git(&f.root, &["commit", "-m", "Record relative source"]);
    let before = fs::read(f.root.join(".git/config")).unwrap();
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "initialize")
        .unwrap();
    assert_eq!(preview.url.as_deref(), Some("../source"));
    assert!(preview.source_base.as_deref().unwrap().contains("parent"));
    assert_eq!(fs::read(f.root.join(".git/config")).unwrap(), before);
    initialize(&f);
    assert_eq!(git(&f.root.join("module"), &["rev-parse", "HEAD"]), f.first);
}
#[test]
fn cached_metadata_and_hidden_index_entries_require_review_without_discarding_work() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    git(&child, &["switch", "--detach", &f.second]);
    git(&child, &["update-index", "--assume-unchanged", "file.txt"]);
    fs::write(child.join("file.txt"), "hidden local work\n").unwrap();
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "restore")
        .unwrap();
    assert!(preview.blocked_reason.unwrap().contains("hidden"));
    assert_eq!(
        fs::read_to_string(child.join("file.txt")).unwrap(),
        "hidden local work\n"
    );
    git(
        &child,
        &["update-index", "--no-assume-unchanged", "file.txt"],
    );
    git(&child, &["restore", "file.txt"]);
    git(&f.root, &["submodule", "deinit", "-f", "--", "module"]);
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "initialize")
        .unwrap();
    assert!(preview
        .blocked_reason
        .unwrap()
        .contains("Cached module metadata"));
    assert!(f.root.join(".git/modules/module/HEAD").exists());
}
#[test]
fn ignored_child_files_are_not_overwritten_and_failed_initialization_reports_partial_state() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    // Create a retained local revision whose tree adds a file currently ignored by the old revision.
    fs::write(child.join(".gitignore"), "local.key\n").unwrap();
    git(&child, &["add", ".gitignore"]);
    git(&child, &["commit", "-m", "Ignore local key"]);
    let ignored = git(&child, &["rev-parse", "HEAD"]);
    fs::write(child.join("local.key"), "tracked target\n").unwrap();
    git(&child, &["add", "-f", "local.key"]);
    git(&child, &["commit", "-m", "Track key on target"]);
    let target = git(&child, &["rev-parse", "HEAD"]);
    git(&child, &["branch", "keep-target", &target]);
    git(&child, &["switch", "--detach", &ignored]);
    fs::write(child.join("local.key"), "private local content\n").unwrap();
    git(
        &f.root,
        &[
            "update-index",
            "--cacheinfo",
            &format!("160000,{target},module"),
        ],
    );
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "restore")
        .unwrap();
    assert!(
        preview.blocked_reason.is_none(),
        "{:?}",
        preview.blocked_reason
    );
    let receipt = f
        .app
        .update_submodule(&f.id, &module(), "restore", &preview.token)
        .unwrap();
    assert_ne!(receipt.status, "succeeded");
    assert_eq!(
        fs::read_to_string(child.join("local.key")).unwrap(),
        "private local content\n"
    );
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), ignored);
    let other = fixture();
    git(
        &other.root,
        &[
            "config",
            "submodule.module.url",
            other._dir.path().join("missing-source").to_str().unwrap(),
        ],
    );
    let preview = other
        .app
        .preview_submodule_update(&other.id, &module(), "initialize")
        .unwrap();
    let before = fs::read(other.root.join(".git/index")).unwrap();
    let receipt = other
        .app
        .update_submodule(&other.id, &module(), "initialize", &preview.token)
        .unwrap();
    assert_ne!(receipt.status, "succeeded");
    assert!(receipt.recovery.unwrap().contains("partially"));
    assert_eq!(fs::read(other.root.join(".git/index")).unwrap(), before);
}
#[test]
fn initializing_a_parent_discovers_but_does_not_initialize_nested_children() {
    let f = fixture();
    let source = f._dir.path().join("source");
    let leaf = f._dir.path().join("leaf-source");
    init(&leaf);
    commit(&leaf, "file", "leaf\n");
    git(
        &source,
        &["submodule", "add", "--", leaf.to_str().unwrap(), "nested"],
    );
    git(&source, &["commit", "-am", "Add nested relation"]);
    let target = git(&source, &["rev-parse", "HEAD"]);
    git(
        &f.root,
        &[
            "update-index",
            "--cacheinfo",
            &format!("160000,{target},module"),
        ],
    );
    git(&f.root, &["config", "submodule.recurse", "true"]);
    initialize(&f);
    let entries = f.app.submodules(&f.id).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].initialized);
    assert!(!entries[1].initialized);
    let preview = f
        .app
        .preview_submodule_update(&f.id, &entries[1].path, "initialize")
        .unwrap();
    assert!(
        preview.blocked_reason.is_none(),
        "{:?}",
        preview.blocked_reason
    );
    let receipt = f
        .app
        .update_submodule(&f.id, &entries[1].path, "initialize", &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert!(f.app.submodules(&f.id).unwrap()[1].initialized);
}

#[test]
fn linked_worktree_modules_use_their_own_git_admin_directory_and_protect_cached_commits() {
    let f = fixture();
    initialize(&f);
    let linked = f._dir.path().join("linked");
    git(
        &f.root,
        &["worktree", "add", "-b", "linked", linked.to_str().unwrap()],
    );
    let catalog = f.app.add_repository(&linked).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    let preview = f
        .app
        .preview_submodule_update(&id, &module(), "initialize")
        .unwrap();
    assert!(
        preview.blocked_reason.is_none(),
        "{:?}",
        preview.blocked_reason
    );
    let receipt = f
        .app
        .update_submodule(&id, &module(), "initialize", &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    let child = linked.join("module");
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), f.first);
    git(
        &child,
        &[
            "commit",
            "--allow-empty",
            "-m",
            "Unretained linked child commit",
        ],
    );
    let retained_metadata_head = git(&child, &["rev-parse", "HEAD"]);
    let metadata = git(
        &linked,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "modules/module",
        ],
    );
    git(&linked, &["submodule", "deinit", "-f", "--", "module"]);
    let blocked = f
        .app
        .preview_submodule_update(&id, &module(), "initialize")
        .unwrap();
    assert!(blocked
        .blocked_reason
        .unwrap()
        .contains("Cached module metadata"));
    assert_eq!(
        git(&linked, &["--git-dir", &metadata, "rev-parse", "HEAD"]),
        retained_metadata_head
    );
    assert_eq!(git(&f.root.join("module"), &["rev-parse", "HEAD"]), f.first);
}

#[cfg(unix)]
#[test]
fn parent_previews_never_execute_a_child_only_clean_filter() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    git(&child, &["switch", "-c", "retained-filter-check"]);
    fs::write(child.join(".gitattributes"), "file.txt filter=child-only\n").unwrap();
    git(&child, &["add", "."]);
    git(&child, &["commit", "-m", "Record child attributes"]);
    let marker = f._dir.path().join("clean-must-not-run");
    let quoted = format!("'{}'", marker.display().to_string().replace('\'', "'\\''"));
    git(
        &child,
        &[
            "config",
            "filter.child-only.clean",
            &format!("printf invoked > {quoted}; cat"),
        ],
    );
    git(&child, &["config", "filter.child-only.required", "true"]);
    // Same length, changed mtime: native status must hash/filter to assess this working file.
    fs::write(child.join("file.txt"), "other\n").unwrap();
    fs::File::open(child.join("file.txt"))
        .unwrap()
        .set_times(
            fs::FileTimes::new()
                .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2)),
        )
        .unwrap();
    assert!(f.app.status(&f.id).unwrap().dirty);
    assert!(!f.app.working_changes(&f.id).unwrap().files.is_empty());
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "restore")
        .unwrap();
    assert!(preview.blocked_reason.is_some());
    assert!(!marker.exists());
    assert_eq!(
        fs::read_to_string(child.join("file.txt")).unwrap(),
        "other\n"
    );
}

#[test]
fn pointer_stage_unstage_and_parent_commit_preserve_child_changes() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    git(&child, &["switch", "--detach", &f.second]);
    fs::write(child.join("file.txt"), "uncommitted child\n").unwrap();
    fs::write(child.join("notes.txt"), "untracked child\n").unwrap();
    fs::write(f.root.join("unrelated.txt"), "parent work\n").unwrap();
    let child_index = PathBuf::from(git(
        &child,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    ));
    let original_child_index = fs::read(&child_index).unwrap();
    let child_refs = git(&child, &["show-ref", "--head"]);
    let original_parent_index = fs::read(f.root.join(".git/index")).unwrap();
    let preview = f
        .app
        .preview_submodule_pointer(&f.id, &module(), true)
        .unwrap();
    assert!(
        preview.blocked_reason.is_none(),
        "{:?}",
        preview.blocked_reason
    );
    assert_eq!(preview.target_oid, f.second);
    assert_eq!(preview.submodule.dirty, Some(true));
    let receipt = f
        .app
        .change_submodule_pointer(&f.id, &module(), true, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    let backup = receipt
        .submodule
        .unwrap()
        .recovery_index
        .unwrap()
        .to_path()
        .unwrap();
    assert_eq!(fs::read(backup).unwrap(), original_parent_index);
    assert_eq!(git(&f.root, &["rev-parse", ":module"]), f.second);
    let changes = f.app.working_changes(&f.id).unwrap();
    let pointer = changes
        .files
        .iter()
        .find(|file| file.path == module())
        .unwrap();
    assert!(pointer.staged && pointer.unstaged);
    let unstage = f
        .app
        .preview_submodule_pointer(&f.id, &module(), false)
        .unwrap();
    assert_eq!(unstage.target_oid, f.first);
    assert_eq!(
        f.app
            .change_submodule_pointer(&f.id, &module(), false, &unstage.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["rev-parse", ":module"]), f.first);
    let stage = f
        .app
        .preview_submodule_pointer(&f.id, &module(), true)
        .unwrap();
    assert_eq!(
        f.app
            .change_submodule_pointer(&f.id, &module(), true, &stage.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let commit = f.app.commit_preview(&f.id).unwrap();
    assert_eq!(commit.files.len(), 1);
    assert_eq!(commit.files[0].path, module());
    assert!(commit.diff.contains(&f.second));
    assert!(!commit.diff.contains("uncommitted child"));
    assert_eq!(
        f.app
            .create_commit(&f.id, "Update module pointer", &commit.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["rev-parse", "HEAD:module"]), f.second);
    assert_eq!(git(&child, &["show-ref", "--head"]), child_refs);
    assert_eq!(fs::read(child_index).unwrap(), original_child_index);
    assert_eq!(
        fs::read_to_string(child.join("file.txt")).unwrap(),
        "uncommitted child\n"
    );
    assert_eq!(
        fs::read_to_string(child.join("notes.txt")).unwrap(),
        "untracked child\n"
    );
    assert_eq!(
        fs::read_to_string(f.root.join("unrelated.txt")).unwrap(),
        "parent work\n"
    );
}
#[test]
fn pointer_previews_reject_stale_child_parent_pending_operations_and_hidden_flags() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    git(&child, &["switch", "--detach", &f.second]);
    let preview = f
        .app
        .preview_submodule_pointer(&f.id, &module(), true)
        .unwrap();
    let original = fs::read(f.root.join(".git/index")).unwrap();
    git(&child, &["switch", "--detach", &f.first]);
    assert!(f
        .app
        .change_submodule_pointer(&f.id, &module(), true, &preview.token)
        .is_err());
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), original);
    git(&child, &["switch", "--detach", &f.second]);
    let preview = f
        .app
        .preview_submodule_pointer(&f.id, &module(), true)
        .unwrap();
    fs::write(f.root.join("parent.txt"), "parent staged\n").unwrap();
    git(&f.root, &["add", "parent.txt"]);
    let changed = fs::read(f.root.join(".git/index")).unwrap();
    assert!(f
        .app
        .change_submodule_pointer(&f.id, &module(), true, &preview.token)
        .is_err());
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), changed);
    let preview = f
        .app
        .preview_submodule_pointer(&f.id, &module(), true)
        .unwrap();
    fs::write(f.root.join(".git/index.lock"), "external owner").unwrap();
    assert_ne!(
        f.app
            .change_submodule_pointer(&f.id, &module(), true, &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        fs::read_to_string(f.root.join(".git/index.lock")).unwrap(),
        "external owner"
    );
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), changed);
    fs::remove_file(f.root.join(".git/index.lock")).unwrap();
    fs::write(
        f.root.join(".git/MERGE_HEAD"),
        git(&f.root, &["rev-parse", "HEAD"]),
    )
    .unwrap();
    assert!(f
        .app
        .preview_submodule_pointer(&f.id, &module(), true)
        .is_err());
    fs::remove_file(f.root.join(".git/MERGE_HEAD")).unwrap();
    git(&f.root, &["update-index", "--skip-worktree", "module"]);
    assert!(f
        .app
        .preview_submodule_pointer(&f.id, &module(), true)
        .is_err());
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), f.second);
}
#[test]
fn dirty_files_without_a_new_child_commit_are_not_parent_pointer_changes() {
    let f = fixture();
    let missing = f
        .app
        .preview_submodule_pointer(&f.id, &module(), true)
        .unwrap();
    assert!(missing.blocked_reason.is_some());
    assert!(!f.root.join("module/.git").exists());
    initialize(&f);
    fs::write(f.root.join("module/file.txt"), "only dirty files\n").unwrap();
    let preview = f
        .app
        .preview_submodule_pointer(&f.id, &module(), true)
        .unwrap();
    assert!(preview
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("No new submodule commit"));
    let before = fs::read(f.root.join(".git/index")).unwrap();
    assert_ne!(
        f.app
            .change_submodule_pointer(&f.id, &module(), true, &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), before);
}

#[test]
fn branch_display_snapshot_follows_head_and_index_without_moving_child_or_pending_operation() {
    let f = fixture();
    let original = f.app.submodule_snapshot(&f.id).unwrap();
    assert_eq!(original.status.branch.as_deref(), Some("main"));
    assert_eq!(original.submodules.len(), 1);
    assert!(!original.submodules[0].initialized);
    initialize(&f);
    let child = f.root.join("module");
    git(&f.root, &["switch", "-c", "develop"]);
    git(&child, &["checkout", "--detach", &f.second]);
    git(&f.root, &["add", "module"]);
    git(&f.root, &["commit", "-m", "Record develop child"]);
    git(&child, &["checkout", "--detach", &f.first]);
    let pending = format!("{}\n", f.first);
    fs::write(f.root.join(".git/MERGE_HEAD"), &pending).unwrap();
    let index = fs::read(f.root.join(".git/index")).unwrap();
    let snapshot = f.app.submodule_snapshot(&f.id).unwrap();
    assert_eq!(snapshot.status.branch.as_deref(), Some("develop"));
    assert_eq!(
        snapshot.submodules[0].head_oid.as_deref(),
        Some(f.second.as_str())
    );
    assert_eq!(snapshot.submodules[0].index_oid, f.second);
    assert_eq!(
        snapshot.submodules[0].checked_out_oid.as_deref(),
        Some(f.first.as_str())
    );
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(f.root.join(".git/MERGE_HEAD")).unwrap(),
        pending
    );
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), f.first);
    fs::remove_file(f.root.join(".git/MERGE_HEAD")).unwrap();
    git(&f.root, &["switch", "main"]);
    let snapshot = f.app.submodule_snapshot(&f.id).unwrap();
    assert_eq!(snapshot.status.branch.as_deref(), Some("main"));
    assert_eq!(
        snapshot.submodules[0].head_oid.as_deref(),
        Some(f.first.as_str())
    );
    assert_eq!(
        snapshot.submodules[0].checked_out_oid.as_deref(),
        Some(f.first.as_str())
    );
    git(&f.root, &["switch", "-c", "without-module"]);
    git(&f.root, &["rm", "--cached", "module"]);
    git(&f.root, &["commit", "-m", "Remove recorded module"]);
    assert!(f
        .app
        .submodule_snapshot(&f.id)
        .unwrap()
        .submodules
        .is_empty());
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), f.first);
}

#[test]
fn cached_remote_update_keeps_parent_index_and_local_branches_and_exposes_pointer_change() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    git(
        &f.root,
        &[
            "config",
            "submodule.module.update",
            "!touch CUSTOM_UPDATE_MUST_NOT_RUN",
        ],
    );
    fs::write(f.root.join("pending.txt"), "keep unstaged\n").unwrap();
    fs::write(f.root.join("prepared.txt"), "keep staged\n").unwrap();
    git(&f.root, &["add", "prepared.txt"]);
    let index = fs::read(f.root.join(".git/index")).unwrap();
    let refs = git(
        &child,
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/heads",
        ],
    );
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(preview.blocked_reason.is_none(), "{preview:?}");
    assert_eq!(preview.target_oid, f.second);
    assert_eq!(
        preview.remote_ref.as_deref(),
        Some("refs/remotes/origin/main")
    );
    assert!(!preview.network_required);
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), f.first);
    assert!(!f.root.join("CUSTOM_UPDATE_MUST_NOT_RUN").exists());
    let receipt = f
        .app
        .update_submodule(&f.id, &module(), "remote", &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{receipt:?}");
    assert_eq!(receipt.kind, "update_submodule_remote");
    assert_eq!(receipt.submodule.unwrap().target_oid, f.second);
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), f.second);
    assert_eq!(git(&child, &["rev-parse", "--abbrev-ref", "HEAD"]), "HEAD");
    assert_eq!(
        git(
            &child,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/heads"
            ]
        ),
        refs
    );
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
    assert_eq!(git(&f.root, &["show", ":prepared.txt"]), "keep staged");
    assert_eq!(
        fs::read_to_string(f.root.join("pending.txt")).unwrap(),
        "keep unstaged\n"
    );
    assert!(!f.root.join("CUSTOM_UPDATE_MUST_NOT_RUN").exists());
    let sub = f.app.submodules(&f.id).unwrap().remove(0);
    assert_eq!(sub.index_oid, f.first);
    assert_eq!(sub.checked_out_oid.as_deref(), Some(f.second.as_str()));
    let again = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(again.blocked_reason.unwrap().contains("already matches"));
}
#[test]
fn cached_remote_update_refuses_dirty_divergent_and_stale_child_state() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(preview.blocked_reason.is_none(), "{preview:?}");
    fs::write(child.join("file.txt"), "keep private work\n").unwrap();
    assert!(f
        .app
        .update_submodule(&f.id, &module(), "remote", &preview.token)
        .is_ok_and(|r| r.status != "succeeded"));
    let dirty = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(dirty.blocked_reason.unwrap().contains("uncommitted"));
    git(&child, &["add", "file.txt"]);
    git(&child, &["commit", "-m", "private child commit"]);
    let private = git(&child, &["rev-parse", "HEAD"]);
    let diverged = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(diverged
        .blocked_reason
        .unwrap()
        .contains("not a descendant"));
    assert!(f
        .app
        .update_submodule(&f.id, &module(), "remote", &diverged.token)
        .is_ok_and(|r| r.status != "succeeded"));
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), private);
}
#[test]
fn cached_remote_target_changes_invalidate_review_and_missing_ref_cannot_fall_back_to_recorded() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    let preview = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    git(
        &child,
        &["update-ref", "refs/remotes/origin/main", &f.first],
    );
    assert!(f
        .app
        .update_submodule(&f.id, &module(), "remote", &preview.token)
        .is_ok_and(|r| r.status != "succeeded"));
    git(&child, &["update-ref", "-d", "refs/remotes/origin/main"]);
    let missing = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(missing.blocked_reason.is_some(), "{missing:?}");
    assert!(missing.remote_ref.is_none());
    assert!(f
        .app
        .update_submodule(&f.id, &module(), "remote", &missing.token)
        .is_ok_and(|r| r.status != "succeeded"));
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), f.first);
}
#[test]
fn configured_branch_local_override_and_parent_branch_inheritance_are_explicit() {
    let f = fixture();
    initialize(&f);
    git(
        &f.root,
        &[
            "config",
            "-f",
            ".gitmodules",
            "submodule.module.branch",
            "missing",
        ],
    );
    git(&f.root, &["add", ".gitmodules"]);
    git(&f.root, &["commit", "-m", "configure child branch"]);
    let missing = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(missing.blocked_reason.is_some());
    git(&f.root, &["config", "submodule.module.branch", "."]);
    let inherited = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(inherited.blocked_reason.is_none(), "{inherited:?}");
    assert_eq!(inherited.target_oid, f.second);
    git(&f.root, &["switch", "--detach"]);
    let detached = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(detached.blocked_reason.is_some());
    assert!(detached.remote_ref.is_none());
    git(&f.root, &["config", "submodule.module.branch", "main"]);
    let explicit = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(explicit.blocked_reason.is_none(), "{explicit:?}");
    assert_eq!(
        explicit.remote_ref.as_deref(),
        Some("refs/remotes/origin/main")
    );
}

#[test]
fn newer_child_revision_with_nested_layout_changes_requires_separate_review() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    git(&child, &["switch", "--detach", &f.second]);
    fs::write(
        child.join(".gitmodules"),
        "[submodule \"nested\"]\n\tpath = nested\n\turl = https://example.invalid/nested.git\n",
    )
    .unwrap();
    git(&child, &["add", ".gitmodules"]);
    git(&child, &["commit", "-m", "nested layout"]);
    let tip = git(&child, &["rev-parse", "HEAD"]);
    git(&child, &["update-ref", "refs/remotes/origin/main", &tip]);
    git(&child, &["switch", "--detach", &f.first]);
    let p = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(
        p.blocked_reason
            .as_deref()
            .is_some_and(|s| s.contains("nested submodule")),
        "{p:?}"
    );
    assert!(f
        .app
        .update_submodule(&f.id, &module(), "remote", &p.token)
        .is_ok_and(|r| r.status != "succeeded"));
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), f.first);
    assert!(!child.join(".gitmodules").exists());
}

#[test]
fn cached_remote_update_refuses_grafted_history_before_ancestry_authorization() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    let grafts = PathBuf::from(git(
        &child,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "info/grafts",
        ],
    ));
    fs::write(&grafts, "").unwrap();
    let p = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(
        p.blocked_reason
            .as_deref()
            .is_some_and(|r| r.contains("grafted")),
        "{p:?}"
    );
    assert!(p.remote_ref.is_none());
    assert!(f
        .app
        .update_submodule(&f.id, &module(), "remote", &p.token)
        .is_ok_and(|r| r.status != "succeeded"));
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), f.first);
}

#[test]
fn configured_child_remote_matches_native_no_fetch_update_and_never_moves_local_branch() {
    let f = fixture();
    initialize(&f);
    let child = f.root.join("module");
    git(&child, &["switch", "-c", "topic", &f.first]);
    git(&child, &["remote", "rename", "origin", "mirror"]);
    git(&child, &["config", "branch.topic.remote", "mirror"]);
    git(&f.root, &["config", "submodule.module.branch", "main"]);
    // A deliberately unreachable URL proves these checks use cached refs, not a fetch.
    git(
        &child,
        &[
            "config",
            "remote.mirror.url",
            "https://example.invalid/not-contacted.git",
        ],
    );
    let p = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    assert!(p.blocked_reason.is_none(), "{p:?}");
    assert_eq!(p.remote_ref.as_deref(), Some("refs/remotes/mirror/main"));
    git(
        &f.root,
        &[
            "submodule",
            "update",
            "--remote",
            "--no-fetch",
            "--checkout",
            "--",
            "module",
        ],
    );
    let native_target = git(&child, &["rev-parse", "HEAD"]);
    assert_eq!(native_target, p.target_oid);
    git(&child, &["switch", "topic"]);
    let before = fs::read(f.root.join(".git/index")).unwrap();
    let p = f
        .app
        .preview_submodule_update(&f.id, &module(), "remote")
        .unwrap();
    let receipt = f
        .app
        .update_submodule(&f.id, &module(), "remote", &p.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{receipt:?}");
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), native_target);
    assert_eq!(git(&child, &["rev-parse", "topic"]), f.first);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), before);
}
