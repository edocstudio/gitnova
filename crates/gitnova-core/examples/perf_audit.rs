//! Serial, opt-in read benchmark. All fixtures and catalog writes are temporary.
//! cargo run -p gitnova-core --example perf_audit -j 1 -- 2000
use gitnova_core::Application;
use gitnova_domain::*;
use serde_json::json;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};

fn git(path: &Path, args: &[&str]) {
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
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn fixture(path: &Path, count: usize) {
    std::fs::create_dir(path).unwrap();
    git(path, &["init", "-b", "main"]);
    let mut child = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for i in 0..count {
        let message = format!("Performance needle {i}\n");
        let content = format!("version {i}\n");
        write!(input, "commit refs/heads/main\ncommitter Benchmark <bench@example.invalid> {} +0000\ndata {}\n{}M 100644 inline file.txt\ndata {}\n{}\n", 1700000000 + i, message.len(), message, content.len(), content).unwrap();
    }
    input.write_all(b"done\n").unwrap();
    drop(input);
    assert!(child.wait().unwrap().success());
    git(path, &["reset", "--hard", "HEAD"]);
}
fn rss() -> Option<u64> {
    #[cfg(unix)]
    {
        let out = Command::new("/bin/ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()?;
        String::from_utf8(out.stdout).ok()?.trim().parse().ok()
    }
    #[cfg(not(unix))]
    {
        None
    }
}
fn measure<T>(rows: &mut Vec<serde_json::Value>, scenario: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = f();
    let ms = start.elapsed().as_secs_f64() * 1000.;
    rows.push(json!({"scenario":scenario,"milliseconds":ms,"core_rss_kib":rss()}));
    value
}
fn main() {
    let count = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "2000".into())
        .parse::<usize>()
        .unwrap();
    assert!((100..=20000).contains(&count));
    let temp = tempfile::tempdir().unwrap();
    fixture(&temp.path().join("one"), count);
    fixture(&temp.path().join("two"), 100);
    let state = temp.path().join("app/state.json");
    let app = Application::new(state.clone()).unwrap();
    let mut ids = vec![];
    for name in ["one", "two"] {
        let c = app.add_repository(&temp.path().join(name)).unwrap();
        ids.push(c.workspaces[0].selected_id.clone().unwrap());
    }
    let mut rows = vec![];
    let page = measure(&mut rows, "graph_initial", || {
        app.graph(&ids[0], None).unwrap()
    });
    let oid = &page.rows[0].commit.oid;
    measure(&mut rows, "commit_detail", || {
        app.detail(&ids[0], oid).unwrap()
    });
    measure(&mut rows, "working_snapshot", || {
        app.snapshot(&ids[0]).unwrap()
    });
    let catalog = app.select(&ids[0]).unwrap();
    let request = CommitSearchRequest {
        workspace_id: catalog.active_workspace_id,
        current_membership_id: ids[0].clone(),
        scope: "workspace".into(),
        selected_ids: vec![],
        query: "needle".into(),
        cursor: None,
    };
    let cold = measure(&mut rows, "workspace_search_cold", || {
        app.search_commits(&request).unwrap()
    });
    assert_eq!(cold.total_matches, count + 100);
    for i in 0..10 {
        let result = measure(&mut rows, &format!("workspace_search_warm_{i}"), || {
            app.search_commits(&request).unwrap()
        });
        assert_eq!(result.total_matches, cold.total_matches);
        assert_eq!(result.hits[0].commit.oid, cold.hits[0].commit.oid);
        measure(&mut rows, &format!("switch_graph_{i}"), || {
            app.graph(&ids[i % 2], None).unwrap()
        });
    }
    drop(app);
    let reopened = Application::new(state).unwrap();
    measure(&mut rows, "workspace_search_persisted_restart", || {
        reopened.search_commits(&request).unwrap()
    });
    println!("{}", serde_json::to_string_pretty(&json!({"profile":"debug core only; excludes WebKit, network, large binary patches and recovery copy", "commits": count + 100, "measurements":rows})).unwrap());
}
