mod credentials;
mod executable;
pub use credentials::install_transport_credential_helper;
mod metrics;
mod wait;
pub use metrics::{measure_git, GitMetrics};
mod clone;
mod init;
mod progress;
mod remote_config;
mod tracking;
pub use progress::with_progress;
pub mod graph_actions;
pub mod merge;
pub mod push;
pub mod ref_operations;
mod safe_read;
pub mod stash;
pub mod submodules;
mod tags;
pub use submodules::SubmoduleWriter;
pub mod remotes;
pub use remotes::{RemoteReader, RemoteWriter};
pub mod branches;
pub use branches::{BranchReader, BranchWriter};
pub mod working;
use gitnova_domain::*;
use std::{
    ffi::{OsStr, OsString},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};
pub use working::{CommitWriter, IndexWriter, WorkingTreeReader};

pub trait RepositoryReader {
    fn discover(&self, path: &Path) -> Result<Discovery>;
}
pub trait HistoryReader {
    fn history(&self, path: &Path, offset: usize, count: usize) -> Result<Vec<Commit>>;
}
pub trait RefReader {
    fn refs(&self, path: &Path) -> Result<Vec<GitRef>>;
}
pub trait StatusReader {
    fn status(&self, path: &Path) -> Result<RepoStatus>;
}
pub trait DiffReader {
    fn detail(&self, path: &Path, oid: &str) -> Result<CommitDetail>;
}
#[derive(Debug)]
pub struct Discovery {
    pub common_dir: PathBuf,
    pub git_dir: PathBuf,
    pub root: Option<PathBuf>,
    pub object_format: String,
}
#[derive(Clone, Default)]
pub struct GitCli;
pub struct Output {
    pub bytes: Vec<u8>,
    pub truncated: bool,
}
const DEFAULT_LIMIT: usize = 8 * 1024 * 1024;

