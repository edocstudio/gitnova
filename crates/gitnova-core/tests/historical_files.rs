use gitnova_core::Application;
use gitnova_domain::NativePath;
use std::{fs, path::Path, process::Command};
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
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
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
#[test]
fn large_patch_has_complete_file_counts_and_exact_late_file_reads() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    fs::write(
        root.join("a-large.txt"),
        "large line content\n".repeat(40_000),
    )
    .unwrap();
    for i in 0..126 {
        fs::write(
            root.join(format!("z-{i:03}.txt")),
            format!("late-file-{i}\n"),
        )
        .unwrap();
    }
    fs::write(root.join("z-binary.dat"), [0, 255, 1, 2]).unwrap();
    fs::write(root.join("z spaced \"quoted\".txt"), "literal path\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "Large root"]);
    let oid = git(&root, &["rev-parse", "HEAD"]);
    let index = fs::read(root.join(".git/index")).unwrap();
    let app = Application::new(temp.path().join("state.json")).unwrap();
    let c = app.add_repository(&root).unwrap();
    let id = c.workspaces[0].selected_id.as_ref().unwrap();
    let detail = app.detail(id, &oid).unwrap();
    assert!(detail.truncated);
    assert_eq!(detail.files.len(), 129);
    assert_eq!(
        detail
            .files
            .iter()
            .map(|f| f.additions.unwrap_or(0))
            .sum::<u64>(),
        40127
    );
    assert!(detail
        .files
        .iter()
        .any(|f| f.path.display == "z-binary.dat" && f.additions.is_none()));
    let late = detail
        .files
        .iter()
        .find(|f| f.path.display == "z-125.txt")
        .unwrap();
    let patch = app.historical_patch(id, &oid, &late.path).unwrap();
    assert!(!patch.truncated);
    assert!(patch.diff.contains("+late-file-125"));
    assert!(!patch.diff.contains("large line content"));
    let quoted = detail
        .files
        .iter()
        .find(|f| f.path.display.contains("quoted"))
        .unwrap();
    assert!(app
        .historical_patch(id, &oid, &quoted.path)
        .unwrap()
        .diff
        .contains("+literal path"));
    assert!(app
        .historical_patch(id, &oid, &NativePath::new(Path::new("../outside")))
        .is_err());
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), oid);
}
#[test]
fn per_file_merge_diff_uses_the_same_first_parent_and_does_not_read_working_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    fs::write(root.join("base"), "base\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);
    git(&root, &["checkout", "-b", "feature"]);
    fs::write(root.join("new.txt"), "committed feature\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "feature"]);
    git(&root, &["checkout", "main"]);
    git(&root, &["merge", "--no-ff", "feature", "-m", "merge"]);
    let oid = git(&root, &["rev-parse", "HEAD"]);
    fs::write(root.join("new.txt"), "private unstaged\n").unwrap();
    let app = Application::new(temp.path().join("state.json")).unwrap();
    let c = app.add_repository(&root).unwrap();
    let id = c.workspaces[0].selected_id.as_ref().unwrap();
    let detail = app.detail(id, &oid).unwrap();
    assert_eq!(detail.files.len(), 1);
    let patch = app
        .historical_patch(id, &oid, &detail.files[0].path)
        .unwrap();
    assert_eq!(patch.diff, detail.diff);
    assert!(!patch.diff.contains("private unstaged"));
}
