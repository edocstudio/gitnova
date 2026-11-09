use std::{fs, path::Path, process::Command};
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
    String::from_utf8(output.stdout).unwrap().trim_end().into()
}
#[test]
fn cli_preview_cancel_and_confirmed_publication_use_exact_selected_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("local");
    let remote = temp.path().join("remote.git");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(&remote).unwrap();
    git(&remote, &["init", "--bare", "--initial-branch=main"]);
    git(&repo, &["init", "--initial-branch=main"]);
    git(&repo, &["config", "user.name", "CLI Fixture"]);
    git(&repo, &["config", "user.email", "cli@example.test"]);
    fs::write(repo.join("chosen.txt"), "target\n").unwrap();
    fs::write(repo.join("other.txt"), "target other\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Base"]);
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&repo, &["push", "origin", "main"]);
    git(&repo, &["switch", "-c", "develop"]);
    fs::write(repo.join("chosen.txt"), "source\n").unwrap();
    fs::write(repo.join("other.txt"), "excluded\n").unwrap();
    git(&repo, &["commit", "-am", "Source"]);
    git(&repo, &["push", "origin", "develop"]);
    fs::write(repo.join("other.txt"), "keep my uncommitted work\n").unwrap();
    let args = [
        "promote",
        "--source",
        "develop",
        "--target",
        "main",
        "--branch",
        "promote/chosen",
        "--no-request",
        "--",
        "chosen.txt",
    ]
    .map(str::to_owned);
    let mut out = Vec::new();
    gitnova_core::nova_cli::run(&args, &repo, &mut &b""[..], &mut out).unwrap();
    assert!(String::from_utf8(out).unwrap().contains("Preview only."));
    assert_eq!(
        git(&remote, &["for-each-ref", "refs/heads/promote/chosen"]),
        ""
    );
    let mut execute = args.to_vec();
    execute.insert(1, "--execute".into());
    assert!(gitnova_core::nova_cli::run(&execute, &repo, &mut &b""[..], &mut Vec::new()).is_err());
    assert_eq!(
        git(&remote, &["for-each-ref", "refs/heads/promote/chosen"]),
        ""
    );
    let mut out = Vec::new();
    gitnova_core::nova_cli::run(
        &execute,
        &repo,
        &mut &b"publish promote/chosen\n"[..],
        &mut out,
    )
    .unwrap();
    let output = String::from_utf8(out).unwrap();
    assert!(output.contains("Published promote/chosen"));
    assert!(output.contains("CLI Fixture <cli@example.test>"));
    assert_eq!(
        git(&remote, &["show", "promote/chosen:chosen.txt"]),
        "source"
    );
    assert_eq!(
        git(&remote, &["show", "promote/chosen:other.txt"]),
        "target other"
    );
    assert_eq!(
        fs::read_to_string(repo.join("other.txt")).unwrap(),
        "keep my uncommitted work\n"
    );
    assert_eq!(git(&repo, &["symbolic-ref", "HEAD"]), "refs/heads/develop");
    let recovery = output
        .lines()
        .find_map(|line| line.strip_prefix("Retained recovery repository: "))
        .unwrap();
    fs::remove_dir_all(recovery).unwrap();
}
