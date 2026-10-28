use gitnova_core::Application;
use gitnova_domain::*;
use std::{fs, path::Path, process::Command};
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim_end().into()
}
fn setup() -> (tempfile::TempDir, Application, std::path::PathBuf, String) {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "user.name", "GitNova Test"]);
    git(&repo, &["config", "user.email", "test@gitnova.invalid"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    git(&repo, &["config", "core.hooksPath", ".git/hooks"]);
    git(
        &repo,
        &[
            "config",
            "core.attributesFile",
            ".git/gitnova-empty-attributes",
        ],
    );
    fs::write(repo.join("file.txt"), "original\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Initial"]);
    let app = Application::new(d.path().join("app/state.json")).unwrap();
    let c = app.add_repository(&repo).unwrap();
    let id = c.workspaces[0].selected_id.clone().unwrap();
    (d, app, repo, id)
}
fn file() -> NativePath {
    NativePath::new(Path::new("file.txt"))
}
#[test]
fn stage_unstage_preserve_working_files_and_unrelated_staged_content() {
    let (d, app, repo, id) = setup();
    fs::write(repo.join("other.txt"), "already staged\n").unwrap();
    git(&repo, &["add", "other.txt"]);
    fs::write(repo.join("file.txt"), "modified\n").unwrap();
    let original_head = git(&repo, &["rev-parse", "HEAD"]);
    let preview = app.file_preview(&id, &file(), false).unwrap();
    assert!(preview.diff.contains("+modified"));
    let receipt = app
        .change_index(&id, &file(), true, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(git(&repo, &["show", ":file.txt"]), "modified");
    assert_eq!(git(&repo, &["show", ":other.txt"]), "already staged");
    assert!(d
        .path()
        .join("app/operations")
        .join(format!("{}.json", receipt.id))
        .exists());
    let preview = app.file_preview(&id, &file(), true).unwrap();
    let receipt = app
        .change_index(&id, &file(), false, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "modified\n"
    );
    assert_eq!(git(&repo, &["show", ":file.txt"]), "original");
    assert_eq!(git(&repo, &["show", ":other.txt"]), "already staged");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original_head);
    assert!(!repo.join(".git/index.lock").exists());
}
#[test]
fn stale_file_and_index_previews_are_rejected_without_publishing() {
    let (_directory, app, repo, id) = setup();
    fs::write(repo.join("file.txt"), "version one\n").unwrap();
    let preview = app.file_preview(&id, &file(), false).unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    fs::write(repo.join("file.txt"), "version two\n").unwrap();
    let result = app
        .change_index(&id, &file(), true, &preview.token)
        .unwrap();
    assert_eq!(result.status, "failed");
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    let preview = app.file_preview(&id, &file(), false).unwrap();
    fs::write(repo.join("external"), "staged externally").unwrap();
    git(&repo, &["add", "external"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    let result = app
        .change_index(&id, &file(), true, &preview.token)
        .unwrap();
    assert_eq!(result.status, "failed");
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
}
#[test]
fn existing_git_locks_are_respected_and_never_removed() {
    let (_directory, app, repo, id) = setup();
    fs::write(repo.join("file.txt"), "change\n").unwrap();
    let preview = app.file_preview(&id, &file(), false).unwrap();
    fs::write(repo.join(".git/index.lock"), "external lock").unwrap();
    let result = app
        .change_index(&id, &file(), true, &preview.token)
        .unwrap();
    assert_eq!(result.status, "failed");
    assert_eq!(
        fs::read_to_string(repo.join(".git/index.lock")).unwrap(),
        "external lock"
    );
    fs::write(
        repo.join(".git/gitnova-operation.lock"),
        "another GitNova instance",
    )
    .unwrap();
    assert_eq!(
        app.change_index(&id, &file(), true, &preview.token)
            .unwrap_err()
            .code,
        "locked"
    );
    assert!(repo.join(".git/gitnova-operation.lock").exists());
}
#[test]
fn commit_uses_reviewed_index_preserves_unstaged_edits_and_writes_receipt() {
    let (_directory, app, repo, id) = setup();
    fs::write(repo.join("file.txt"), "staged version\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    fs::write(repo.join("file.txt"), "later unstaged version\n").unwrap();
    let preview = app.commit_preview(&id).unwrap();
    assert!(preview.diff.contains("+staged version"));
    assert!(!preview.diff.contains("later unstaged"));
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let receipt = app
        .create_commit(&id, "Reviewed commit\n\nA clear reason.", &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(receipt.head_before.as_deref(), Some(head.as_str()));
    assert_ne!(receipt.head_before, receipt.head_after);
    assert_eq!(git(&repo, &["show", "HEAD:file.txt"]), "staged version");
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "later unstaged version\n"
    );
    assert_eq!(
        git(&repo, &["log", "-1", "--format=%B"]),
        "Reviewed commit\n\nA clear reason."
    );
}
#[test]
fn changed_head_or_staged_content_invalidates_commit_review() {
    let (_directory, app, repo, id) = setup();
    fs::write(repo.join("file.txt"), "v1\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    let preview = app.commit_preview(&id).unwrap();
    fs::write(repo.join("file.txt"), "v2\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let result = app
        .create_commit(&id, "Must not commit", &preview.token)
        .unwrap();
    assert_eq!(result.status, "failed");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    let preview = app.commit_preview(&id).unwrap();
    git(&repo, &["commit", "--allow-empty", "-m", "External commit"]);
    let result = app
        .create_commit(&id, "Stale head", &preview.token)
        .unwrap();
    assert_eq!(result.status, "failed");
    assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "External commit");
}
#[test]
fn first_commit_unstage_deletion_and_literal_special_paths_work() {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    git(&repo, &["config", "core.hooksPath", ".git/hooks"]);
    git(
        &repo,
        &[
            "config",
            "core.attributesFile",
            ".git/gitnova-empty-attributes",
        ],
    );
    let app = Application::new(d.path().join("app/state.json")).unwrap();
    let c = app.add_repository(&repo).unwrap();
    let id = c.workspaces[0].selected_id.clone().unwrap();
    let name = "--odd [name] file.txt";
    let path = NativePath::new(Path::new(name));
    fs::write(repo.join(name), "first\n").unwrap();
    let preview = app.file_preview(&id, &path, false).unwrap();
    assert_eq!(
        app.change_index(&id, &path, true, &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let preview = app.file_preview(&id, &path, true).unwrap();
    assert_eq!(
        app.change_index(&id, &path, false, &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert!(repo.join(name).exists());
    let preview = app.file_preview(&id, &path, false).unwrap();
    assert_eq!(
        app.change_index(&id, &path, true, &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let review = app.commit_preview(&id).unwrap();
    let result = app
        .create_commit(&id, "First commit", &review.token)
        .unwrap();
    assert_eq!(result.status, "succeeded", "{}", result.summary);
    fs::remove_file(repo.join(name)).unwrap();
    let preview = app.file_preview(&id, &path, false).unwrap();
    assert_eq!(
        app.change_index(&id, &path, true, &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let preview = app.file_preview(&id, &path, true).unwrap();
    assert_eq!(
        app.change_index(&id, &path, false, &preview.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert!(!repo.join(name).exists());
    assert_eq!(git(&repo, &["show", &format!(":{name}")]), "first");
}
#[test]
fn operation_in_progress_and_path_traversal_are_refused() {
    let (_directory, app, repo, id) = setup();
    fs::write(repo.join("file.txt"), "new\n").unwrap();
    let preview = app.file_preview(&id, &file(), false).unwrap();
    fs::write(
        repo.join(".git/MERGE_HEAD"),
        git(&repo, &["rev-parse", "HEAD"]),
    )
    .unwrap();
    assert_eq!(
        app.change_index(&id, &file(), true, &preview.token)
            .unwrap()
            .status,
        "failed"
    );
    assert!(app
        .file_preview(&id, &NativePath::new(Path::new("../outside")), false)
        .is_err());
    assert!(app.create_commit(&id, "", &preview.token).unwrap().status != "succeeded");
}
#[cfg(unix)]
#[test]
fn configured_commit_hooks_run_and_failure_does_not_commit() {
    use std::os::unix::fs::PermissionsExt;
    let (_directory, app, repo, id) = setup();
    fs::write(repo.join("file.txt"), "staged\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    let hook = repo.join(".git/hooks/pre-commit");
    fs::write(
        &hook,
        "#!/bin/sh\necho hook-ran > hook-marker\necho hook-rejected >&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let preview = app.commit_preview(&id).unwrap();
    assert!(!repo.join("hook-marker").exists());
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let result = app
        .create_commit(&id, "Blocked by hook", &preview.token)
        .unwrap();
    assert_eq!(result.status, "failed");
    assert!(result.summary.contains("hook-rejected"));
    assert!(repo.join("hook-marker").exists());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&repo, &["show", ":file.txt"]), "staged");
    assert!(!repo.join(".git/index.lock").exists());
}
#[cfg(unix)]
#[test]
fn failing_clean_filter_preserves_original_index_and_never_runs_in_preview() {
    use std::os::unix::fs::PermissionsExt;
    let (_directory, app, repo, id) = setup();
    let filter = repo.join("filter.sh");
    fs::write(&filter, "#!/bin/sh\necho ran > filter-marker\nexit 1\n").unwrap();
    fs::set_permissions(&filter, fs::Permissions::from_mode(0o755)).unwrap();
    git(&repo, &["config", "filter.fail.clean", "./filter.sh"]);
    git(&repo, &["config", "filter.fail.required", "true"]);
    fs::write(repo.join(".gitattributes"), "file.txt filter=fail\n").unwrap();
    fs::write(repo.join("file.txt"), "new\n").unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    let preview = app.file_preview(&id, &file(), false).unwrap();
    assert!(!repo.join("filter-marker").exists());
    let result = app
        .change_index(&id, &file(), true, &preview.token)
        .unwrap();
    assert_eq!(result.status, "failed");
    assert!(repo.join("filter-marker").exists());
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
}
#[test]
fn snapshots_detect_external_changes_without_mutating_index() {
    let (_directory, app, repo, id) = setup();
    let before = app.snapshot(&id).unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    fs::write(repo.join("file.txt"), "modified externally\n").unwrap();
    let after = app.snapshot(&id).unwrap();
    assert_eq!(before.fingerprint, after.fingerprint);
    assert_ne!(before.changes.token, after.changes.token);
    assert!(after.changes.status.dirty);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
}

#[test]
fn deleted_nested_file_can_be_staged_without_recreating_its_directory() {
    let (_directory, app, repo, id) = setup();
    fs::create_dir(repo.join("nested")).unwrap();
    fs::write(repo.join("nested/gone.txt"), "tracked\n").unwrap();
    git(&repo, &["add", "nested/gone.txt"]);
    git(&repo, &["commit", "-m", "Add nested file"]);
    fs::remove_dir_all(repo.join("nested")).unwrap();
    let path = NativePath::new(Path::new("nested/gone.txt"));
    let preview = app.file_preview(&id, &path, false).unwrap();
    let result = app.change_index(&id, &path, true, &preview.token).unwrap();
    assert_eq!(result.status, "succeeded", "{}", result.summary);
    assert!(!repo.join("nested").exists());
    assert_eq!(
        git(&repo, &["diff", "--cached", "--name-only"]),
        "nested/gone.txt"
    );
}
#[test]
fn complete_staged_manifest_allows_bounded_code_preview_for_large_commits() {
    let (_directory, app, repo, id) = setup();
    fs::write(repo.join("large.txt"), "a".repeat(600_000)).unwrap();
    git(&repo, &["add", "large.txt"]);
    let preview = app.commit_preview(&id).unwrap();
    assert!(preview.truncated);
    let before = git(&repo, &["rev-parse", "HEAD"]);
    let result = app
        .create_commit(&id, "Incomplete review", &preview.token)
        .unwrap();
    assert!(preview.complete_manifest);
    assert_eq!(result.status, "succeeded");
    assert_eq!(git(&repo, &["show", "-s", "--format=%P", "HEAD"]), before);
}

#[test]
fn staged_gitlink_removal_is_not_silently_included_in_a_commit() {
    let (_directory, app, repo, id) = setup();
    let oid = git(&repo, &["rev-parse", "HEAD"]);
    git(
        &repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{oid},module"),
        ],
    );
    git(&repo, &["commit", "-m", "Record gitlink"]);
    git(&repo, &["rm", "--cached", "module"]);
    let changes = app.working_changes(&id).unwrap();
    let module = changes
        .files
        .iter()
        .find(|f| f.path.display == "module")
        .unwrap();
    assert!(module.blocked_reason.is_some());
    assert_eq!(
        app.commit_preview(&id).unwrap_err().code,
        "unsupported_write"
    );
}

#[cfg(unix)]
#[test]
fn diff_paths_preserve_raw_names_and_override_display_prefix_config() {
    use std::os::unix::ffi::OsStringExt;
    let (_directory, app, repo, id) = setup();
    let name = std::ffi::OsString::from_vec(b"space \"tab\tline\n.txt".to_vec());
    let file = NativePath::new(Path::new(&name));
    fs::write(repo.join(&name), "new file\n").unwrap();
    let preview = app.file_preview(&id, &file, false).unwrap();
    assert!(preview.diff.starts_with("diff --git \"\\141\\057"));
    assert_eq!(preview.diff.matches("\ndiff --git ").count(), 0);
    assert!(!preview.diff.contains("line\n"));
    git(&repo, &["config", "diff.noprefix", "true"]);
    git(&repo, &["config", "diff.mnemonicPrefix", "true"]);
    let receipt = app.change_index(&id, &file, true, &preview.token).unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    let staged = app.commit_preview(&id).unwrap();
    assert!(staged.diff.contains("+++ \"b/"));
    assert!(staged.diff.contains("\\t"));
    assert!(staged.diff.contains("\\n"));
    assert_eq!(staged.files[0].path.hex, file.hex);
}

#[test]
fn staging_same_size_edits_preserves_racy_index_detection_when_stat_fields_match() {
    use std::time::{Duration, SystemTime};
    let (_temp, app, repo, id) = setup();
    git(&repo, &["config", "core.trustctime", "false"]);
    git(&repo, &["config", "core.checkStat", "minimal"]);
    fs::write(repo.join("other.txt"), "unrelated staged\n").unwrap();
    git(&repo, &["add", "other.txt"]);
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    let set_time = |path: &Path| {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(stamp))
            .unwrap();
    };
    // Record a coarse timestamp in the index, then keep it equal to the index's
    // own timestamp. Git must hash a same-size edit in this racy interval.
    set_time(&repo.join("file.txt"));
    git(&repo, &["add", "file.txt"]);
    set_time(&repo.join(".git/index"));
    fs::write(repo.join("file.txt"), "modified\n").unwrap();
    set_time(&repo.join("file.txt"));
    let review = app.file_preview(&id, &file(), false).unwrap();
    assert!(review.diff.contains("+modified"));
    let receipt = app.change_index(&id, &file(), true, &review.token).unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(git(&repo, &["show", ":file.txt"]), "modified");
    assert_eq!(git(&repo, &["show", ":other.txt"]), "unrelated staged");
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "modified\n"
    );
}
