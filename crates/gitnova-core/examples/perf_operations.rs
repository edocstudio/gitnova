//! Serial local-transport benchmark; only disposable repositories are changed.
//! Network/provider latency, WebView RSS and cross-product comparisons are not measured here.
use gitnova_core::Application;
use gitnova_domain::*;
use gitnova_git::measure_git;
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
fn measure<T>(rows: &mut Vec<Value>, name: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let (value, metrics) = measure_git(f);
    rows.push(
        json!({"scenario":name,"milliseconds":start.elapsed().as_secs_f64()*1000.,"git":metrics}),
    );
    value
}
fn refresh(app: &Application, id: &str) {
    app.graph(id, None).unwrap();
    app.snapshot(id).unwrap();
    app.stashes(id).unwrap();
}
fn main() {
    let tracked_files = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "500".into())
        .parse::<usize>()
        .unwrap();
    let history_commits = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "0".into())
        .parse::<usize>()
        .unwrap();
    assert!((100..=50000).contains(&tracked_files) && history_commits <= 100000);
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("local");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Perf"),
        ("user.email", "perf@example.invalid"),
        ("core.hooksPath", "/dev/null"),
        ("commit.gpgsign", "false"),
        ("gc.auto", "0"),
        ("pack.threads", "1"),
    ] {
        git(&root, &["config", key, value]);
    }
    if history_commits > 0 {
        let mut child = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["fast-import", "--quiet"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        for i in 0..history_commits {
            let message = format!("Performance history {i}\n");
            let content = format!("revision {i}\n");
            write!(input,"commit refs/heads/main\ncommitter Perf <perf@example.invalid> {} +0000\ndata {}\n{}M 100644 inline history.txt\ndata {}\n{}\n",1700000000+i,message.len(),message,content.len(),content).unwrap();
        }
        input.write_all(b"done\n").unwrap();
        drop(input);
        assert!(child.wait().unwrap().success());
        git(&root, &["reset", "--hard", "HEAD"]);
    }
    for i in 0..tracked_files {
        fs::write(root.join(format!("file-{i}.txt")), format!("base {i}\n")).unwrap();
    }
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);
    git(&root, &["switch", "-c", "feature"]);
    fs::write(root.join("feature.txt"), "feature\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "feature"]);
    let feature = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["switch", "main"]);
    let remote = dir.path().join("remote.git");
    git(
        dir.path(),
        &[
            "clone",
            "--bare",
            root.to_str().unwrap(),
            remote.to_str().unwrap(),
        ],
    );
    // A receive-triggered background repack can race a later local clone while
    // constructing a large fixture. Keep fixture maintenance serial/bounded.
    git(&remote, &["config", "gc.auto", "0"]);
    git(&remote, &["config", "pack.threads", "1"]);
    git(
        &root,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&root, &["fetch", "origin"]);
    git(&root, &["branch", "--set-upstream-to=origin/main", "main"]);
    let app = Application::new(dir.path().join("state/catalog.json")).unwrap();
    let id = app.add_repository(&root).unwrap().workspaces[0]
        .selected_id
        .clone()
        .unwrap();
    let mut rows = Vec::new();
    for branch in ["feature", "main"] {
        let p = measure(&mut rows, &format!("switch_{branch}_preview"), || {
            app.preview_branch_switch(&id, branch).unwrap()
        });
        assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
        let r = measure(&mut rows, &format!("switch_{branch}_apply"), || {
            app.switch_branch(&id, branch, &p.token).unwrap()
        });
        assert_eq!(r.status, "succeeded");
        measure(&mut rows, "switch_refresh", || refresh(&app, &id));
    }
    fs::write(root.join("outgoing.txt"), "outgoing\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "outgoing"]);
    let request = PushRequest {
        branch: "main".into(),
        oid: git(&root, &["rev-parse", "HEAD"]),
        remote: "origin".into(),
        remote_branch: "main".into(),
        set_upstream: false,
        buffered_https: false,
    };
    let p = measure(&mut rows, "push_preview", || {
        app.preview_push(&id, &request).unwrap()
    });
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let r = measure(&mut rows, "push_apply", || {
        app.push_branch(&id, &request, &p.token).unwrap()
    });
    assert_eq!(r.status, "succeeded");
    measure(&mut rows, "push_refresh", || refresh(&app, &id));
    let writer = dir.path().join("writer");
    git(
        dir.path(),
        &["clone", remote.to_str().unwrap(), writer.to_str().unwrap()],
    );
    git(&writer, &["config", "user.name", "Perf"]);
    git(&writer, &["config", "user.email", "perf@example.invalid"]);
    git(&writer, &["config", "gc.auto", "0"]);
    git(&writer, &["config", "pack.threads", "1"]);
    fs::write(writer.join("incoming.txt"), "incoming\n").unwrap();
    git(&writer, &["add", "."]);
    git(&writer, &["commit", "-m", "incoming"]);
    git(&writer, &["push", "origin", "main"]);
    measure(&mut rows, "pull_fetch", || {
        app.fetch_remote(&id, "origin", Some("main")).unwrap()
    });
    let p = measure(&mut rows, "pull_preview", || {
        app.preview_pull_current(&id).unwrap()
    });
    assert!(
        p.switch.blocked_reason.is_none(),
        "{:?}",
        p.switch.blocked_reason
    );
    let r = measure(&mut rows, "pull_apply", || {
        app.pull_current(&id, &p.switch.token).unwrap()
    });
    assert_eq!(r.status, "succeeded");
    measure(&mut rows, "pull_refresh", || refresh(&app, &id));
    let request = MergeRequest {
        rebase_steps: vec![],
        acknowledge_rewrite: false,
        delete_source_branch: false,
        method: MergeMethod::MergeCommit,
        operation: "merge".into(),
        mainline: None,
        source_oid: feature,
        source_ref: Some("refs/heads/feature".into()),
        session_id: String::new(),
    };
    let p = measure(&mut rows, "merge_preview", || {
        app.preview_merge(&id, &request).unwrap()
    });
    assert!(p.blocked_reason.is_none(), "{:?}", p.blocked_reason);
    let r = measure(&mut rows, "merge_begin", || {
        app.begin_merge(&id, &p.request, &p.token).unwrap()
    });
    assert_eq!(r.status, "pending");
    let action = MergeAction {
        kind: "finish".into(),
        path: None,
        choice: None,
        content: None,
        message: "Merge feature".into(),
    };
    let p = measure(&mut rows, "merge_finish_preview", || {
        app.preview_merge_action(&id, &action).unwrap()
    });
    let r = measure(&mut rows, "merge_finish", || {
        app.apply_merge_action(&id, &action, &p.token).unwrap()
    });
    assert_eq!(r.status, "succeeded");
    measure(&mut rows, "merge_refresh", || refresh(&app, &id));
    assert!(git(&root, &["status", "--porcelain"]).is_empty());
    assert_eq!(
        git(&root, &["show", "-s", "--format=%P", "HEAD"])
            .split_whitespace()
            .count(),
        2
    );
    println!("{}",serde_json::to_string_pretty(&json!({"tracked_files":tracked_files,"history_commits":history_commits,"transport":"local temporary bare repository","build":"debug","measurements":rows})).unwrap());
}
