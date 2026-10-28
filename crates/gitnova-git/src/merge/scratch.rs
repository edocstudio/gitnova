//! Isolated object writes for prediction/diff. Source objects are read-only alternates.
use crate::{GitCli, RepositoryReader};
use gitnova_domain::*;
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};
pub(crate) struct Scratch {
    pub dir: tempfile::TempDir,
    alternate: OsString,
}
pub(super) fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    match std::fs::File::open(path) {
        Ok(file) => {
            let mut bytes = vec![];
            file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
            if bytes.len() > limit {
                return Err(AppError::new(
                    "safety_budget",
                    "Merge content exceeds its bounded preview/recovery budget.",
                ));
            }
            Ok(bytes)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}
pub(crate) fn attributes(git: &GitCli, root: &Path) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut info = git
        .read(
            root,
            &[
                "rev-parse".as_ref(),
                "--path-format=absolute".as_ref(),
                "--git-path".as_ref(),
                "info/attributes".as_ref(),
            ],
            65536,
        )?
        .bytes;
    if info.last() == Some(&b'\n') {
        info.pop();
    }
    let info = read_file(&crate::path_from_bytes(info)?, 1024 * 1024)?;
    let global = if let Some(path) =
        git.optional_text(root, &["config", "--path", "--get", "core.attributesFile"])?
    {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
            })
            .join("git/attributes")
    };
    // A linked worktree has a .git *file*. Git treats a relative attributes
    // filename underneath that file as absent, just like a missing file.
    let global = match std::fs::metadata(&global) {
        Err(error) if error.kind() == std::io::ErrorKind::NotADirectory => vec![],
        _ => read_file(&global, 1024 * 1024)?,
    };
    Ok((info, global))
}
impl Scratch {
    pub(crate) fn remove_stash_record(
        &self,
        tip: &[u8],
        log: &[u8],
        index: usize,
    ) -> Result<(Option<Vec<u8>>, Vec<u8>)> {
        std::fs::create_dir_all(self.dir.path().join("refs"))?;
        std::fs::create_dir_all(self.dir.path().join("logs/refs"))?;
        std::fs::write(self.dir.path().join("refs/stash"), tip)?;
        std::fs::write(self.dir.path().join("logs/refs/stash"), log)?;
        self.run(
            &[
                "reflog".into(),
                "delete".into(),
                "--rewrite".into(),
                "--updateref".into(),
                format!("refs/stash@{{{index}}}").into(),
            ],
            None,
            false,
        )?;
        let log = std::fs::read(self.dir.path().join("logs/refs/stash"))?;
        let tip = if log.is_empty() {
            None
        } else {
            Some(std::fs::read(self.dir.path().join("refs/stash"))?)
        };
        Ok((tip, log))
    }
    pub fn new(git: &GitCli, root: &Path) -> Result<Self> {
        let d = git.discover(root)?;
        if !["sha1", "sha256"].contains(&d.object_format.as_str()) {
            return Err(AppError::new("merge", "Unsupported object format."));
        }
        let objects = std::fs::canonicalize(d.common_dir.join("objects"))?;
        #[cfg(unix)]
        let bytes = {
            use std::os::unix::ffi::OsStrExt;
            objects.as_os_str().as_bytes().to_vec()
        };
        #[cfg(windows)]
        let bytes = objects
            .to_str()
            .ok_or_else(|| AppError::new("encoding", "Cannot represent object-store path."))?
            .replace('\\', "/")
            .into_bytes();
        let alternate = OsString::from(format!(
            "\"{}\"",
            bytes
                .iter()
                .map(|b| format!("\\{b:03o}"))
                .collect::<String>()
        ));
        let this = Self {
            dir: tempfile::tempdir()?,
            alternate,
        };
        std::fs::create_dir(this.dir.path().join("empty-template"))?;
        this.run(
            &[
                "init".into(),
                "--bare".into(),
                format!("--object-format={}", d.object_format).into(),
                "--template=empty-template".into(),
                ".".into(),
            ],
            None,
            false,
        )?;
        let (info, global) = attributes(git, root)?;
        std::fs::create_dir_all(this.dir.path().join("info"))?;
        std::fs::write(this.dir.path().join("info/attributes"), info)?;
        std::fs::write(this.dir.path().join("global-attributes"), global)?;
        let configured=git.read(root,&["config".as_ref(),"--null".as_ref(),"--get-regexp".as_ref(),r"^(merge\.(renames|renamelimit|directoryrenames|conflictstyle|default)|core\.(autocrlf|eol|safecrlf))$".as_ref()],65536);
        let bytes = match configured {
            Ok(o) => o.bytes,
            Err(e) if e.code == "git_exit_1" => vec![],
            Err(e) => return Err(e),
        };
        for item in bytes.split(|b| *b == 0).filter(|i| !i.is_empty()) {
            let sep = item
                .iter()
                .position(|b| *b == b'\n')
                .ok_or_else(|| AppError::new("git_parse", "Invalid merge configuration"))?;
            let key = String::from_utf8(item[..sep].to_vec())
                .map_err(|_| AppError::new("encoding", "Invalid merge configuration"))?;
            let value = String::from_utf8(item[sep + 1..].to_vec())
                .map_err(|_| AppError::new("encoding", "Invalid merge configuration"))?;
            this.run(&["config".into(), key.into(), value.into()], None, false)?;
        }
        Ok(this)
    }
    pub fn run(
        &self,
        args: &[OsString],
        input: Option<&[u8]>,
        conflict_ok: bool,
    ) -> Result<Vec<u8>> {
        self.run_limited(args, input, conflict_ok, 4 * 1024 * 1024, false)
    }
    fn run_limited(
        &self,
        args: &[OsString],
        input: Option<&[u8]>,
        conflict_ok: bool,
        limit: usize,
        partial: bool,
    ) -> Result<Vec<u8>> {
        let mut cmd = Command::new(crate::executable::git_executable());
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                cmd.env_remove(key);
            }
        }
        cmd.args([
            "--no-pager",
            "--no-optional-locks",
            "--literal-pathspecs",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "color.ui=false",
            "-c",
            "core.quotePath=false",
            "-c",
            "protocol.allow=never",
            "-c",
        ])
        .arg(format!(
            "core.attributesFile={}",
            self.dir.path().join("global-attributes").display()
        ))
        .arg("-c")
        .arg(format!(
            "core.hooksPath={}",
            self.dir.path().join("empty-template").display()
        ))
        .arg("-C")
        .arg(self.dir.path())
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_ALTERNATE_OBJECT_DIRECTORIES", &self.alternate)
        .env("GIT_AUTHOR_NAME", "GitNova Preview")
        .env("GIT_AUTHOR_EMAIL", "preview@gitnova.invalid")
        .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_COMMITTER_NAME", "GitNova Preview")
        .env("GIT_COMMITTER_EMAIL", "preview@gitnova.invalid")
        .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_ALLOW_PROTOCOL", "")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let mut child = cmd.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::new("git", "Missing output"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::new("git", "Missing error output"))?;
        let (wake_tx, wake_rx) = mpsc::channel();
        let drain = |mut pipe: Box<dyn Read + Send>| {
            let wake = wake_tx.clone();
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let mut bytes = vec![];
                let result = pipe
                    .by_ref()
                    .take(limit as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map(|_| bytes);
                let _ = tx.send(result);
                let _ = wake.send(());
            });
            rx
        };
        let out = drain(Box::new(stdout));
        let err = drain(Box::new(stderr));
        drop(wake_tx);
        if let Some(input) = input {
            if let Err(error) = child
                .stdin
                .take()
                .ok_or_else(|| AppError::new("git", "Missing input"))?
                .write_all(input)
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
        let status = match crate::wait::wait_for_exit(
            &mut child,
            Duration::from_secs(20),
            &wake_rx,
            Duration::from_millis(10),
        )? {
            Some(status) => status,
            None => {
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::new(
                    "timeout",
                    "Isolated merge preview exceeded its 20-second subprocess budget.",
                ));
            }
        };
        let bytes = out
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| AppError::new("timeout", "Merge output did not finish"))??;
        let error = err
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| AppError::new("timeout", "Merge error output did not finish"))??;
        if (bytes.len() > limit && !partial) || error.len() > limit {
            return Err(AppError::new(
                "safety_budget",
                "Merge output exceeded its complete preview budget.",
            ));
        }
        if !status.success()
            && !(conflict_ok && status.code() == Some(1) || partial && bytes.len() > limit)
        {
            return Err(AppError::new(
                "merge_preview",
                String::from_utf8_lossy(&error).trim().to_string(),
            ));
        }
        Ok(bytes)
    }
    pub fn text(&self, args: &[&str]) -> Result<String> {
        String::from_utf8(self.run(
            &args.iter().map(OsString::from).collect::<Vec<_>>(),
            None,
            false,
        )?)
        .map_err(|_| AppError::new("encoding", "Invalid merge metadata"))
    }
    pub(crate) fn worktree_is_clean(&self, worktree: &Path, head: &str) -> Result<bool> {
        self.text(&["read-tree", head])?;
        let mut target = OsString::from("--work-tree=");
        target.push(worktree);
        // read-tree intentionally has no stat cache. Refresh only this disposable
        // index so clean files are hashed, rather than reported as uncertain M.
        let refresh = self.run(
            &[
                target.clone(),
                "update-index".into(),
                "--really-refresh".into(),
            ],
            None,
            true,
        )?;
        if !refresh.is_empty() {
            return Ok(false);
        }
        let output = self.run(
            &[
                target,
                "diff-files".into(),
                "--raw".into(),
                "-z".into(),
                "--no-renames".into(),
                "--no-ext-diff".into(),
                "--no-textconv".into(),
                "--".into(),
            ],
            None,
            false,
        )?;
        Ok(output.is_empty())
    }
    pub fn diff(&self, before: &str, after: &str) -> Result<String> {
        self.text(&[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            before,
            after,
            "--",
        ])
    }
    pub fn diff_preview(&self, before: &str, after: &str) -> Result<(String, bool)> {
        let limit = 512 * 1024;
        let mut bytes = self.run_limited(
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-renames",
                "--src-prefix=a/",
                "--dst-prefix=b/",
                before,
                after,
                "--",
            ]
            .iter()
            .map(OsString::from)
            .collect::<Vec<_>>(),
            None,
            false,
            limit,
            true,
        )?;
        let truncated = bytes.len() > limit;
        if truncated {
            bytes.truncate(limit);
            if let Some(end) = bytes.iter().rposition(|b| *b == b'\n') {
                bytes.truncate(end + 1);
            }
        }
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        let truncated = truncated || text.len() > limit;
        if text.len() > limit {
            let mut end = limit;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        Ok((text, truncated))
    }
    pub fn commit(&self, tree: &str, parent: Option<&str>) -> Result<String> {
        let mut args = vec!["commit-tree".into(), tree.into()];
        if let Some(parent) = parent {
            args.extend(["-p".into(), parent.into()]);
        }
        let bytes = self.run(&args, Some(b"GitNova isolated change preview\n"), false)?;
        String::from_utf8(bytes)
            .map(|s| s.trim().to_owned())
            .map_err(|_| AppError::new("git_parse", "Invalid scratch commit"))
    }
    pub fn blob_tree(&self, path: &Path, mode: &str, bytes: Option<&[u8]>) -> Result<String> {
        self.text(&["read-tree", "--empty"])?;
        if let Some(bytes) = bytes {
            let oid = String::from_utf8(self.run(
                &[
                    "hash-object".into(),
                    "-w".into(),
                    "--stdin".into(),
                    "--no-filters".into(),
                ],
                Some(bytes),
                false,
            )?)
            .map_err(|_| AppError::new("encoding", "Invalid blob OID"))?;
            let oid = oid.trim();
            crate::validate_oid(oid)?;
            self.run(
                &[
                    "update-index".into(),
                    "--add".into(),
                    "--cacheinfo".into(),
                    OsString::from(mode),
                    OsString::from(oid),
                    path.as_os_str().to_owned(),
                ],
                None,
                false,
            )?;
        }
        Ok(self.text(&["write-tree"])?.trim().to_owned())
    }
}
