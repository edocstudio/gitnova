use gitnova_domain::NativePath;
use gitnova_git::GitCli;
use std::{fs, path::Path, process::Command};
use tempfile::{tempdir, TempDir};
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Editor Test",
            "-c",
            "user.email=editor@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=.disabled-hooks",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
fn fixture() -> TempDir {
    let folder = tempdir().unwrap();
    git(folder.path(), &["init", "-b", "main"]);
    git(folder.path(), &["config", "core.autocrlf", "false"]);
    fs::write(folder.path().join("file.txt"), "original\n").unwrap();
    git(folder.path(), &["add", "."]);
    git(folder.path(), &["commit", "-m", "Initial"]);
    folder
}
fn path() -> NativePath {
    NativePath::new(Path::new("file.txt"))
}
#[test]
fn saves_only_working_bytes_and_keeps_raw_recovery_and_staging() {
    let folder = fixture();
    let root = folder.path();
    fs::write(root.join("file.txt"), "staged\n").unwrap();
    git(root, &["add", "file.txt"]);
    fs::write(root.join("file.txt"), "current\r\n").unwrap();
    let head = git(root, &["rev-parse", "HEAD"]);
    let index = git(root, &["write-tree"]);
    let preview = GitCli.working_file_text(root, &path()).unwrap();
    assert_eq!(preview.text, "current\n");
    GitCli
        .save_working_file_text(root, &path(), "edited\n", &preview.token)
        .unwrap();
    assert_eq!(fs::read(root.join("file.txt")).unwrap(), b"edited\r\n");
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(root, &["write-tree"]), index);
    let files: Vec<_> = fs::read_dir(root.join(".git/gitnova-file-edits"))
        .unwrap()
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(
        fs::read(files[0].as_ref().unwrap().path()).unwrap(),
        b"current\r\n"
    );
    assert!(!root.join(".git/gitnova-operation.lock").exists());
    assert!(!root.join(".git/index.lock").exists());
}
#[test]
fn rejects_external_changes_and_branch_or_index_changes() {
    let folder = fixture();
    let root = folder.path();
    let preview = GitCli.working_file_text(root, &path()).unwrap();
    fs::write(root.join("file.txt"), "external\n").unwrap();
    assert_eq!(
        GitCli
            .save_working_file_text(root, &path(), "draft\n", &preview.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    assert_eq!(
        fs::read_to_string(root.join("file.txt")).unwrap(),
        "external\n"
    );
    let preview = GitCli.working_file_text(root, &path()).unwrap();
    git(root, &["checkout", "-b", "other"]);
    assert!(GitCli
        .save_working_file_text(root, &path(), "draft\n", &preview.token)
        .is_err());
    let preview = GitCli.working_file_text(root, &path()).unwrap();
    git(root, &["add", "file.txt"]);
    assert!(GitCli
        .save_working_file_text(root, &path(), "draft\n", &preview.token)
        .is_err());
}
#[test]
fn rejects_missing_binary_oversize_unsafe_paths_and_pending_operations() {
    let folder = fixture();
    let root = folder.path();
    for value in ["../file.txt", ".git/config", "missing.txt"] {
        assert!(GitCli
            .working_file_text(root, &NativePath::new(Path::new(value)))
            .is_err());
    }
    for bytes in [
        vec![0, 1],
        vec![b'x'; 4 * 1024 * 1024 + 1],
        vec![0xff],
        b"mixed\r\nline\n".to_vec(),
    ] {
        fs::write(root.join("file.txt"), bytes).unwrap();
        assert!(GitCli.working_file_text(root, &path()).is_err());
    }
    fs::write(root.join("file.txt"), "ok\n").unwrap();
    fs::write(root.join(".git/MERGE_HEAD"), "pending").unwrap();
    assert!(GitCli.working_file_text(root, &path()).is_err());
}

#[test]
fn edits_large_utf8_files_preserving_crlf_staging_history_and_stale_protection() {
    let folder = fixture();
    let root = folder.path();
    let original = "description: ภาษาไทย 😀\r\n".repeat(20000);
    assert!(original.len() > 512 * 1024);
    fs::write(root.join("file.txt"), &original).unwrap();
    let head = git(root, &["rev-parse", "HEAD"]);
    let index = git(root, &["write-tree"]);
    let preview = GitCli.working_file_text(root, &path()).unwrap();
    assert_eq!(preview.max_bytes, 4 * 1024 * 1024);
    let edited = format!("{}edited: true\n", preview.text);
    GitCli
        .save_working_file_text(root, &path(), &edited, &preview.token)
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("file.txt")).unwrap(),
        edited.replace('\n', "\r\n")
    );
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(root, &["write-tree"]), index);
    let backup = fs::read_dir(root.join(".git/gitnova-file-edits"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(fs::read_to_string(backup.path()).unwrap(), original);
    assert_eq!(
        GitCli
            .save_working_file_text(root, &path(), "old draft\n", &preview.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
}

#[test]
fn enforces_raw_and_saved_byte_limits_without_modifying_the_file() {
    let folder = fixture();
    let root = folder.path();
    let original = "x".repeat(4 * 1024 * 1024);
    fs::write(root.join("file.txt"), &original).unwrap();
    let preview = GitCli.working_file_text(root, &path()).unwrap();
    assert!(GitCli
        .save_working_file_text(root, &path(), &format!("{original}x"), &preview.token)
        .is_err());
    assert_eq!(fs::read_to_string(root.join("file.txt")).unwrap(), original);
    fs::write(root.join("file.txt"), "old\r\n").unwrap();
    let preview = GitCli.working_file_text(root, &path()).unwrap();
    // Normalized text fits; restoring CRLF would exceed the raw-file budget.
    let expanded = "x\n".repeat(4 * 1024 * 1024 / 2);
    assert!(GitCli
        .save_working_file_text(root, &path(), &expanded, &preview.token)
        .is_err());
    assert_eq!(fs::read(root.join("file.txt")).unwrap(), b"old\r\n");
}
#[cfg(unix)]
#[test]
fn rejects_symlinks_and_preserves_executable_mode() {
    use std::os::unix::{fs::symlink, fs::PermissionsExt};
    let folder = fixture();
    let root = folder.path();
    fs::remove_file(root.join("file.txt")).unwrap();
    symlink(".git/config", root.join("file.txt")).unwrap();
    assert!(GitCli.working_file_text(root, &path()).is_err());
    fs::remove_file(root.join("file.txt")).unwrap();
    fs::write(root.join("file.txt"), "script\n").unwrap();
    fs::set_permissions(root.join("file.txt"), fs::Permissions::from_mode(0o755)).unwrap();
    let preview = GitCli.working_file_text(root, &path()).unwrap();
    GitCli
        .save_working_file_text(root, &path(), "edited\n", &preview.token)
        .unwrap();
    assert_eq!(
        fs::metadata(root.join("file.txt"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
}

#[test]
fn refuses_nested_repository_and_existing_index_lock() {
    let folder = fixture();
    let root = folder.path();
    fs::create_dir(root.join("child")).unwrap();
    fs::write(root.join("child/nested.txt"), "nested\n").unwrap();
    git(root, &["add", "child/nested.txt"]);
    git(root, &["commit", "-m", "Nested file"]);
    git(&root.join("child"), &["init", "-b", "main"]);
    assert!(GitCli
        .working_file_text(root, &NativePath::new(Path::new("child/nested.txt")))
        .is_err());
    let preview = GitCli.working_file_text(root, &path()).unwrap();
    fs::write(root.join(".git/index.lock"), "another writer").unwrap();
    assert!(GitCli
        .save_working_file_text(root, &path(), "draft\n", &preview.token)
        .is_err());
    assert_eq!(
        fs::read_to_string(root.join("file.txt")).unwrap(),
        "original\n"
    );
    assert_eq!(
        fs::read_to_string(root.join(".git/index.lock")).unwrap(),
        "another writer"
    );
    assert!(!root.join(".git/gitnova-operation.lock").exists());
}
