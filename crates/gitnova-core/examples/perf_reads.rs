//! Serial, opt-in read benchmark. All fixtures and catalog writes are temporary.
//! cargo run -p gitnova-core --example perf_reads -j 1
use gitnova_core::Application;
use gitnova_domain::NativePath;
use gitnova_git::measure_git;
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
            if cfg!(windows) {
                "core.hooksPath=NUL"
            } else {
                "core.hooksPath=/dev/null"
            },
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
fn sample<T: serde::Serialize>(
    rows: &mut Vec<serde_json::Value>,
    name: &str,
    f: impl FnOnce() -> T,
) -> T {
    let start = Instant::now();
    let (value, git) = measure_git(f);
    let execute_ms = start.elapsed().as_secs_f64() * 1000.;
    let encode = Instant::now();
    let bytes = serde_json::to_vec(&value).unwrap();
    rows.push(json!({"scenario":name,"executionMs":execute_ms,"serializationProbeMs":encode.elapsed().as_secs_f64()*1000.,"bytes":bytes.len(),"git":git}));
    value
}
fn main() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    fixture(&root, 2000);
    git(&root, &["config", "user.name", "Benchmark"]);
    git(&root, &["config", "user.email", "bench@example.invalid"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    git(&root, &["branch", "feature"]);
    let mut rows = vec![];
    // Focused, repeatable graph measurements without running unrelated writes.
    if std::env::args().any(|arg| arg == "--graph-only") {
        for round in 0..20 {
            let app =
                Application::new(temp.path().join(format!("graph-{round}/catalog.json"))).unwrap();
            let catalog = app.add_repository(&root).unwrap();
            let id = catalog.workspaces[0].selected_id.as_ref().unwrap();
            let first = sample(&mut rows, "graph_first_cold_app", || {
                app.graph(id, None).unwrap()
            });
            sample(&mut rows, "graph_first_warm", || {
                app.graph(id, None).unwrap()
            });
            sample(&mut rows, "graph_next", || {
                app.graph(id, first.next.clone()).unwrap()
            });
            sample(&mut rows, "graph_refresh", || app.graph(id, None).unwrap());
        }
        println!("{}", serde_json::to_string_pretty(&json!({"schemaVersion":1,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"fixture":{"commits":2000,"trackedFiles":1,"untrackedFiles":0},"samples":rows,"limits":["Cold means fresh Application/catalog, not cold OS disk cache.","Serial core reads exclude native queue, IPC, UI and network."]})).unwrap());
        return;
    }
    for round in 0..20 {
        let app =
            Application::new(temp.path().join(format!("state-{round}/catalog.json"))).unwrap();
        let catalog = sample(&mut rows, "open_registered_cold_app", || {
            app.add_repository(&root).unwrap()
        });
        let id = catalog.workspaces[0].selected_id.as_ref().unwrap();
        let first = sample(&mut rows, "graph_first_cold_app", || {
            app.graph(id, None).unwrap()
        });
        sample(&mut rows, "graph_first_warm", || {
            app.graph(id, None).unwrap()
        });
        let mut page = sample(&mut rows, "graph_next", || {
            app.graph(id, first.next.clone()).unwrap()
        });
        for _ in 0..7 {
            page = app.graph(id, page.next).unwrap();
        }
        // Same deep refresh policy as the baseline UI: first 1,000 rows eagerly.
        sample(&mut rows, "deep_refresh_first_page", || {
            app.graph(id, None).unwrap()
        });
        sample(&mut rows, "deep_refresh_all_ten_pages", || {
            let mut p = app.graph(id, None).unwrap();
            for _ in 0..9 {
                p = app.graph(id, p.next).unwrap();
            }
            p
        });
        sample(&mut rows, "detail", || {
            app.detail(id, &first.rows[0].commit.oid).unwrap()
        });
        sample(&mut rows, "snapshot_clean", || app.snapshot(id).unwrap());
        std::fs::write(root.join("file.txt"), "changed\n".repeat(20000)).unwrap();
        std::fs::write(root.join("untracked.txt"), "new file\n").unwrap();
        sample(&mut rows, "snapshot_dirty_untracked", || {
            app.snapshot(id).unwrap()
        });
        let file = NativePath::new(Path::new("file.txt"));
        let preview = sample(&mut rows, "large_file_preview", || {
            app.file_preview(id, &file, false).unwrap()
        });
        sample(&mut rows, "stage", || {
            app.change_index(id, &file, true, &preview.token).unwrap()
        });
        let staged = app.file_preview(id, &file, true).unwrap();
        app.change_index(id, &file, false, &staged.token).unwrap();
        git(&root, &["restore", "file.txt"]);
        std::fs::remove_file(root.join("untracked.txt")).unwrap();
        for branch in ["feature", "main"] {
            let preview = sample(&mut rows, "switch_preview", || {
                app.preview_branch_switch(id, branch).unwrap()
            });
            assert!(preview.blocked_reason.is_none());
            sample(&mut rows, "switch_apply", || {
                app.switch_branch(id, branch, &preview.token).unwrap()
            });
        }
    }
    // Related identities and dirty/untracked scans use only disposable checkouts.
    let child = temp.path().join("child-source");
    fixture(&child, 10);
    let parent = temp.path().join("parent");
    fixture(&parent, 120);
    git(&parent, &["config", "user.name", "Benchmark"]);
    git(&parent, &["config", "user.email", "bench@example.invalid"]);
    git(
        &parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child.to_str().unwrap(),
            "child",
        ],
    );
    git(&parent, &["commit", "-am", "Record child"]);
    let linked = temp.path().join("linked");
    git(
        &parent,
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            linked.to_str().unwrap(),
            "HEAD",
        ],
    );
    let app = Application::new(temp.path().join("related/catalog.json")).unwrap();
    let catalog = app.add_repository(&parent).unwrap();
    let id = catalog.workspaces[0].selected_id.as_ref().unwrap();
    for index in 0..1000 {
        std::fs::write(parent.join(format!("untracked-{index}.txt")), "synthetic\n").unwrap();
    }
    for _ in 0..20 {
        sample(&mut rows, "snapshot_submodule_1000_untracked", || {
            app.snapshot(id).unwrap()
        });
        sample(&mut rows, "submodule_discovery", || {
            app.submodules(id).unwrap()
        });
        sample(&mut rows, "worktree_list", || app.worktrees(id).unwrap());
    }
    println!("{}",serde_json::to_string_pretty(&json!({"schemaVersion":1,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"fixture":{"commits":2000,"trackedFiles":1,"untrackedFiles":1,"largeDiffLines":20000,"relatedFixture":{"parentCommits":121,"childCommits":10,"submodules":1,"linkedWorktrees":1,"untrackedFiles":1000}},"samples":rows,"limits":["Fresh Application is cold application cache, not cold OS disk cache.","Serial core and serialization probes exclude native queue, IPC, UI and real network."]})).unwrap());
}