impl GitCli {
    pub fn read(&self, path: &Path, args: &[&OsStr], limit: usize) -> Result<Output> {
        self.execute(path, args, limit, false)
    }
    pub(crate) fn execute(
        &self,
        path: &Path,
        args: &[&OsStr],
        limit: usize,
        allow_truncation: bool,
    ) -> Result<Output> {
        self.execute_transport(path, args, limit, allow_truncation, "")
    }
    pub(crate) fn execute_transport(
        &self,
        path: &Path,
        args: &[&OsStr],
        limit: usize,
        allow_truncation: bool,
        protocols: &str,
    ) -> Result<Output> {
        self.execute_transport_input(path, args, limit, allow_truncation, protocols, None)
    }
    pub fn read_input(
        &self,
        path: &Path,
        args: &[&OsStr],
        input: &[u8],
        limit: usize,
    ) -> Result<Output> {
        if input.len() > 2 * 1024 * 1024 {
            return Err(AppError::new(
                "input_limit",
                "Git batch input exceeds 2 MiB.",
            ));
        }
        self.execute_transport_input(path, args, limit, false, "", Some(input))
    }
    fn execute_transport_input(
        &self,
        path: &Path,
        args: &[&OsStr],
        limit: usize,
        allow_truncation: bool,
        protocols: &str,
        input: Option<&[u8]>,
    ) -> Result<Output> {
        // Even `git status`/working-tree `diff` may invoke clean/process filters.
        // Discover driver keys through config-only reads, then neutralize those
        // commands for every inspection (including nested Git invocations).
        let mut overrides = Vec::<OsString>::new();
        let command = args.first().and_then(|arg| arg.to_str()).unwrap_or("");
        let metadata_only = matches!(
            command,
            "config"
                | "ls-files"
                | "rev-parse"
                | "ls-tree"
                | "symbolic-ref"
                | "check-ref-format"
                | "show-ref"
                | "for-each-ref"
                | "rev-list"
        ) || command == "log"
            && args.contains(&OsStr::new("--no-show-signature"))
            && args.contains(&OsStr::new("--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s"))
            && args.iter().skip(1).all(|arg| {
                let Some(arg) = arg.to_str() else {
                    return false;
                };
                matches!(
                    arg,
                    "--date-order"
                        | "--no-show-signature"
                        | "--no-decorate"
                        | "-z"
                        | "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s"
                        | "--single-worktree"
                        | "--exclude=refs/stash"
                        | "--exclude=refs/gitnova/recovery/*"
                        | "--all"
                        | "HEAD"
                        | "--"
                ) || ["--skip=", "--max-count="].iter().any(|prefix| {
                    arg.strip_prefix(prefix)
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                })
            })
            || command == "cat-file"
                && !args
                    .iter()
                    .any(|a| a == &OsStr::new("--filters") || a == &OsStr::new("--textconv"));
        if !metadata_only {
            let mut drivers = self.filter_drivers(path)?;
            if command == "status" || (command == "diff" && !args.contains(&OsStr::new("--raw"))) {
                self.nested_filter_drivers(path, &mut drivers)?;
            }
            for driver in drivers {
                for assignment in [
                    format!("{driver}.clean="),
                    format!("{driver}.process="),
                    format!("{driver}.required=false"),
                ] {
                    overrides.push("-c".into());
                    overrides.push(assignment.into());
                }
            }
        }
        let mut cmd = Command::new(executable::git_executable());
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                cmd.env_remove(key);
            }
        }
        cmd.env_remove("GITNOVA_CREDENTIAL_TARGETS");
        if !protocols.is_empty() {
            let arguments = args.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>();
            credentials::configure(&mut cmd, path, &arguments);
        }
        cmd.env_remove("GITNOVA_CREDENTIAL_FALLBACK");
        if command == "credential" {
            cmd.env("GITNOVA_CREDENTIAL_FALLBACK", "1");
        }
        cmd.args(overrides)
            .args([
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
                "-C",
            ])
            .arg(path)
            .args(args)
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_ALLOW_PROTOCOL", protocols)
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
        let _timer = metrics::CommandTimer::new(command);
        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::new("git_unavailable", format!("Cannot launch Git: {e}")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::new("git", "Missing stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::new("git", "Missing stderr"))?;
        let (wake_tx, wake_rx) = std::sync::mpsc::channel();
        let out_wake = wake_tx.clone();
        let (out_tx, out_rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let mut bytes = vec![];
            let result = stdout
                .take((limit + 1) as u64)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = out_tx.send(result);
            let _ = out_wake.send(());
        });
        let (err_tx, err_rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let mut bytes = vec![];
            let result = stderr.take(65536).read_to_end(&mut bytes).map(|_| bytes);
            let _ = err_tx.send(result);
            let _ = wake_tx.send(());
        });
        let input_rx = if let Some(input) = input {
            let mut pipe = child
                .stdin
                .take()
                .ok_or_else(|| AppError::new("git", "Missing batch input"))?;
            let input = input.to_vec();
            let (tx, rx) = std::sync::mpsc::channel();
            thread::spawn(move || {
                use std::io::Write;
                let _ = tx.send(pipe.write_all(&input));
            });
            Some(rx)
        } else {
            None
        };
        let mut timed_out = false;
        let status = match wait::wait_for_exit(
            &mut child,
            Duration::from_secs(20),
            &wake_rx,
            Duration::from_millis(10),
        )? {
            Some(status) => status,
            None => {
                timed_out = true;
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                child.wait()?
            }
        };
        let mut bytes = out_rx.recv_timeout(Duration::from_secs(2)).map_err(|_| {
            AppError::new(
                "timeout",
                "Git output did not finish within its bounded read budget",
            )
        })??;
        let error = err_rx.recv_timeout(Duration::from_secs(2)).map_err(|_| {
            AppError::new(
                "timeout",
                "Git error output did not finish within its bounded read budget",
            )
        })??;
        if timed_out {
            return Err(AppError::new(
                "timeout",
                "Git read exceeded its 20-second budget",
            ));
        }
        if let Some(rx) = input_rx {
            rx.recv_timeout(Duration::from_secs(2))
                .map_err(|_| AppError::new("timeout", "Git batch input did not finish."))??;
        }
        let truncated = bytes.len() > limit;
        bytes.truncate(limit);
        if truncated && !allow_truncation {
            return Err(AppError::new(
                "output_limit",
                "Git output exceeded its bounded read budget",
            ));
        }
        if !status.success() && !(truncated && allow_truncation) {
            let code = format!("git_exit_{}", status.code().unwrap_or(-1));
            let message = String::from_utf8_lossy(&error).trim().to_string();
            return Err(if protocols.is_empty() {
                AppError::new(&code, message)
            } else {
                credentials::transport_error(&code, message)
            });
        }
        Ok(Output { bytes, truncated })
    }
    pub fn text(&self, path: &Path, args: &[&str]) -> Result<String> {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        String::from_utf8(self.read(path, &args, DEFAULT_LIMIT)?.bytes)
            .map_err(|_| AppError::new("encoding", "Git returned non-UTF-8 metadata"))
    }
    pub fn optional_text(&self, path: &Path, args: &[&str]) -> Result<Option<String>> {
        match self.text(path, args) {
            Ok(s) => Ok(Some(s.trim_end_matches('\n').to_string())),
            Err(e) if e.code == "git_exit_1" => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn path(&self, path: &Path, arg: &str) -> Result<PathBuf> {
        let mut bytes = self
            .read(
                path,
                &[OsStr::new("rev-parse"), OsStr::new(arg)],
                DEFAULT_LIMIT,
            )?
            .bytes;
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        let p = path_from_bytes(bytes)?;
        std::fs::canonicalize(if p.is_absolute() { p } else { path.join(p) }).map_err(Into::into)
    }
    pub fn fingerprint(&self, path: &Path) -> Result<String> {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.text(path, &["show-ref", "--head"])
            .or_else(|e| {
                if e.code == "git_exit_1" {
                    Ok(String::new())
                } else {
                    Err(e)
                }
            })?
            .hash(&mut hash);
        // show-ref --head already includes the resolved HEAD, including detached
        // HEAD. A second rev-parse spawned another Git process for identical data.
        self.optional_text(path, &["symbolic-ref", "--quiet", "HEAD"])?
            .hash(&mut hash);
        let shallow_path = self.path(path, "--git-common-dir")?.join("shallow");
        match std::fs::read(shallow_path) {
            Ok(bytes) => bytes.hash(&mut hash),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(format!("{:016x}", hash.finish()))
    }
}
pub fn path_from_bytes(bytes: Vec<u8>) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        Ok(PathBuf::from(OsString::from_vec(bytes)))
    }
    #[cfg(windows)]
    {
        String::from_utf8(bytes).map(PathBuf::from).map_err(|_| {
            AppError::new(
                "encoding",
                "Git returned a path not representable as UTF-8 on Windows",
            )
        })
    }
}
pub fn validate_oid(oid: &str) -> Result<()> {
    if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
        Err(AppError::new(
            "oid",
            "Expected a full SHA-1 or SHA-256 object ID",
        ))
    } else {
        Ok(())
    }
}
fn parse_commit(fields: &[&[u8]]) -> Result<Commit> {
    let s = |i: usize| String::from_utf8_lossy(fields[i]).into_owned();
    let oid = s(0);
    validate_oid(&oid)?;
    Ok(Commit {
        oid,
        parents: s(1).split_whitespace().map(str::to_owned).collect(),
        author: s(2),
        email: s(3),
        timestamp: s(4)
            .parse()
            .map_err(|_| AppError::new("git_parse", "Invalid commit timestamp"))?,
        subject: s(5),
    })
}
impl RepositoryReader for GitCli {
    fn discover(&self, path: &Path) -> Result<Discovery> {
        let path = std::fs::canonicalize(path)?;
        // One metadata query instead of four. Newline-containing native paths
        // use the original single-field parser below, preserving raw identity.
        let raw = self
            .read(
                &path,
                &[
                    OsStr::new("rev-parse"),
                    OsStr::new("--path-format=absolute"),
                    OsStr::new("--is-bare-repository"),
                    OsStr::new("--absolute-git-dir"),
                    OsStr::new("--git-common-dir"),
                    OsStr::new("--show-object-format"),
                ],
                DEFAULT_LIMIT,
            )?
            .bytes;
        let fields: Vec<_> = raw
            .strip_suffix(b"\n")
            .unwrap_or(&raw)
            .split(|b| *b == b'\n')
            .collect();
        if fields.len() == 4
            && matches!(fields[0], b"true" | b"false")
            && matches!(fields[3], b"sha1" | b"sha256")
        {
            let git_dir = path_from_bytes(fields[1].to_vec())?;
            let common_dir = path_from_bytes(fields[2].to_vec())?;
            if git_dir.is_absolute() && common_dir.is_absolute() {
                return Ok(Discovery {
                    git_dir: std::fs::canonicalize(git_dir)?,
                    common_dir: std::fs::canonicalize(common_dir)?,
                    root: if fields[0] == b"true" {
                        None
                    } else {
                        Some(self.path(&path, "--show-toplevel")?)
                    },
                    object_format: String::from_utf8_lossy(fields[3]).into_owned(),
                });
            }
        }
        let common_dir = self.path(&path, "--git-common-dir")?;
        let git_dir = self.path(&path, "--absolute-git-dir")?;
        let bare = self
            .text(&path, &["rev-parse", "--is-bare-repository"])?
            .trim()
            == "true";
        let root = if bare {
            None
        } else {
            Some(self.path(&path, "--show-toplevel")?)
        };
        let object_format = self
            .text(&path, &["rev-parse", "--show-object-format"])?
            .trim()
            .to_string();
        Ok(Discovery {
            common_dir,
            git_dir,
            root,
            object_format,
        })
    }
}
impl RefReader for GitCli {
    fn refs(&self, path: &Path) -> Result<Vec<GitRef>> {
        let raw = self.text(
            path,
            &[
                "for-each-ref",
                "--format=%(objectname)%00%(refname)%00%(*objectname)",
            ],
        )?;
        raw.lines()
            .filter(|line| {
                !line
                    .split('\0')
                    .nth(1)
                    .is_some_and(|name| name.starts_with("refs/gitnova/recovery/"))
            })
            .map(|line| {
                let fields: Vec<_> = line.split('\0').collect();
                if fields.len() != 3 {
                    return Err(AppError::new("git_parse", "Invalid ref record"));
                }
                let oid = if fields[2].is_empty() {
                    fields[0]
                } else {
                    fields[2]
                };
                Ok(GitRef {
                    oid: oid.into(),
                    name: fields[1].into(),
                })
            })
            .collect()
    }
}
impl GitCli {
    fn status_metadata(&self, path: &Path) -> Result<(bool, Option<String>, Option<String>)> {
        // Ordinary committed checkouts need one metadata process, not three.
        // Unborn/unusual symbolic HEAD shapes retain the original independent
        // probes rather than guessing from an incomplete rev-parse response.
        let batched = match self.text(
            path,
            &[
                "rev-parse",
                "--is-bare-repository",
                "HEAD",
                "--symbolic-full-name",
                "HEAD",
            ],
        ) {
            Ok(value) => Some(value),
            Err(error) if matches!(error.code.as_str(), "git_exit_128" | "git_exit_1") => None,
            Err(error) => return Err(error),
        };
        if let Some(value) = batched {
            let fields = value.trim_end_matches('\n').split('\n').collect::<Vec<_>>();
            if fields.len() == 3
                && matches!(fields[0], "true" | "false")
                && validate_oid(fields[1]).is_ok()
            {
                if let Some(branch) = fields[2].strip_prefix("refs/heads/") {
                    return Ok((
                        fields[0] == "true",
                        Some(fields[1].into()),
                        Some(branch.into()),
                    ));
                }
                if fields[2] == "HEAD" {
                    return Ok((fields[0] == "true", Some(fields[1].into()), None));
                }
            }
        }
        Ok((
            self.text(path, &["rev-parse", "--is-bare-repository"])?
                .trim()
                == "true",
            self.optional_text(path, &["rev-parse", "--verify", "--quiet", "HEAD"])?,
            self.optional_text(path, &["symbolic-ref", "--quiet", "--short", "HEAD"])?,
        ))
    }
    /// One porcelain observation shared by display counts and the changed-file list.
    pub(crate) fn status_observation(
        &self,
        path: &Path,
        limit: usize,
    ) -> Result<(RepoStatus, Vec<u8>)> {
        let (bare, head, branch) = self.status_metadata(path)?;
        let mut changed_entries = 0;
        let mut raw = Vec::new();
        if !bare {
            let bytes = self
                .read(
                    path,
                    &[
                        OsStr::new("status"),
                        OsStr::new("--porcelain=v1"),
                        OsStr::new("-z"),
                        OsStr::new("--untracked-files=all"),
                        OsStr::new("--no-renames"),
                        OsStr::new("--ignore-submodules=none"),
                    ],
                    limit,
                )?
                .bytes;
            let mut records = bytes.split(|b| *b == 0).filter(|r| !r.is_empty());
            while let Some(record) = records.next() {
                changed_entries += 1;
                if record.first() == Some(&b'R')
                    || record.first() == Some(&b'C')
                    || record.get(1) == Some(&b'R')
                    || record.get(1) == Some(&b'C')
                {
                    records.next();
                }
            }
            raw = bytes;
        }
        Ok((
            RepoStatus {
                branch,
                head,
                dirty: changed_entries > 0,
                changed_entries,
                bare,
            },
            raw,
        ))
    }
}
impl StatusReader for GitCli {
    fn status(&self, path: &Path) -> Result<RepoStatus> {
        self.status_observation(path, DEFAULT_LIMIT)
            .map(|(status, _)| status)
    }
}
impl HistoryReader for GitCli {
    fn history(&self, path: &Path, offset: usize, count: usize) -> Result<Vec<Commit>> {
        if count > 101 || offset > 1_000_000 {
            return Err(AppError::new(
                "pagination",
                "History request exceeds page bounds",
            ));
        }
        self.history_range(path, offset, count)
    }
}
impl GitCli {
    pub fn history_oids(&self, path: &Path, count: usize) -> Result<Vec<String>> {
        if count > 20101 {
            return Err(AppError::new(
                "pagination",
                "History traversal exceeds its bounded budget.",
            ));
        }
        let head = self.optional_text(path, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
        if head.is_none() && self.refs(path)?.is_empty() {
            return Ok(vec![]);
        }
        let mut args: Vec<OsString> = vec![
            "rev-list".into(),
            "--date-order".into(),
            format!("--max-count={count}").into(),
            "--single-worktree".into(),
            "--exclude=refs/stash".into(),
            "--exclude=refs/gitnova/recovery/*".into(),
            "--all".into(),
        ];
        if head.is_some() {
            args.push("HEAD".into());
        }
        args.push("--".into());
        let bytes = self
            .read(
                path,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                2 * 1024 * 1024,
            )?
            .bytes;
        let text = String::from_utf8(bytes)
            .map_err(|_| AppError::new("git_parse", "Invalid history OIDs."))?;
        let oids = text.lines().map(str::to_owned).collect::<Vec<_>>();
        for oid in &oids {
            validate_oid(oid)?;
        }
        Ok(oids)
    }
    pub fn history_range(&self, path: &Path, offset: usize, count: usize) -> Result<Vec<Commit>> {
        if count > 20101 || offset > 1_000_000 {
            return Err(AppError::new(
                "pagination",
                "History range exceeds its bounded budget.",
            ));
        }
        let head = self.optional_text(path, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
        if head.is_none() && self.refs(path)?.is_empty() {
            return Ok(vec![]);
        }
        let mut args: Vec<OsString> = [
            "log",
            "--date-order",
            "--no-show-signature",
            "--no-decorate",
            "-z",
            "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s",
            "--single-worktree",
            "--exclude=refs/stash",
            "--exclude=refs/gitnova/recovery/*",
            "--all",
        ]
        .iter()
        .map(OsString::from)
        .collect();
        args.push(format!("--skip={offset}").into());
        args.push(format!("--max-count={count}").into());
        if head.is_some() {
            args.push("HEAD".into());
        }
        args.push("--".into());
        let bytes = self
            .read(
                path,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                DEFAULT_LIMIT,
            )?
            .bytes;
        let mut fields: Vec<&[u8]> = bytes.split(|b| *b == 0).collect();
        if fields.last() == Some(&&b""[..]) {
            fields.pop();
        }
        if !fields.len().is_multiple_of(6) {
            return Err(AppError::new(
                "git_parse",
                "Unexpected commit record format",
            ));
        }
        fields
            .as_chunks::<6>()
            .0
            .iter()
            .map(|fields| parse_commit(fields))
            .collect()
    }
}
impl GitCli {
    pub fn historical_patch(
        &self,
        root: &Path,
        oid: &str,
        file: &NativePath,
    ) -> Result<HistoricalPatch> {
        validate_oid(oid)?;
        let path = file.to_path()?;
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(AppError::new(
                "path",
                "Choose a repository-relative file path.",
            ));
        }
        let parents = self.text(
            root,
            &[
                "show",
                "-s",
                "--no-show-signature",
                "--format=%P",
                oid,
                "--",
            ],
        )?;
        let mut args: Vec<&OsStr> = vec![
            "diff-tree".as_ref(),
            "--root".as_ref(),
            "--no-commit-id".as_ref(),
            "--no-ext-diff".as_ref(),
            "--no-textconv".as_ref(),
            "--no-renames".as_ref(),
            "--no-color".as_ref(),
            "--submodule=short".as_ref(),
            "--src-prefix=a/".as_ref(),
            "--dst-prefix=b/".as_ref(),
            "--no-abbrev".as_ref(),
            "--ignore-submodules=none".as_ref(),
            "-r".as_ref(),
            "-p".as_ref(),
        ];
        if let Some(parent) = parents.split_whitespace().next() {
            validate_oid(parent)?;
            args.push(parent.as_ref());
        }
        args.extend([oid.as_ref(), "--".as_ref(), path.as_os_str()]);
        let output = self.execute(root, &args, 512 * 1024, true)?;
        Ok(HistoricalPatch {
            diff: String::from_utf8_lossy(&output.bytes).into_owned(),
            truncated: output.truncated,
        })
    }
}

impl DiffReader for GitCli {
    fn detail(&self, path: &Path, oid: &str) -> Result<CommitDetail> {
        validate_oid(oid)?;
        let raw = self.text(
            path,
            &[
                "show",
                "-s",
                "--no-show-signature",
                "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s%x00%B",
                oid,
                "--",
            ],
        )?;
        let fields: Vec<&[u8]> = raw.as_bytes().splitn(7, |b| *b == 0).collect();
        if fields.len() != 7 {
            return Err(AppError::new("git_parse", "Unexpected commit details"));
        }
        let commit = parse_commit(&fields[..6])?;
        let mut args = vec![
            "diff-tree",
            "--root",
            "--no-commit-id",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--no-color",
            "--submodule=short",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "--no-abbrev",
            "--ignore-submodules=none",
            "-r",
            "-p",
        ];
        let comparison = if let Some(parent) = commit.parents.first() {
            args.push(parent);
            "First parent"
        } else {
            "Empty tree (root commit)"
        };
        args.push(oid);
        args.push("--");
        let output = self.execute(
            path,
            &args.iter().map(OsStr::new).collect::<Vec<_>>(),
            512 * 1024,
            true,
        )?;
        let mut stat_args = args.clone();
        let patch_flag = stat_args.iter().position(|arg| *arg == "-p").unwrap();
        stat_args[patch_flag] = "--numstat";
        stat_args.insert(patch_flag + 1, "-z");
        let stats = self
            .read(
                path,
                &stat_args.iter().map(OsStr::new).collect::<Vec<_>>(),
                4 * 1024 * 1024,
            )?
            .bytes;
        let mut files = vec![];
        for record in stats.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            if files.len() >= 20_000 {
                return Err(AppError::new(
                    "diff_limit",
                    "Commit file list exceeds 20,000 paths.",
                ));
            }
            let parts = record.splitn(3, |b| *b == b'\t').collect::<Vec<_>>();
            if parts.len() != 3 {
                return Err(AppError::new(
                    "git_parse",
                    "Invalid commit file statistics.",
                ));
            }
            let count = |bytes: &[u8]| -> Result<Option<u64>> {
                if bytes == b"-" {
                    return Ok(None);
                }
                Ok(Some(
                    std::str::from_utf8(bytes)
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| AppError::new("git_parse", "Invalid line count."))?,
                ))
            };
            files.push(CommitFile {
                path: NativePath::new(&path_from_bytes(parts[2].to_vec())?),
                additions: count(parts[0])?,
                deletions: count(parts[1])?,
            });
        }
        Ok(CommitDetail {
            files,
            commit,
            message: String::from_utf8_lossy(fields[6])
                .trim_end_matches('\n')
                .into(),
            diff: String::from_utf8_lossy(&output.bytes).into(),
            truncated: output.truncated,
            comparison: comparison.into(),
        })
    }
}

pub mod worktree;

pub mod worktree_remove;

mod remote_delete;
