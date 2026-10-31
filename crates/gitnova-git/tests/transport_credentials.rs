#![cfg(unix)]
//! Real Git + loopback HTTPS challenge. No user's Keychain, network remote or repo is used.
use gitnova_domain::*;
use gitnova_git::{GitCli, RemoteWriter};
use std::{
    fs,
    io::{BufRead, BufReader},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, Stdio},
};
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let out = command
        .arg("-C")
        .arg(root)
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
        "Fixture Git failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
#[test]
fn connected_helper_authenticates_real_https_push_and_fetch_without_persistent_config() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("openssl.cnf"),"[req]\ndistinguished_name=dn\nx509_extensions=v3\nprompt=no\n[dn]\nCN=localhost\n[v3]\nsubjectAltName=DNS:localhost\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,digitalSignature,keyEncipherment,keyCertSign\n").unwrap();
    assert!(Command::new("openssl")
        .current_dir(root)
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "key.pem",
            "-out",
            "cert.pem",
            "-days",
            "1",
            "-config",
            "openssl.cnf"
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success());
    let script = r#"
import http.server,ssl,sys,base64,pathlib,subprocess,os
root=pathlib.Path(sys.argv[1])
class Handler(http.server.BaseHTTPRequestHandler):
 protocol_version='HTTP/1.1'
 def log_message(self,*args): pass
 def do_GET(self): self.respond()
 def do_POST(self): self.respond()
 def respond(self):
  self.connection.settimeout(15)
  # Drain the request before a challenge/rejection, so TLS shutdown does not
  # race an in-flight upload and turn the intended response into a broken pipe.
  chunked=self.headers.get('Transfer-Encoding','').lower()=='chunked'
  if chunked:
   parts=[];total=0
   while True:
    size=int(self.rfile.readline(8192).split(b';',1)[0],16)
    if not size:
     while True:
      trailer=self.rfile.readline(8192)
      if trailer==b'\r\n':break
      if not trailer:raise ValueError('incomplete chunk trailer')
     break
    total+=size
    if total>16*1024*1024:raise ValueError('fixture upload too large')
    parts.append(self.rfile.read(size))
    if self.rfile.read(2)!=b'\r\n':raise ValueError('invalid chunk')
   data=b''.join(parts)
  else:
   size=int(self.headers.get('Content-Length','0'))
   if not 0<=size<=16*1024*1024:raise ValueError('fixture upload too large')
   data=self.rfile.read(size)
  if self.headers.get('Authorization')!='Basic '+base64.b64encode(b'fixture:fixture-test-only').decode():
   self.send_response(401);self.send_header('WWW-Authenticate','Basic realm="fixture"');self.send_header('Content-Length','0');self.end_headers();return
  with (root/'authenticated').open('a') as f:f.write('ok\n')
  if self.path.startswith('/redirect.git'):
   self.send_response(302);self.send_header('Location',f'https://localhost:{self.server.server_port}/outside.git/info/refs?service=git-upload-pack');self.send_header('Content-Length','0');self.end_headers();return
  if self.path.startswith('/outside.git'):
   (root/'leaked').write_text('unexpected authenticated redirect');self.send_response(403);self.send_header('Content-Length','0');self.end_headers();return
  if (root/'reject-chunked').exists() and chunked:
   with (root/'rejected-chunked').open('a') as f:f.write('rejected\n')
   self.send_response(400);self.send_header('Content-Length','0');self.end_headers();return
  path,_,query=self.path.partition('?')
  env={**{k:v for k,v in os.environ.items() if not k.startswith('GIT_')},'GIT_PROJECT_ROOT':str(root),'GIT_HTTP_EXPORT_ALL':'1','PATH_INFO':path,'QUERY_STRING':query,'REQUEST_METHOD':self.command,'REMOTE_USER':'fixture','REMOTE_ADDR':'127.0.0.1','CONTENT_TYPE':self.headers.get('Content-Type',''),'CONTENT_LENGTH':str(len(data)),'GIT_CONFIG_GLOBAL':'/dev/null','GIT_CONFIG_NOSYSTEM':'1'}
  result=subprocess.run([sys.argv[2]],input=data,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,env=env,timeout=10)
  headers,sep,body=result.stdout.partition(b'\r\n\r\n')
  if not sep:headers,sep,body=result.stdout.partition(b'\n\n')
  pairs=[line.split(':',1) for line in headers.decode().splitlines() if ':' in line]
  status=next((int(v.strip().split()[0]) for k,v in pairs if k.lower()=='status'),200)
  self.send_response(status)
  for k,v in pairs:
   if k.lower() not in ['status','content-length']:self.send_header(k,v.strip())
  self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
