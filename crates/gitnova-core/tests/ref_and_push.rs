use gitnova_core::Application;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
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
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim_end().into()
}
fn setup() -> (tempfile::TempDir, Application, PathBuf, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "GitNova Test"),
        ("user.email", "test@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", ".git/hooks"),
        ("core.attributesFile", ".git/no-global-attributes"),
    ] {
        git(&repo, &["config", key, value]);
    }
    fs::write(repo.join("file.txt"), "main\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Main"]);
    git(&repo, &["switch", "-c", "feature"]);
    fs::write(repo.join("file.txt"), "feature\n").unwrap();
    git(&repo, &["commit", "-am", "Feature"]);
    git(&repo, &["switch", "main"]);
    let app = Application::new(directory.path().join("app/state.json")).unwrap();
    let catalog = app.add_repository(&repo).unwrap();
    let id = catalog.workspaces[0].selected_id.clone().unwrap();
    (directory, app, repo, id)
}

use gitnova_domain::*;
fn ref_action(kind: RefOperationKind, name: &str, oid: &str) -> RefOperation {
    RefOperation {
        kind,
        name: name.into(),
        oid: oid.into(),
        new_name: String::new(),
        remote: String::new(),
        remote_branch: String::new(),
    }
}
fn push_request(oid: &str) -> PushRequest {
    PushRequest {
        branch: "feature".into(),
        oid: oid.into(),
        remote: "origin".into(),
        remote_branch: "main".into(),
        set_upstream: false,
        buffered_https: false,
    }
}
fn remote(repo: &Path, folder: &Path) -> PathBuf {
    let bare = folder.join("remote.git");
    git(folder, &["init", "--bare", bare.to_str().unwrap()]);
    git(repo, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git(repo, &["push", "origin", "main"]);
    bare
}
fn remote_oid(bare: &Path, name: &str) -> String {
    git(bare, &["rev-parse", name])
}
#[test]
fn rename_current_branch_preserves_dirty_files_index_upstream_config_and_reflog() {
    let (temp, app, repo, id) = setup();
    let _bare = remote(&repo, temp.path());
    git(&repo, &["branch", "--set-upstream-to=origin/main", "main"]);
    git(
        &repo,
        &["config", "branch.main.description", "Keep description"],
    );
    fs::write(repo.join("file.txt"), "dirty kept\n").unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    let oid = git(&repo, &["rev-parse", "HEAD"]);
    let mut request = ref_action(RefOperationKind::RenameBranch, "main", &oid);
    request.new_name = "renamed/main".into();
    let review = app.preview_ref_operation(&id, &request).unwrap();
    assert!(review.blocked_reason.is_none());
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
    let result = app
        .apply_ref_operation(&id, &request, &review.token)
        .unwrap();
    assert_eq!(result.status, "succeeded", "{}", result.summary);
    assert_eq!(git(&repo, &["branch", "--show-current"]), "renamed/main");
    assert_eq!(
        git(&repo, &["config", "branch.renamed/main.description"]),
        "Keep description"
    );
    assert_eq!(
        git(&repo, &["rev-parse", "--symbolic-full-name", "@{upstream}"]),
        "refs/remotes/origin/main"
    );
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "dirty kept\n"
    );
    assert!(git(&repo, &["reflog", "show", "renamed/main"]).contains("renamed"));
}
#[test]
fn deleting_unmerged_branch_preserves_tip_in_recovery_and_excludes_internal_refs_from_graph() {
    let (_temp, app, repo, id) = setup();
    let oid = git(&repo, &["rev-parse", "feature"]);
    let request = ref_action(RefOperationKind::DeleteBranch, "feature", &oid);
    let review = app.preview_ref_operation(&id, &request).unwrap();
    assert!(review.retained_by.is_empty());
    let backup = review.recovery_ref.clone().unwrap();
    assert!(!repo.join(".git").join(&backup).exists());
    let result = app
        .apply_ref_operation(&id, &request, &review.token)
        .unwrap();
    assert_eq!(result.status, "succeeded", "{}", result.summary);
    assert!(result.planned.is_some());
    assert_eq!(git(&repo, &["rev-parse", &backup]), oid);
    assert!(git(&repo, &["branch", "--list", "feature"]).is_empty());
    assert!(!app
        .graph(&id, None)
        .unwrap()
        .rows
        .iter()
        .any(|r| r.commit.oid == oid));
    assert_eq!(app.detail(&id, &oid).unwrap().commit.oid, oid);
}
#[test]
fn deleting_annotated_tag_retains_exact_annotation_object_and_does_not_delete_remote_tags() {
    let (_temp, app, repo, id) = setup();
    git(
        &repo,
        &[
            "-c",
            "tag.gpgSign=false",
            "tag",
            "-a",
            "release/test",
            "-m",
            "Important annotation",
            "feature",
        ],
    );
    let oid = git(&repo, &["rev-parse", "feature"]);
    let object = git(&repo, &["rev-parse", "refs/tags/release/test"]);
    let request = ref_action(RefOperationKind::DeleteTag, "release/test", &oid);
    let review = app.preview_ref_operation(&id, &request).unwrap();
    assert_eq!(review.object_oid, object);
    assert!(review
        .annotation
        .as_ref()
        .unwrap()
        .contains("Important annotation"));
    let result = app
        .apply_ref_operation(&id, &request, &review.token)
        .unwrap();
    assert_eq!(result.status, "succeeded", "{}", result.summary);
    let backup = review.recovery_ref.unwrap();
    assert_eq!(git(&repo, &["rev-parse", &backup]), object);
    assert!(git(&repo, &["cat-file", "-p", &backup]).contains("Important annotation"));
    assert!(git(&repo, &["tag", "--list"]).is_empty());
}
#[test]
fn ref_occupancy_collisions_config_changes_and_recovery_collisions_stop_writes() {
    let (temp, app, repo, id) = setup();
    let main = git(&repo, &["rev-parse", "main"]);
    let feature = git(&repo, &["rev-parse", "feature"]);
    assert!(app
        .preview_ref_operation(
            &id,
            &ref_action(RefOperationKind::DeleteBranch, "main", &main)
        )
        .unwrap()
        .blocked_reason
        .is_some());
    let mut rename = ref_action(RefOperationKind::RenameBranch, "main", &main);
    rename.new_name = "feature".into();
    assert!(app
        .preview_ref_operation(&id, &rename)
        .unwrap()
        .blocked_reason
        .is_some());
    rename.new_name = "safe-name".into();
    let review = app.preview_ref_operation(&id, &rename).unwrap();
    git(&repo, &["config", "branch.main.description", "changed"]);
    assert!(app
        .apply_ref_operation(&id, &rename, &review.token)
        .is_err());
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
    let tree = temp.path().join("linked");
    git(
        &repo,
        &["worktree", "add", tree.to_str().unwrap(), "feature"],
    );
    let delete = ref_action(RefOperationKind::DeleteBranch, "feature", &feature);
    assert!(app
        .preview_ref_operation(&id, &delete)
        .unwrap()
        .blocked_reason
        .is_some());
    git(&repo, &["worktree", "remove", tree.to_str().unwrap()]);
    let review = app.preview_ref_operation(&id, &delete).unwrap();
    let backup = review.recovery_ref.unwrap();
    git(&repo, &["update-ref", &backup, &main]);
    assert!(app
        .preview_ref_operation(&id, &delete)
        .unwrap()
        .blocked_reason
        .is_some());
    assert_eq!(git(&repo, &["rev-parse", "feature"]), feature);
}
#[test]
fn changing_upstream_preserves_checkout_and_can_unset_without_fetch_or_pull() {
    let (temp, app, repo, id) = setup();
    let bare = remote(&repo, temp.path());
    let oid = git(&repo, &["rev-parse", "feature"]);
    let mut request = ref_action(RefOperationKind::SetUpstream, "feature", &oid);
    request.remote = "origin".into();
    request.remote_branch = "main".into();
    let review = app.preview_ref_operation(&id, &request).unwrap();
    assert!(review.upstream_before.is_none());
    assert_eq!(
        app.apply_ref_operation(&id, &request, &review.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&repo, &["config", "branch.feature.remote"]), "origin");
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
    assert_eq!(
        remote_oid(&bare, "main"),
        git(&repo, &["rev-parse", "main"])
    );
    let request = ref_action(RefOperationKind::UnsetUpstream, "feature", &oid);
    let review = app.preview_ref_operation(&id, &request).unwrap();
    assert_eq!(
        app.apply_ref_operation(&id, &request, &review.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert!(git(
        &repo,
        &["for-each-ref", "--format=%(upstream)", "refs/heads/feature"]
    )
    .is_empty());
}
#[test]
fn push_is_pinned_and_single_branch_ignoring_matching_forced_refspecs_and_follow_tags() {
    let (temp, app, repo, id) = setup();
    let bare = remote(&repo, temp.path());
    let oid = git(&repo, &["rev-parse", "feature"]);
    git(&repo, &["config", "push.default", "matching"]);
    git(
        &repo,
        &["config", "remote.origin.push", "+refs/heads/*:refs/heads/*"],
    );
    git(&repo, &["config", "push.followTags", "true"]);
    git(
        &repo,
        &[
            "-c",
            "tag.gpgSign=false",
            "tag",
            "-a",
            "not-published",
            "-m",
            "Do not publish",
            "feature",
        ],
    );
    let request = push_request(&oid);
    let review = app.preview_push(&id, &request).unwrap();
    assert!(review.blocked_reason.is_none());
    assert_eq!(review.outgoing_count, 1);
    assert_eq!(review.outgoing.len(), 1);
    assert_ne!(remote_oid(&bare, "main"), oid);
    let index = fs::read(repo.join(".git/index")).unwrap();
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let result = app.push_branch(&id, &request, &review.token).unwrap();
    assert_eq!(result.status, "succeeded", "{}", result.summary);
    assert_eq!(remote_oid(&bare, "main"), oid);
    assert!(git(
        &bare,
        &["for-each-ref", "--format=%(refname)", "refs/tags/"]
    )
    .is_empty());
    assert!(git(
        &bare,
        &["for-each-ref", "--format=%(refname)", "refs/heads/feature"]
    )
    .is_empty());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(git(&repo, &["rev-parse", "refs/remotes/origin/main"]), oid);
}
#[test]
fn first_push_and_optional_upstream_keep_uncommitted_work_out_of_publication() {
    let (temp, app, repo, id) = setup();
    let bare = remote(&repo, temp.path());
    let oid = git(&repo, &["rev-parse", "feature"]);
    let mut request = push_request(&oid);
    request.remote_branch = "published/feature".into();
    request.set_upstream = true;
    fs::write(repo.join("untracked.txt"), "not published").unwrap();
    fs::write(repo.join("file.txt"), "dirty not published\n").unwrap();
    git(&repo, &["add", "file.txt"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    let review = app.preview_push(&id, &request).unwrap();
    assert!(review.remote_oid.is_none());
    assert!(review.blocked_reason.is_none());
    let result = app.push_branch(&id, &request, &review.token).unwrap();
    assert_eq!(result.status, "succeeded", "{}", result.summary);
    assert_eq!(remote_oid(&bare, "refs/heads/published/feature"), oid);
    assert_eq!(
        git(&bare, &["show", "refs/heads/published/feature:file.txt"]),
        "feature"
    );
    assert_eq!(
        git(&repo, &["config", "branch.feature.merge"]),
        "refs/heads/published/feature"
    );
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(repo.join("untracked.txt")).unwrap(),
        "not published"
    );
}
#[test]
fn push_rejects_stale_destination_divergence_mirror_and_multiple_destinations() {
    let (temp, app, repo, id) = setup();
    let bare = remote(&repo, temp.path());
    let oid = git(&repo, &["rev-parse", "feature"]);
    let request = push_request(&oid);
    let review = app.preview_push(&id, &request).unwrap();
    git(&repo, &["push", "origin", "feature:main"]);
    assert!(app.push_branch(&id, &request, &review.token).is_err());
    git(&repo, &["switch", "-c", "side", "main"]);
    fs::write(repo.join("side.txt"), "side\n").unwrap();
    git(&repo, &["add", "side.txt"]);
    git(&repo, &["commit", "-m", "Divergent side"]);
    git(&repo, &["push", "origin", "+side:main"]);
    assert!(app
        .preview_push(&id, &request)
        .unwrap()
        .blocked_reason
        .is_some());
    git(&repo, &["config", "remote.origin.mirror", "true"]);
    assert!(app
        .preview_push(&id, &request)
        .unwrap()
        .blocked_reason
        .is_some());
    git(&repo, &["config", "--unset", "remote.origin.mirror"]);
    git(
        &repo,
        &[
            "remote",
            "set-url",
            "--add",
            "--push",
            "origin",
            bare.to_str().unwrap(),
        ],
    );
    git(
        &repo,
        &[
            "remote",
            "set-url",
            "--add",
            "--push",
            "origin",
            temp.path().join("another.git").to_str().unwrap(),
        ],
    );
    assert!(app.preview_push(&id, &request).is_err());
}
#[cfg(unix)]
#[test]
fn push_preview_does_not_run_pre_push_hook_and_a_destination_race_never_overwrites_new_work() {
    use std::os::unix::fs::PermissionsExt;
    let (temp, app, repo, id) = setup();
    let bare = remote(&repo, temp.path());
    let oid = git(&repo, &["rev-parse", "feature"]);
    git(&repo, &["switch", "-c", "racing", "main"]);
    fs::write(repo.join("racing.txt"), "remote work\n").unwrap();
    git(&repo, &["add", "racing.txt"]);
    git(&repo, &["commit", "-m", "Remote race"]);
    let foreign = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["push", "origin", "racing:racing"]);
    git(&repo, &["switch", "main"]);
    let hook = repo.join(".git/hooks/pre-push");
    let bare_quoted = bare.to_string_lossy().replace(char::from(39), r"'\''");
    fs::write(&hook,format!("#!/bin/sh\nprintf ran > .git/push-marker\ngit --git-dir='{bare_quoted}' update-ref refs/heads/main {foreign}\n")).unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let request = push_request(&oid);
    let review = app.preview_push(&id, &request).unwrap();
    assert!(!repo.join(".git/push-marker").exists());
    let result = app.push_branch(&id, &request, &review.token).unwrap();
    assert_ne!(result.status, "succeeded");
    assert!(result.recovery.is_some());
    assert!(repo.join(".git/push-marker").exists());
    assert_eq!(remote_oid(&bare, "main"), foreign);
}
#[test]
fn custom_push_transport_is_rejected_without_executing_a_helper() {
    let (temp, app, repo, id) = setup();
    let _bare = remote(&repo, temp.path());
    let marker = temp.path().join("custom-executed");
    git(
        &repo,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            &format!("ext::sh -c 'touch {}'", marker.display()),
        ],
    );
    git(&repo, &["config", "protocol.ext.allow", "always"]);
    let request = push_request(&git(&repo, &["rev-parse", "feature"]));
    assert!(app.preview_push(&id, &request).is_err());
    assert!(!marker.exists());
}

#[cfg(unix)]
#[test]
fn confirmed_push_reports_remote_publication_when_upstream_configuration_fails() {
    use std::os::unix::fs::PermissionsExt;
    let (temp, app, repo, id) = setup();
    let bare = remote(&repo, temp.path());
    let oid = git(&repo, &["rev-parse", "feature"]);
    let mut request = push_request(&oid);
    request.set_upstream = true;
    let hook = repo.join(".git/hooks/pre-push");
    fs::write(&hook, "#!/bin/sh\nprintf held > .git/config.lock\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let review = app.preview_push(&id, &request).unwrap();
    assert!(!repo.join(".git/config.lock").exists());
    let result = app.push_branch(&id, &request, &review.token).unwrap();
    assert_eq!(result.status, "unknown");
    assert!(result.recovery.unwrap().contains("remote may already"));
    assert_eq!(remote_oid(&bare, "main"), oid);
    assert!(repo.join(".git/config.lock").exists());
}
#[cfg(unix)]
#[test]
fn remote_inspection_timeout_kills_its_helpers_and_returns_within_a_bounded_time() {
    use std::os::unix::fs::PermissionsExt;
    let (temp, app, repo, id) = setup();
    let _bare = remote(&repo, temp.path());
    let script = temp.path().join("fake-ssh");
    fs::write(&script, "#!/bin/sh\nexec sleep 60\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let quoted = script.to_string_lossy().replace(char::from(39), r"'\''");
    git(
        &repo,
        &["config", "core.sshCommand", &format!("'{quoted}'")],
    );
    git(
        &repo,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "ssh://unused.invalid/example.git",
        ],
    );
    let request = push_request(&git(&repo, &["rev-parse", "feature"]));
    let started = std::time::Instant::now();
    let error = app.preview_push(&id, &request).unwrap_err();
    assert_eq!(error.code, "push_inspection");
    assert!(started.elapsed() < std::time::Duration::from_secs(26));
}
#[test]
fn a_symbolic_recovery_destination_cannot_redirect_deletion() {
    let (_temp, app, repo, id) = setup();
    let oid = git(&repo, &["rev-parse", "feature"]);
    let action = ref_action(RefOperationKind::DeleteBranch, "feature", &oid);
    let preview = app.preview_ref_operation(&id, &action).unwrap();
    let backup = preview.recovery_ref.unwrap();
    git(&repo, &["symbolic-ref", &backup, "refs/heads/feature"]);
    assert!(app.preview_ref_operation(&id, &action).is_err());
    assert_eq!(git(&repo, &["rev-parse", "feature"]), oid);
}

#[test]
fn push_to_the_same_repository_is_blocked_even_if_native_receive_allows_worktree_updates() {
    let (_temp, app, repo, id) = setup();
    git(
        &repo,
        &["config", "receive.denyCurrentBranch", "updateInstead"],
    );
    git(&repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
    let request = push_request(&git(&repo, &["rev-parse", "feature"]));
    let review = app.preview_push(&id, &request).unwrap();
    assert!(review.blocked_reason.unwrap().contains("same repository"));
    git(
        &repo,
        &[
            "remote",
            "set-url",
            "origin",
            &format!("file://{}", repo.display()),
        ],
    );
    let review = app.preview_push(&id, &request).unwrap();
    assert!(review.blocked_reason.unwrap().contains("same repository"));
    assert_eq!(git(&repo, &["branch", "--show-current"]), "main");
}

#[test]
fn upstream_selection_keeps_the_named_remote_when_another_remote_maps_the_same_tracking_ref() {
    let (temp, app, repo, id) = setup();
    let _bare = remote(&repo, temp.path());
    let other = temp.path().join("other.git");
    git(temp.path(), &["init", "--bare", other.to_str().unwrap()]);
    git(&repo, &["remote", "add", "other", other.to_str().unwrap()]);
    git(
        &repo,
        &[
            "config",
            "remote.other.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
    );
    let oid = git(&repo, &["rev-parse", "feature"]);
    let mut request = ref_action(RefOperationKind::SetUpstream, "feature", &oid);
    request.remote = "origin".into();
    request.remote_branch = "main".into();
    let preview = app.preview_ref_operation(&id, &request).unwrap();
    assert!(preview.blocked_reason.is_none());
    let receipt = app
        .apply_ref_operation(&id, &request, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
    assert_eq!(git(&repo, &["config", "branch.feature.remote"]), "origin");
    assert_eq!(
        git(&repo, &["config", "branch.feature.merge"]),
        "refs/heads/main"
    );
}
#[test]
fn inherited_tracking_is_not_silently_replaced_by_ref_or_push_configuration() {
    let (temp, app, repo, id) = setup();
    let _bare = remote(&repo, temp.path());
    fs::write(
        repo.join(".git/tracking.conf"),
        "[branch \"feature\"]\n remote = origin\n merge = refs/heads/main\n",
    )
    .unwrap();
    git(&repo, &["config", "include.path", "tracking.conf"]);
    let oid = git(&repo, &["rev-parse", "feature"]);
    let mut request = ref_action(RefOperationKind::SetUpstream, "feature", &oid);
    request.remote = "origin".into();
    request.remote_branch = "main".into();
    assert!(app
        .preview_ref_operation(&id, &request)
        .unwrap()
        .blocked_reason
        .is_some());
    let mut push = push_request(&oid);
    push.set_upstream = true;
    assert!(app
        .preview_push(&id, &push)
        .unwrap()
        .blocked_reason
        .is_some());
    assert!(fs::read_to_string(repo.join(".git/tracking.conf"))
        .unwrap()
        .contains("merge = refs/heads/main"));
}

#[test]
fn forced_duplicate_branch_checkouts_block_shared_metadata_changes() {
    let (temp, app, repo, id) = setup();
    let _bare = remote(&repo, temp.path());
    let oid = git(&repo, &["rev-parse", "main"]);
    let tree = temp.path().join("duplicate-main");
    git(
        &repo,
        &["worktree", "add", "--force", tree.to_str().unwrap(), "main"],
    );
    let mut rename = ref_action(RefOperationKind::RenameBranch, "main", &oid);
    rename.new_name = "new-main".into();
    assert!(app
        .preview_ref_operation(&id, &rename)
        .unwrap()
        .blocked_reason
        .unwrap()
        .contains("multiple worktrees"));
    let mut request = push_request(&oid);
    request.branch = "main".into();
    request.set_upstream = true;
    assert!(app
        .preview_push(&id, &request)
        .unwrap()
        .blocked_reason
        .is_some());
    assert_eq!(git(&tree, &["branch", "--show-current"]), "main");
}

#[cfg(unix)]
#[test]
fn partial_deletion_reports_the_recovery_location_when_config_cleanup_fails() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, app, repo, id) = setup();
    git(
        &repo,
        &["config", "branch.feature.description", "Keep metadata"],
    );
    let oid = git(&repo, &["rev-parse", "feature"]);
    let request = ref_action(RefOperationKind::DeleteBranch, "feature", &oid);
    let hook = repo.join(".git/hooks/reference-transaction");
    fs::write(
        &hook,
        "#!/bin/sh\nif test \"$1\" = committed; then printf held > .git/config.lock; fi\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let preview = app.preview_ref_operation(&id, &request).unwrap();
    let backup = preview.recovery_ref.unwrap();
    assert!(!repo.join(".git/config.lock").exists());
    let receipt = app
        .apply_ref_operation(&id, &request, &preview.token)
        .unwrap();
    assert_eq!(receipt.status, "unknown");
    assert!(receipt.recovery.unwrap().contains(&backup));
    assert_eq!(git(&repo, &["rev-parse", &backup]), oid);
    assert!(git(&repo, &["branch", "--list", "feature"]).is_empty());
    assert!(repo.join(".git/config.lock").exists());
}

#[test]
fn embedded_push_credentials_are_rejected_before_connecting_or_returning_secrets() {
    let (temp, app, repo, id) = setup();
    let _bare = remote(&repo, temp.path());
    let request = push_request(&git(&repo, &["rev-parse", "feature"]));
    for url in [
        "https://user:private-token@127.0.0.1:1/repo.git",
        "HTTP://private-token@127.0.0.1:1/repo.git",
        "https://127.0.0.1:1/repo.git?token=private-token",
    ] {
        git(&repo, &["remote", "set-url", "--push", "origin", url]);
        let error = app.preview_push(&id, &request).unwrap_err();
        assert_eq!(error.code, "push");
        assert!(error.message.contains("credential helper"));
        assert!(!error.message.contains("private-token"));
    }
}

#[test]
fn pushing_a_parent_never_implicitly_publishes_an_unpushed_submodule_commit() {
    let (temp, app, repo, id) = setup();
    let child_source = temp.path().join("child-source");
    let child_remote = temp.path().join("child.git");
    fs::create_dir(&child_source).unwrap();
    git(&child_source, &["init", "-b", "main"]);
    git(&child_source, &["config", "user.name", "GitNova Test"]);
    git(
        &child_source,
        &["config", "user.email", "test@example.invalid"],
    );
    git(&child_source, &["config", "commit.gpgSign", "false"]);
    fs::write(child_source.join("child.txt"), "first\n").unwrap();
    git(&child_source, &["add", "."]);
    git(&child_source, &["commit", "-m", "First child"]);
    let first = git(&child_source, &["rev-parse", "HEAD"]);
    git(
        temp.path(),
        &["init", "--bare", child_remote.to_str().unwrap()],
    );
    git(
        &child_source,
        &["remote", "add", "origin", child_remote.to_str().unwrap()],
    );
    git(&child_source, &["push", "origin", "main"]);
    git(&child_remote, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    git(
        &repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child_remote.to_str().unwrap(),
            "libs/child",
        ],
    );
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Track first child"]);
    let parent_before = git(&repo, &["rev-parse", "HEAD"]);
    let parent_remote = remote(&repo, temp.path());
    let child = repo.join("libs/child");
    fs::write(child.join("child.txt"), "second not pushed\n").unwrap();
    git(
        &child,
        &[
            "-c",
            "user.name=GitNova Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgSign=false",
            "commit",
            "-am",
            "Unpushed child",
        ],
    );
    let second = git(&child, &["rev-parse", "HEAD"]);
    git(&repo, &["add", "libs/child"]);
    git(&repo, &["commit", "-m", "Record unpublished child"]);
    git(&repo, &["config", "push.recurseSubmodules", "on-demand"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let request = PushRequest {
        branch: "main".into(),
        oid: head.clone(),
        remote: "origin".into(),
        remote_branch: "main".into(),
        set_upstream: false,
        buffered_https: false,
    };
    let preview = app.preview_push(&id, &request).unwrap();
    assert!(preview.blocked_reason.is_none());
    let receipt = app.push_branch(&id, &request, &preview.token).unwrap();
    assert_ne!(receipt.status, "succeeded");
    assert_eq!(remote_oid(&parent_remote, "main"), parent_before);
    assert_eq!(remote_oid(&child_remote, "main"), first);
    assert_eq!(git(&child, &["rev-parse", "HEAD"]), second);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
}

#[test]
fn large_new_and_existing_branch_pushes_keep_full_scope_with_bounded_subject_review() {
    use std::io::Write;
    use std::process::Stdio;
    let (directory, app, repo, id) = setup();
    let bare = remote(&repo, directory.path());
    let base = git(&repo, &["rev-parse", "feature"]);
    let mut input = String::new();
    for n in 1..=520 {
        let subject = format!("Long history {n}");
        let parent = if n == 1 {
            base.clone()
        } else {
            format!(":{}", n - 1)
        };
        input.push_str(&format!("commit refs/heads/feature\nmark :{n}\ncommitter GitNova Test <test@example.invalid> {} +0000\ndata {}\n{subject}\nfrom {parent}\n\n", 1700000000+n,subject.len()));
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(&repo)
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
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tip = git(&repo, &["rev-parse", "feature"]);
    let local_head = git(&repo, &["rev-parse", "HEAD"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    let config = fs::read(repo.join(".git/config")).unwrap();
    for destination in ["new-feature", "main"] {
        let mut request = push_request(&tip);
        request.remote_branch = destination.into();
        let review = app.preview_push(&id, &request).unwrap();
        assert!(
            review.blocked_reason.is_none(),
            "{:?}",
            review.blocked_reason
        );
        assert_eq!(
            review.outgoing_count,
            if destination == "new-feature" {
                522
            } else {
                521
            }
        );
        assert_eq!(review.outgoing.len(), 500);
        assert_eq!(review.outgoing[0].oid, tip);
        if destination == "new-feature" {
            assert!(review.remote_oid.is_none());
            assert!(git(
                &bare,
                &[
                    "for-each-ref",
                    "--format=%(refname)",
                    "refs/heads/new-feature"
                ]
            )
            .is_empty());
            // A destination created after preview must invalidate that preview,
            // even if it is an ancestor and would allow a normal fast-forward.
            git(
                &bare,
                &["update-ref", "refs/heads/new-feature", &local_head],
            );
            assert!(app.push_branch(&id, &request, &review.token).is_err());
            assert_eq!(remote_oid(&bare, "new-feature"), local_head);
            git(&bare, &["update-ref", "-d", "refs/heads/new-feature"]);
        }
        let review = app.preview_push(&id, &request).unwrap();
        let receipt = app.push_branch(&id, &request, &review.token).unwrap();
        assert_eq!(receipt.status, "succeeded", "{}", receipt.summary);
        assert_eq!(remote_oid(&bare, destination), tip);
        assert_eq!(git(&bare, &["rev-list", "--count", destination]), "522");
    }
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), local_head);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(repo.join(".git/config")).unwrap(), config);
    assert_eq!(fs::read_to_string(repo.join("file.txt")).unwrap(), "main\n");
}
