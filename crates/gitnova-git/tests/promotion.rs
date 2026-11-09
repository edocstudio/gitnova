use gitnova_git::{
    promotion::{Promotion, PromotionRequest},
    GitCli,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end()
        .to_owned()
}
struct Fixture {
    _temp: TempDir,
    repo: PathBuf,
    remote: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("checkout with spaces");
        let remote = temp.path().join("remote.git");
        fs::create_dir(&repo).unwrap();
        fs::create_dir(&remote).unwrap();
        git(&remote, &["init", "--bare", "--initial-branch=main"]);
        git(&repo, &["init", "--initial-branch=main"]);
        git(&repo, &["config", "user.name", "Fixture Author"]);
        git(&repo, &["config", "user.email", "author@example.test"]);
        fs::write(repo.join("selected.txt"), "base\n").unwrap();
        fs::write(repo.join("untouched.txt"), "target version\n").unwrap();
        fs::write(repo.join("delete.txt"), "remove me\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "Base"]);
        git(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        git(&repo, &["push", "origin", "main"]);
        git(&repo, &["switch", "-c", "develop"]);
        fs::write(repo.join("selected.txt"), "source snapshot\n").unwrap();
        fs::write(repo.join("untouched.txt"), "exclude source edit\n").unwrap();
        fs::remove_file(repo.join("delete.txt")).unwrap();
        fs::write(repo.join("new file.bin"), [0, 1, 255, 0]).unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-m", "Source changes"]);
        git(&repo, &["push", "origin", "develop"]);
        git(&repo, &["switch", "main"]);
        fs::write(repo.join("dirty.txt"), "staged local work\n").unwrap();
        git(&repo, &["add", "dirty.txt"]);
        fs::write(repo.join("dirty.txt"), "unstaged local work\n").unwrap();
        Self {
            _temp: temp,
            repo,
            remote,
        }
    }
    fn request(&self, files: &[&str]) -> PromotionRequest {
        PromotionRequest {
            remote: "origin".into(),
            source: "develop".into(),
            target: "main".into(),
            branch: "promote/selected".into(),
            files: files.iter().map(|p| (*p).into()).collect(),
            allow_deletions: false,
        }
    }
    fn prepare(&self, files: &[&str]) -> Promotion {
        GitCli
            .prepare_promotion(&self.repo, self.request(files))
            .unwrap()
    }
}
#[test]
fn snapshots_preserve_unselected_files_dirty_checkout_and_staging() {
    let f = Fixture::new();
    let head = git(&f.repo, &["rev-parse", "HEAD"]);
    let index = fs::read(f.repo.join(".git/index")).unwrap();
    let status = git(&f.repo, &["status", "--porcelain"]);
    let preview = f.prepare(&["selected.txt", "new file.bin"]);
    assert_eq!(preview.files.len(), 2);
    assert_eq!(preview.identity.email, "author@example.test");
    assert_eq!(
        git(&f.remote, &["for-each-ref", "refs/heads/promote/selected"]),
        ""
    );
    let mut recovery = None;
    let oid = preview
        .publish("Promote selected snapshots", |p| {
            recovery = Some(p.to_owned());
            Ok(())
        })
        .unwrap();
    assert_eq!(git(&f.remote, &["rev-parse", "promote/selected"]), oid);
    assert_eq!(
        git(&f.remote, &["show", "promote/selected:selected.txt"]),
        "source snapshot"
    );
    assert_eq!(
        git(&f.remote, &["show", "promote/selected:untouched.txt"]),
        "target version"
    );
    assert_eq!(git(&f.remote, &["rev-parse", "promote/selected^"]), head);
    assert_eq!(git(&f.remote, &["rev-parse", "main"]), head);
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&f.repo, &["symbolic-ref", "HEAD"]), "refs/heads/main");
    assert_eq!(fs::read(f.repo.join(".git/index")).unwrap(), index);
    assert_eq!(git(&f.repo, &["status", "--porcelain"]), status);
    assert_eq!(
        fs::read_to_string(f.repo.join("dirty.txt")).unwrap(),
        "unstaged local work\n"
    );
    fs::remove_dir_all(recovery.unwrap()).unwrap();
}
#[test]
fn deletions_are_explicit_and_noop_selection_is_rejected() {
    let f = Fixture::new();
    assert!(GitCli
        .prepare_promotion(&f.repo, f.request(&["delete.txt"]))
        .is_err());
    assert!(GitCli
        .prepare_promotion(&f.repo, f.request(&["absent.txt"]))
        .is_err());
    let mut request = f.request(&["delete.txt"]);
    request.allow_deletions = true;
    let preview = GitCli.prepare_promotion(&f.repo, request).unwrap();
    assert_eq!(preview.files[0].action, "delete");
    let mut recovery = None;
    preview
        .publish("Remove selected file", |p| {
            recovery = Some(p.to_owned());
            Ok(())
        })
        .unwrap();
    assert_eq!(
        git(
            &f.remote,
            &["ls-tree", "promote/selected", "--", "delete.txt"]
        ),
        ""
    );
    fs::remove_dir_all(recovery.unwrap()).unwrap();
    assert!(GitCli
        .prepare_promotion(&f.repo, f.request(&["selected.txt", "selected.txt"]))
        .is_err());
}
#[test]
fn destination_movement_and_existing_review_branch_cannot_overwrite_history() {
    let f = Fixture::new();
    let preview = f.prepare(&["selected.txt"]);
    let source = git(&f.remote, &["rev-parse", "develop"]);
    git(&f.remote, &["update-ref", "refs/heads/main", &source]);
    assert!(preview
        .publish("Stale target", |_| panic!(
            "must fail before retaining candidate"
        ))
        .is_err());
    git(
        &f.remote,
        &["update-ref", "refs/heads/promote/selected", &source],
    );
    assert!(GitCli
        .prepare_promotion(&f.repo, f.request(&["selected.txt"]))
        .is_err());
    assert_eq!(git(&f.remote, &["rev-parse", "promote/selected"]), source);
}
#[test]
fn unsafe_paths_and_submodule_layout_are_rejected_before_remote_reads() {
    let f = Fixture::new();
    for path in [
        "../outside",
        ".git/config",
        ".gitmodules",
        "selected.txt\nother",
        "folder\\file",
    ] {
        assert!(
            GitCli
                .prepare_promotion(&f.repo, f.request(&[path]))
                .is_err(),
            "{path}"
        );
    }
    for branch in ["--delete", "bad..branch", "main"] {
        let mut request = f.request(&["selected.txt"]);
        request.branch = branch.into();
        assert!(GitCli.prepare_promotion(&f.repo, request).is_err());
    }
}
#[test]
fn publication_race_retains_recovery_and_never_overwrites_remote_branch() {
    let f = Fixture::new();
    let preview = f.prepare(&["selected.txt"]);
    let target = git(&f.remote, &["rev-parse", "main"]);
    git(
        &f.remote,
        &["update-ref", "refs/heads/promote/selected", &target],
    );
    let mut recovery = None;
    assert!(preview
        .publish("Race", |p| {
            recovery = Some(p.to_owned());
            Ok(())
        })
        .is_err());
    assert_eq!(git(&f.remote, &["rev-parse", "promote/selected"]), target);
    let recovery = recovery.unwrap();
    assert_ne!(git(&recovery, &["rev-parse", "promote/selected"]), target);
    assert_eq!(
        git(&recovery, &["show", "promote/selected:selected.txt"]),
        "source snapshot"
    );
    fs::remove_dir_all(recovery).unwrap();
}
#[test]
fn source_movement_after_review_is_rejected() {
    let f = Fixture::new();
    let preview = f.prepare(&["selected.txt"]);
    let target = git(&f.remote, &["rev-parse", "main"]);
    git(&f.remote, &["update-ref", "refs/heads/develop", &target]);
    assert!(preview
        .publish("Stale source", |_| panic!("must re-review source"))
        .is_err());
    assert_eq!(
        git(&f.remote, &["for-each-ref", "refs/heads/promote/selected"]),
        ""
    );
}
