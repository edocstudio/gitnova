//! Opt-in persistent-core soak, confined to fresh synthetic repositories.
use gitnova_core::Application;
use gitnova_domain::*;
use serde_json::json;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn git(root: &Path, args: &[&str]) {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "core.hooksPath=",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "gc.auto=0",
            "-c",
            "pack.threads=1",
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
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
fn seed(root: &Path, count: usize, files: usize) {
    std::fs::create_dir(root).unwrap();
    git(root, &["init", "-b", "main"]);
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "gc.auto=0", "fast-import", "--quiet"])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for i in 0..count {
        let message = format!("Soak needle revision {i}\n");
        let content = format!("revision {i}\n");
        write!(input,"commit refs/heads/main\ncommitter Synthetic Fixture <fixture@example.invalid> {} +0000\ndata {}\n{}",1700000000+i,message.len(),message).unwrap();
        if i == 0 {
            for f in 0..files {
                let value = format!("synthetic tracked file {f}\n");
                write!(
                    input,
                    "M 100644 inline files/file-{f}.txt\ndata {}\n{}\n",
                    value.len(),
                    value
                )
                .unwrap();
            }
        }
        write!(
            input,
            "M 100644 inline history.txt\ndata {}\n{}\n\n",
            content.len(),
            content
        )
        .unwrap();
    }
    input.write_all(b"done\n").unwrap();
    drop(input);
    assert!(child.wait().unwrap().success());
    git(root, &["reset", "--hard", "HEAD"]);
    git(root, &["branch", "release", "HEAD~10"]);
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let seconds = args
        .get(1)
        .map(|s| s.parse::<u64>().unwrap())
        .unwrap_or(1800);
    let commits = args
        .get(2)
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(50000);
    let files = args
        .get(3)
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(10000);
    assert!(
        (60..=7200).contains(&seconds)
            && (100..=100000).contains(&commits)
            && (100..=20000).contains(&files)
    );
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("large");
    seed(&root, commits, files);
    let app = Application::new(temp.path().join("state/catalog.json")).unwrap();
    let mut ids = vec![];
    for i in 0..3 {
        let path = if i == 0 {
            root.clone()
        } else {
            let path = temp.path().join(format!("linked-{i}"));
            git(
                &root,
                &[
                    "worktree",
                    "add",
                    "--detach",
                    path.to_str().unwrap(),
                    "HEAD",
                ],
            );
            path
        };
        let c = app.add_repository(&path).unwrap();
        ids.push(c.workspaces[0].selected_id.clone().unwrap());
    }
    let baseline = app.working_changes(&ids[0]).unwrap();
    assert!(baseline.files.is_empty());
    println!(
        "{}",
        json!({"event":"ready","commits":commits,"trackedFiles":files+1,"locations":ids.len(),"durationSeconds":seconds})
    );
    std::io::stdout().flush().unwrap();
    let start = Instant::now();
    let mut cycle = 0;
    while start.elapsed() < Duration::from_secs(seconds) {
        let id = &ids[cycle % ids.len()];
        let catalog = app.select(id).unwrap();
        let request = CommitSearchRequest {
            workspace_id: catalog.active_workspace_id,
            current_membership_id: id.clone(),
            scope: "workspace".into(),
            selected_ids: vec![],
            query: "needle".into(),
            cursor: None,
        };
        for scenario in [
            "graph",
            "snapshot",
            "search",
            "file_history",
            "code_history",
            "comparison",
        ] {
            let at = Instant::now();
            let mut partial = false;
            match scenario {
                "graph" => {
                    let p = app.graph(id, None).unwrap();
                    assert!(!p.rows.is_empty());
                    app.detail(id, &p.rows[0].commit.oid).unwrap();
                }
                "snapshot" => {
                    assert!(app.snapshot(id).unwrap().changes.files.is_empty());
                }
                "search" => {
                    let p = app.search_commits(&request).unwrap();
                    assert!(!p.hits.is_empty());
                    partial = p.partial;
                }
                mode => {
                    let r = InvestigationRequest {
                        line_start: None,
                        line_end: None,
                        selection: request.clone(),
                        mode: if mode == "comparison" {
                            "compare"
                        } else {
                            "history"
                        }
                        .into(),
                        path: "history.txt".into(),
                        needle: if mode == "code_history" {
                            format!("revision {}", commits - 1)
                        } else {
                            String::new()
                        },
                        source: "main".into(),
                        target: "release".into(),
                    };
                    let p = app.investigate_history(&r).unwrap();
                    assert!(p.evidence.iter().all(|e| e.issue.is_none()), "{p:?}");
                    assert!(!p.hits.is_empty());
                    partial = p.partial;
                }
            }
            println!(
                "{}",
                json!({"event":"measurement","scenario":scenario,"cycle":cycle,"milliseconds":at.elapsed().as_secs_f64()*1000.,"partial":partial,"elapsedSeconds":start.elapsed().as_secs_f64()})
            );
        }
        cycle += 1;
        std::io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_secs(2));
    }
    for id in ids {
        let current = app.working_changes(&id).unwrap();
        assert!(current.files.is_empty());
        assert_eq!(current.status.head, baseline.status.head);
    }
    println!(
        "{}",
        json!({"event":"complete","cycles":cycle,"elapsedSeconds":start.elapsed().as_secs_f64(),"fixtureStatePreserved":true})
    );
}
