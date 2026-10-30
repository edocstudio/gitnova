use gitnova_core::Application;
use gitnova_domain::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
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
struct F {
    _dir: tempfile::TempDir,
    root: PathBuf,
    app: Application,
    id: String,
}
fn fixture(name: &str) -> F {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Fixture"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    git(&root, &["config", "core.autocrlf", "false"]);
    fs::write(root.join(name), text("base", "base")).unwrap();
    fs::write(root.join("other"), "before\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);
    let app = Application::new(dir.path().join("catalog.json")).unwrap();
    let id = app.add_repository(&root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    F {
        _dir: dir,
        root,
        app,
        id,
    }
}
fn text(a: &str, b: &str) -> String {
    (0..40)
        .map(|i| {
            format!(
                "line-{i}: {}\n",
                if i == 2 {
                    a
                } else if i == 35 {
                    b
                } else {
                    "unchanged"
                }
            )
        })
        .collect()
}
fn path(name: &str) -> NativePath {
    NativePath::new(Path::new(name))
}
#[test]
fn stage_and_unstage_exact_hunks_preserve_other_staged_and_working_content() {
    let f = fixture("file");
    fs::write(f.root.join("file"), text("first", "second")).unwrap();
    fs::write(f.root.join("other"), "already staged\n").unwrap();
    git(&f.root, &["add", "other"]);
    let p = f.app.preview_hunks(&f.id, &path("file"), false).unwrap();
    assert_eq!(p.headers.len(), 2);
    assert_eq!(
        f.app
            .change_hunk(&f.id, &path("file"), false, 1, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        git(&f.root, &["show", ":file"]),
        text("base", "second").trim_end()
    );
    assert_eq!(git(&f.root, &["show", ":other"]), "already staged");
    assert_eq!(
        fs::read_to_string(f.root.join("file")).unwrap(),
        text("first", "second")
    );
    let p = f.app.preview_hunks(&f.id, &path("file"), true).unwrap();
    assert_eq!(p.headers.len(), 1);
    assert_eq!(
        f.app
            .change_hunk(&f.id, &path("file"), true, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        git(&f.root, &["show", ":file"]),
        text("base", "base").trim_end()
    );
    assert_eq!(
        fs::read_to_string(f.root.join("file")).unwrap(),
        text("first", "second")
    );
}
#[test]
fn stale_file_index_config_and_index_lock_preserve_the_index() {
    let f = fixture("file");
    fs::write(f.root.join("file"), text("one", "two")).unwrap();
    let p = f.app.preview_hunks(&f.id, &path("file"), false).unwrap();
    let before = fs::read(f.root.join(".git/index")).unwrap();
    fs::write(f.root.join("file"), text("new", "two")).unwrap();
    assert_ne!(
        f.app
            .change_hunk(&f.id, &path("file"), false, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), before);
    let p = f.app.preview_hunks(&f.id, &path("file"), false).unwrap();
    git(&f.root, &["config", "diff.algorithm", "histogram"]);
    assert_ne!(
        f.app
            .change_hunk(&f.id, &path("file"), false, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let p = f.app.preview_hunks(&f.id, &path("file"), false).unwrap();
    fs::write(f.root.join(".git/index.lock"), "foreign").unwrap();
    assert_ne!(
        f.app
            .change_hunk(&f.id, &path("file"), false, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        fs::read_to_string(f.root.join(".git/index.lock")).unwrap(),
        "foreign"
    );
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), before);
}
#[test]
fn binary_and_mode_changes_are_refused_without_guessing() {
    let f = fixture("file");
    fs::write(f.root.join("file"), b"binary\0changed").unwrap();
    assert!(f.app.preview_hunks(&f.id, &path("file"), false).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        git(&f.root, &["config", "core.filemode", "true"]);
        fs::write(f.root.join("file"), text("one", "two")).unwrap();
        fs::set_permissions(f.root.join("file"), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(f.app.preview_hunks(&f.id, &path("file"), false).is_err());
    }
}
#[test]
fn no_newline_and_quoted_paths_use_exact_patch_bytes() {
    let name = "space and\ttab.txt";
    let f = fixture(name);
    let value = text("one", "two").trim_end().to_string();
    fs::write(f.root.join(name), &value).unwrap();
    let p = f.app.preview_hunks(&f.id, &path(name), false).unwrap();
    assert_eq!(
        f.app
            .change_hunk(&f.id, &path(name), false, 1, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let p = f.app.preview_hunks(&f.id, &path(name), false).unwrap();
    assert_eq!(
        f.app
            .change_hunk(&f.id, &path(name), false, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["show", &format!(":{name}")]), value);
    assert_eq!(fs::read_to_string(f.root.join(name)).unwrap(), value);
}

#[test]
fn unused_filter_configuration_does_not_hide_hunks_and_active_filters_are_never_executed() {
    let f = fixture("file");
    git(
        &f.root,
        &[
            "config",
            "filter.fixture.clean",
            "touch FILTER_SHOULD_NOT_RUN",
        ],
    );
    fs::write(f.root.join("file"), text("one", "two")).unwrap();
    assert!(f.app.preview_hunks(&f.id, &path("file"), false).is_ok());
    fs::write(f.root.join(".gitattributes"), "file filter=fixture\n").unwrap();
    assert!(f.app.preview_hunks(&f.id, &path("file"), false).is_err());
    assert!(!f.root.join("FILTER_SHOULD_NOT_RUN").exists());
}

#[test]
fn patch_command_text_inside_code_is_not_a_second_file_header() {
    let f = fixture("file");
    let value = text("diff --git a/example b/example", "two");
    fs::write(f.root.join("file"), &value).unwrap();
    let p = f.app.preview_hunks(&f.id, &path("file"), false).unwrap();
    assert_eq!(p.headers.len(), 2);
    let result = f
        .app
        .change_hunk(&f.id, &path("file"), false, 0, &p.token)
        .unwrap();
    assert_eq!(result.status, "succeeded", "{result:?}");
    assert!(git(&f.root, &["show", ":file"]).contains("diff --git a/example b/example"));
    let p = f.app.preview_hunks(&f.id, &path("file"), true).unwrap();
    assert_eq!(
        f.app
            .change_hunk(&f.id, &path("file"), true, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(fs::read_to_string(f.root.join("file")).unwrap(), value);
    assert_eq!(git(&f.root, &["diff", "--cached"]), "");
}

#[test]
fn added_and_deleted_text_hunks_roundtrip_without_touching_working_bytes_or_other_staging() {
    let f = fixture("file");
    fs::write(f.root.join("other"), "keep staged\n").unwrap();
    git(&f.root, &["add", "other"]);
    let name = "new tab\tand space.txt";
    let bytes = "new text without final newline";
    fs::write(f.root.join(name), bytes).unwrap();
    let p = f.app.preview_hunks(&f.id, &path(name), false).unwrap();
    assert_eq!(p.headers.len(), 1);
    assert_eq!(
        f.app
            .change_hunk(&f.id, &path(name), false, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["show", &format!(":{name}")]), bytes);
    let p = f.app.preview_hunks(&f.id, &path(name), true).unwrap();
    assert_eq!(
        f.app
            .change_hunk(&f.id, &path(name), true, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["ls-files", "--", name]), "");
    assert_eq!(fs::read_to_string(f.root.join(name)).unwrap(), bytes);
    fs::remove_file(f.root.join("file")).unwrap();
    let p = f.app.preview_hunks(&f.id, &path("file"), false).unwrap();
    assert_eq!(
        f.app
            .change_hunk(&f.id, &path("file"), false, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(git(&f.root, &["ls-files", "--", "file"]), "");
    let p = f.app.preview_hunks(&f.id, &path("file"), true).unwrap();
    assert_eq!(
        f.app
            .change_hunk(&f.id, &path("file"), true, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        git(&f.root, &["show", ":file"]),
        text("base", "base").trim_end()
    );
    assert!(!f.root.join("file").exists());
    assert_eq!(git(&f.root, &["show", ":other"]), "keep staged");
}

#[test]
fn initial_repository_without_index_can_stage_then_unstage_a_text_hunk() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "core.autocrlf", "false"]);
    fs::write(dir.path().join("new"), "first\n").unwrap();
    let app = Application::new(dir.path().join("catalog.json")).unwrap();
    let id = app.add_repository(dir.path()).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    let p = app.preview_hunks(&id, &path("new"), false).unwrap();
    assert_eq!(
        app.change_hunk(&id, &path("new"), false, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let p = app.preview_hunks(&id, &path("new"), true).unwrap();
    assert_eq!(
        app.change_hunk(&id, &path("new"), true, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("new")).unwrap(),
        "first\n"
    );
    assert_eq!(git(dir.path(), &["ls-files", "--", "new"]), "");
}

#[test]
fn new_file_hunks_never_bypass_native_line_ending_or_attribute_conversion() {
    let f = fixture("file");
    fs::write(f.root.join("new"), b"first\r\nsecond\r\n").unwrap();
    let before = fs::read(f.root.join(".git/index")).unwrap();
    git(&f.root, &["config", "core.autocrlf", "true"]);
    assert!(f.app.preview_hunks(&f.id, &path("new"), false).is_err());
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), before);
    git(&f.root, &["config", "core.autocrlf", "false"]);
    fs::write(f.root.join(".gitattributes"), "new text eol=lf\n").unwrap();
    assert!(f.app.preview_hunks(&f.id, &path("new"), false).is_err());
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), before);
    fs::remove_file(f.root.join(".gitattributes")).unwrap();
    let p = f.app.preview_hunks(&f.id, &path("new"), false).unwrap();
    assert_eq!(
        f.app
            .change_hunk(&f.id, &path("new"), false, 0, &p.token)
            .unwrap()
            .status,
        "succeeded"
    );
    let app_oid = git(&f.root, &["rev-parse", ":new"]);
    git(&f.root, &["reset", "--quiet", "HEAD", "--", "new"]);
    git(&f.root, &["add", "new"]);
    assert_eq!(git(&f.root, &["rev-parse", ":new"]), app_oid);
}