server=http.server.ThreadingHTTPServer(('localhost',0),Handler)
context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(root/'cert.pem',root/'key.pem');server.socket=context.wrap_socket(server.socket,server_side=True)
print(server.server_port,flush=True);server.serve_forever()
"#;
    git(root, &["init", "--bare", "remote.git"]);
    git(
        &root.join("remote.git"),
        &["config", "core.hooksPath", "/dev/null"],
    );
    git(
        &root.join("remote.git"),
        &["config", "http.receivepack", "true"],
    );
    let backend = Path::new(&git(root, &["--exec-path"])).join("git-http-backend");
    assert!(backend.exists());
    fs::write(root.join("server.py"), script).unwrap();
    let mut server = Server(
        Command::new("python3")
            .arg(root.join("server.py"))
            .arg(root)
            .arg(backend)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut port = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut port)
        .unwrap();
    let port = port.trim().parse::<u16>().unwrap();
    let url = format!("https://localhost:{port}/remote.git");
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "Fixture"]);
    git(root, &["config", "user.email", "fixture@example.invalid"]);
    git(root, &["config", "core.hooksPath", "/dev/null"]);
    git(root, &["config", "http.proxy", ""]);
    git(
        root,
        &[
            "config",
            "http.sslCAInfo",
            root.join("cert.pem").to_str().unwrap(),
        ],
    );
    fs::write(root.join("file.txt"), "fixture\n").unwrap();
    git(root, &["add", "file.txt"]);
    git(root, &["commit", "-m", "fixture"]);
    git(root, &["remote", "add", "origin", &url]);
    let helper = root.join("native helper's path");
    fs::write(&helper,"#!/bin/sh\n[ \"$1\" = --gitnova-credential-helper ] || exit 1\n[ \"$2\" = get ] || exit 0\ncat >/dev/null\nprintf 'username=fixture\\npassword=fixture-test-only\\n\\n'\n").unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    gitnova_git::install_transport_credential_helper(&helper).unwrap();
    git(root, &["config", "credential.username", "legacy-account"]);
    let credential_file = root.join("unexpected-credential-store");
    git(
        root,
        &[
            "config",
            &format!("credential.https://localhost:{port}/.helper"),
            &format!("store --file={}", credential_file.display()),
        ],
    );
    let before = fs::read(root.join(".git/config")).unwrap();
    let request = PushRequest {
        branch: "main".into(),
        oid: git(root, &["rev-parse", "HEAD"]),
        remote: "origin".into(),
        remote_branch: "main".into(),
        set_upstream: false,
        buffered_https: false,
    };
    let preview = GitCli.preview_push(root, &request).unwrap();
    assert!(preview.blocked_reason.is_none());
    assert!(preview.remote_oid.is_none());
    GitCli.apply_push(root, &request, &preview.token).unwrap();
    assert_eq!(
        git(&root.join("remote.git"), &["rev-parse", "refs/heads/main"]),
        request.oid
    );
    GitCli.fetch_remote(root, "origin", None).unwrap();
    assert!(
        fs::read_to_string(root.join("authenticated"))
            .unwrap()
            .lines()
            .count()
            >= 4
    );
    assert_eq!(fs::read(root.join(".git/config")).unwrap(), before);
    assert!(!credential_file.exists());
    assert!(!root.join(".git-credentials").exists());
    // A real >1 MiB upload reproduces a server rejecting chunked POSTs.
    // The compatibility choice is explicit and must receive a new review token.
    fs::write(root.join("reject-chunked"), "enabled").unwrap();
    let mut state = 0x0123_4567_89ab_cdefu64;
    let payload: Vec<u8> = (0..2 * 1024 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect();
    fs::write(root.join("payload.bin"), payload).unwrap();
    git(root, &["add", "payload.bin"]);
    git(root, &["commit", "-m", "Large reviewed upload"]);
    let large = PushRequest {
        oid: git(root, &["rev-parse", "HEAD"]),
        ..request.clone()
    };
    let normal = GitCli.preview_push(root, &large).unwrap();
    assert!(normal.blocked_reason.is_none());
    let error = GitCli.apply_push(root, &large, &normal.token).unwrap_err();
    assert!(error.message.contains("400"), "{error:?}");
    assert_eq!(
        fs::read_to_string(root.join("rejected-chunked"))
            .unwrap()
            .lines()
            .count(),
        1,
        "A failed upload must not be replayed automatically"
    );
    assert_eq!(
        git(&root.join("remote.git"), &["rev-parse", "refs/heads/main"]),
        request.oid
    );
    let buffered = PushRequest {
        buffered_https: true,
        ..large
    };
    assert_eq!(
        GitCli
            .apply_push(root, &buffered, &normal.token)
            .unwrap_err()
            .code,
        "stale_preview"
    );
    let compatible = GitCli.preview_push(root, &buffered).unwrap();
    assert_eq!(compatible.remote_oid.as_ref(), Some(&request.oid));
    assert_ne!(compatible.token, normal.token);
    GitCli
        .apply_push(root, &buffered, &compatible.token)
        .unwrap();
    assert_eq!(
        git(&root.join("remote.git"), &["rev-parse", "refs/heads/main"]),
        buffered.oid
    );
    assert_eq!(fs::read(root.join(".git/config")).unwrap(), before);
    assert!(!credential_file.exists());
    git(
        root,
        &[
            "remote",
            "add",
            "redirect",
            &format!("https://localhost:{port}/redirect.git"),
        ],
    );
    git(
        root,
        &[
            "config",
            &format!("http.https://localhost:{port}/.followRedirects"),
            "true",
        ],
    );
    let redirect = PushRequest {
        remote: "redirect".into(),
        ..request
    };
    assert!(GitCli.preview_push(root, &redirect).is_err());
    assert!(!root.join("leaked").exists());
}
