//! Black-box MCP handshake against the executable and disposable real Git data.
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{channel, Receiver},
    time::Duration,
};
struct Client {
    child: Child,
    input: Option<ChildStdin>,
    output: Receiver<String>,
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Client {
    fn start(config: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gitnova-mcp"))
            .args(["--config", config.to_str().unwrap(), "serve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if tx.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            input,
            output: rx,
        }
    }
    fn send(&mut self, v: Value) {
        writeln!(self.input.as_mut().unwrap(), "{v}").unwrap();
        self.input.as_mut().unwrap().flush().unwrap();
    }
    fn response(&self) -> Value {
        let line = self
            .output
            .recv_timeout(Duration::from_secs(25))
            .expect("MCP response timeout");
        serde_json::from_str(&line).expect("stdout must contain JSON-RPC only")
    }
    fn request(&mut self, id: u32, method: &str, params: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        self.response()
    }
    fn init(&mut self) {
        let r=self.request(1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"smoke","version":"1"}}));
        assert_eq!(r["result"]["serverInfo"]["name"], "gitnova-mcp");
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    }
}
#[test]
fn executable_protocol_permissions_and_revocation() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    let out = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["init", "-b", "main"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let config = dir.path().join("policy.json");
    let mut c = Client::start(&config);
    c.init();
    let tools = c.request(2, "tools/list", json!({}));
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 6);
    for t in tools["result"]["tools"].as_array().unwrap() {
        assert_eq!(t["annotations"]["readOnlyHint"], true);
        assert_eq!(t["inputSchema"]["additionalProperties"], false);
    }
    let list = c.request(
        3,
        "tools/call",
        json!({"name":"list_repositories","arguments":{}}),
    );
    assert_eq!(
        list["result"]["structuredContent"]["data"]["repositories"],
        json!([])
    );
    let denied = c.request(
        4,
        "tools/call",
        json!({"name":"get_status","arguments":{"repository_id":"unknown"}}),
    );
    assert_eq!(denied["result"]["isError"], true);
    let schema = c.request(
        5,
        "tools/call",
        json!({"name":"get_status","arguments":{"repository_id":"unknown","shell":"echo bad"}}),
    );
    assert!(schema.get("error").is_some());
    let auth = Command::new(env!("CARGO_BIN_EXE_gitnova-mcp"))
        .args([
            "--config",
            config.to_str().unwrap(),
            "authorize",
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(auth.status.success());
    assert!(auth.stdout.is_empty());
    let list = c.request(
        6,
        "tools/call",
        json!({"name":"list_repositories","arguments":{}}),
    );
    let id = list["result"]["structuredContent"]["data"]["repositories"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let status = c.request(
        7,
        "tools/call",
        json!({"name":"get_status","arguments":{"repository_id":id}}),
    );
    assert_eq!(
        status["result"]["structuredContent"]["data"]["branch"],
        "main"
    );
    let schema = c.request(
        8,
        "tools/call",
        json!({"name":"get_diff","arguments":{"repository_id":id,"limit":"many"}}),
    );
    assert!(schema.get("error").is_some() || schema["result"]["isError"] == true);
    for invalid in [
        json!({"limit":null}),
        json!({"limit":101}),
        json!({"offset":-1}),
    ] {
        let r = c.request(
            80,
            "tools/call",
            json!({"name":"list_repositories","arguments":invalid}),
        );
        assert_eq!(r["error"]["code"], -32602);
    }
    fs::write(root.join("source.txt"), "first line\nsecond line\n").unwrap();
    assert!(Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["add", "."])
        .output()
        .unwrap()
        .status
        .success());
    assert!(Command::new("git")
        .arg("-C")
        .arg(&root)
        .args([
            "-c",
            "user.name=Synthetic",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "MCP fixture"
        ])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" }
        )
        .output()
        .unwrap()
        .status
        .success());
    fs::write(root.join("source.txt"), "changed line\nsecond line\n").unwrap();
    let before_index = fs::read(root.join(".git/index")).unwrap();
    let before_head = fs::read(root.join(".git/refs/heads/main")).unwrap();
    for (i, name, args) in [
        (
            20,
            "get_diff",
            json!({"repository_id":id,"path":"source.txt"}),
        ),
        (
            21,
            "search_commits",
            json!({"repository_id":id,"message":"MCP fixture"}),
        ),
        (
            22,
            "get_line_history",
            json!({"repository_id":id,"path":"source.txt","line_start":1,"line_end":2}),
        ),
        (23, "get_conflict_summary", json!({"repository_id":id})),
    ] {
        let response = c.request(i, "tools/call", json!({"name":name,"arguments":args}));
        assert_ne!(response["result"]["isError"], true, "{name}: {response}");
        assert_eq!(response["result"]["structuredContent"]["schemaVersion"], 1);
        assert!(response["result"]["structuredContent"]["data"].is_object());
    }
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), before_index);
    assert_eq!(
        fs::read(root.join(".git/refs/heads/main")).unwrap(),
        before_head
    );
    assert_eq!(
        fs::read_to_string(root.join("source.txt")).unwrap(),
        "changed line\nsecond line\n"
    );
    let unknown = c.request(
        24,
        "tools/call",
        json!({"name":"execute_shell","arguments":{"command":"push"}}),
    );
    assert!(unknown.get("error").is_some());
    fs::write(&config, b"{\"repositories\":[]}").unwrap();
    let revoked = c.request(
        9,
        "tools/call",
        json!({"name":"get_status","arguments":{"repository_id":id}}),
    );
    assert_eq!(revoked["result"]["isError"], true);
    // A malformed configuration must fail closed rather than use a cached grant.
    fs::write(&config, b"broken").unwrap();
    assert_eq!(
        c.request(
            10,
            "tools/call",
            json!({"name":"list_repositories","arguments":{}})
        )["result"]["isError"],
        true
    );
    c.input.take();
    let start = std::time::Instant::now();
    loop {
        if c.child.try_wait().unwrap().is_some() {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "disconnect did not shut down server"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[cfg(unix)]
#[test]
fn cancellation_reaps_a_slow_git_process() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    assert!(Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["init", "-b", "main"])
        .status()
        .unwrap()
        .success());
    let config = dir.path().join("policy.json");
    assert!(Command::new(env!("CARGO_BIN_EXE_gitnova-mcp"))
        .args([
            "--config",
            config.to_str().unwrap(),
            "authorize",
            root.to_str().unwrap()
        ])
        .output()
        .unwrap()
        .status
        .success());
    let policy: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    let id = policy["repositories"][0]["id"].as_str().unwrap();
    // Task-owned diagnostic executable simulates a stalled Git read. No shell
    // strings supplied by clients and no real credentials/repositories involved.
    let bin = dir.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let marker = dir.path().join("pid");
    let shim = bin.join("git");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\necho $$ > '{}'\nexec /bin/sleep 30\n",
            marker.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_gitnova-mcp"))
        .args(["--config", config.to_str().unwrap(), "serve"])
        .env("PATH", &bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let input = child.stdin.take();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for l in BufReader::new(stdout).lines() {
            if tx.send(l.unwrap()).is_err() {
                break;
            }
        }
    });
    let mut c = Client {
        child,
        input,
        output: rx,
    };
    c.init();
    c.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_status","arguments":{"repository_id":id}}}));
    let start = std::time::Instant::now();
    while !marker.exists() {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    c.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2,"reason":"test"}}));
    let pid = fs::read_to_string(&marker).unwrap();
    let start = std::time::Instant::now();
    loop {
        let alive = Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        if !alive {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "cancelled Git child still alive"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
