//! Selected file snapshots in an isolated bare repository; never checks out user files.
use crate::{clone::validate_clone_source, working::run_fetch, GitCli, RepositoryReader};
use gitnova_domain::{AppError, GitCommitIdentity, NativePath, Result};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

const LIMIT: usize = 2 * 1024 * 1024;
#[derive(Debug, Clone)]
pub struct PromotionRequest {
    pub remote: String,
    pub source: String,
    pub target: String,
    pub branch: String,
    pub files: Vec<String>,
    pub allow_deletions: bool,
}
#[derive(Debug)]
pub struct SelectedFile {
    pub path: String,
    pub action: &'static str,
    entry: String,
}
pub struct Promotion {
    scratch: TempDir,
    original: PathBuf,
    pub request: PromotionRequest,
    pub url: String,
    pub source_oid: String,
    pub target_oid: String,
    pub files: Vec<SelectedFile>,
    pub identity: GitCommitIdentity,
    pub diff_stat: String,
}
fn error(message: impl Into<String>) -> AppError {
    AppError::new("promotion", message)
}
fn remote_tip(root: &Path, url: &str, branch: &str) -> Result<Option<String>> {
    let reference = format!("refs/heads/{branch}");
    let bytes = GitCli
        .execute_transport(
            root,
            &[
                OsStr::new("ls-remote"),
                OsStr::new("--refs"),
                OsStr::new("--"),
                OsStr::new(url),
                OsStr::new(&reference),
            ],
            4096,
            false,
            "file:http:https:ssh:git",
        )?
        .bytes;
    let text = String::from_utf8(bytes).map_err(|_| error("Invalid remote ref encoding."))?;
    let mut matches = text
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(_, name)| *name == reference);
    let first = matches.next().map(|(oid, _)| oid.to_owned());
    if matches.next().is_some() {
        return Err(error("Ambiguous remote branch."));
    }
    Ok(first)
}
fn entry(root: &Path, oid: &str, path: &str) -> Result<Option<String>> {
    let bytes = GitCli
        .read(
            root,
            &[
                OsStr::new("ls-tree"),
                OsStr::new("-z"),
                OsStr::new(oid),
                OsStr::new("--"),
                OsStr::new(path),
            ],
            16384,
        )?
        .bytes;
    if bytes.is_empty() {
        return Ok(None);
    }
    let record = bytes
        .strip_suffix(&[0])
        .ok_or_else(|| error("Invalid tree entry."))?;
    let text = std::str::from_utf8(record).map_err(|_| error("Use UTF-8 file paths."))?;
    let (metadata, actual) = text
        .split_once('\t')
        .ok_or_else(|| error("Invalid tree entry."))?;
    let fields = metadata.split_whitespace().collect::<Vec<_>>();
    if actual != path
        || fields.len() != 3
        || fields[1] != "blob"
        || !matches!(fields[0], "100644" | "100755")
    {
        return Err(error("Select exact regular files. Directories, symlinks and submodule pointers require separate review."));
    }
    Ok(Some(metadata.to_owned()))
}
impl GitCli {
    pub fn prepare_promotion(&self, root: &Path, request: PromotionRequest) -> Result<Promotion> {
        self.prepare_promotion_with_identity(root, request, None)
    }
    pub fn prepare_promotion_with_identity(
        &self,
        root: &Path,
        request: PromotionRequest,
        identity: Option<GitCommitIdentity>,
    ) -> Result<Promotion> {
        let discovery = self.discover(root)?;
        let original = discovery.root.unwrap_or(discovery.git_dir);
        for branch in [&request.source, &request.target, &request.branch] {
            if branch.is_empty()
                || branch.len() > 256
                || branch.starts_with('-')
                || branch.chars().any(char::is_control)
            {
                return Err(error("Enter explicit source, target and new branch names."));
            }
            self.text(
                &original,
                &["check-ref-format", &format!("refs/heads/{branch}")],
            )?;
        }
        if request.branch == request.source || request.branch == request.target {
            return Err(error(
                "Use a new review branch distinct from source and target.",
            ));
        }
        if request.files.is_empty() || request.files.len() > 200 {
            return Err(error("Select between 1 and 200 exact file paths after --."));
        }
        let mut unique = BTreeSet::new();
        for file in &request.files {
            crate::working::relative(&NativePath::new(Path::new(file)))?;
            if file.contains('\\')
                || file.chars().any(char::is_control)
                || file == ".gitmodules"
                || file.len() > 4096
                || !unique.insert(file)
            {
                return Err(error("Use unique, portable exact file paths; submodule layout changes require separate review."));
            }
        }
        let url = self.push_destination(&original, &request.remote)?;
        let url = validate_clone_source(&url)?;
        let fetch_url = self.text(&original, &["remote", "get-url", "--", &request.remote])?;
        if validate_clone_source(fetch_url.trim_end())? != url {
            return Err(error(
                "Source and push destinations differ. Choose one explicit remote repository.",
            ));
        }
        let identity = match identity {
            Some(identity) => identity,
            None => self.effective_commit_identity(&original)?,
        };
        crate::commit_identity::validate_identity(&identity)?;
        let scratch = tempfile::Builder::new()
            .prefix("gitnova-promote-")
            .tempdir()?;
        let object_format = format!("--object-format={}", discovery.object_format);
        self.text(
            scratch.path(),
            &["init", "--bare", "--template=", &object_format, "."],
        )?;
        // Disable inherited/global hooks for this isolated operation, including preview fetch.
        let hooks = scratch.path().join("disabled-hooks");
        std::fs::create_dir(&hooks)?;
        self.text(
            scratch.path(),
            &[
                "config",
                "core.hooksPath",
                hooks
                    .to_str()
                    .ok_or_else(|| error("Unsupported scratch path."))?,
            ],
        )?;
        self.text(scratch.path(), &["remote", "add", "--", "origin", &url])?;
        let source_ref = format!("refs/heads/{}:refs/nova/source", request.source);
        let target_ref = format!("refs/heads/{}:refs/nova/target", request.target);
        run_fetch(
            scratch.path(),
            &[
                "fetch".into(),
                "--no-tags".into(),
                "--no-recurse-submodules".into(),
                "--".into(),
                url.clone().into(),
                source_ref.into(),
                target_ref.into(),
            ],
        )?;
        if remote_tip(scratch.path(), &url, &request.branch)?.is_some() {
            return Err(error(
                "The review branch already exists remotely. Choose a new name.",
            ));
        }
        let source_oid = self
            .text(
                scratch.path(),
                &["rev-parse", "--verify", "refs/nova/source^{commit}"],
            )?
            .trim()
            .to_owned();
        let target_oid = self
            .text(
                scratch.path(),
                &["rev-parse", "--verify", "refs/nova/target^{commit}"],
            )?
            .trim()
            .to_owned();
        let mut files = Vec::new();
        for path in &request.files {
            let source = entry(scratch.path(), &source_oid, path)?;
            let target = entry(scratch.path(), &target_oid, path)?;
            if source == target {
                continue;
            }
            let (action, entry) = match (source, target) {
                (Some(value), old) => (if old.is_some() { "replace" } else { "add" }, format!("{value}\t{path}\0")),
                (None, Some(_)) if request.allow_deletions => ("delete", format!("0 {}\t{path}\0", "0".repeat(source_oid.len()))),
                _ => return Err(error(format!("{path} is missing from source. Use --allow-deletions only for an intended deletion."))),
            };
            files.push(SelectedFile {
                path: path.clone(),
                action,
                entry,
            });
        }
        if files.is_empty() {
            return Err(error(
                "Selected files already match the destination; no request is needed.",
            ));
        }
        let mut args = vec![
            "diff",
            "--raw",
            "--no-ext-diff",
            "--no-textconv",
            "--stat",
            &target_oid,
            &source_oid,
            "--",
        ];
        args.extend(files.iter().map(|file| file.path.as_str()));
        // --raw keeps the read runner on object metadata in a bare repository;
        // remove its object records and show only the selected-file statistics.
        let diff_stat = self
            .text(scratch.path(), &args)?
            .lines()
            .filter(|line| !line.starts_with(':'))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Promotion {
            scratch,
            original,
            request,
            url,
            source_oid,
            target_oid,
            files,
            identity,
            diff_stat,
        })
    }
}
impl Promotion {
    /// Retains the isolated repository before any publication attempt, even on ambiguous failures.
    pub fn publish(
        self,
        message: &str,
        recovery: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<String> {
        if message.trim().is_empty() || message.len() > 65536 || message.contains('\0') {
            return Err(error("Enter a commit message up to 64 KiB."));
        }
        let current =
            validate_clone_source(&GitCli.push_destination(&self.original, &self.request.remote)?)?;
        if current != self.url
            || remote_tip(self.scratch.path(), &self.url, &self.request.source)?.as_deref()
                != Some(&self.source_oid)
            || remote_tip(self.scratch.path(), &self.url, &self.request.target)?.as_deref()
                != Some(&self.target_oid)
        {
            return Err(error(
                "Remote configuration or branch tips changed. Run the preview again.",
            ));
        }
        let root = self.scratch.keep();
        recovery(&root)?;
        GitCli.text(&root, &["read-tree", &self.target_oid])?;
        let entries = self
            .files
            .iter()
            .map(|f| f.entry.as_str())
            .collect::<String>();
        GitCli.read_input(
            &root,
            &[
                OsStr::new("update-index"),
                OsStr::new("-z"),
                OsStr::new("--index-info"),
            ],
            entries.as_bytes(),
            LIMIT,
        )?;
        let tree = GitCli.text(&root, &["write-tree"])?.trim().to_owned();
        let original_tree = GitCli.text(
            &root,
            &["rev-parse", &format!("{}^{{tree}}", self.target_oid)],
        )?;
        if tree == original_tree.trim() {
            return Err(error("No tree changes remain."));
        }
        let oid = crate::commit_identity::with_commit_identity(&root, Some(self.identity), || {
            GitCli.read_input(
                &root,
                &[
                    OsStr::new("commit-tree"),
                    OsStr::new(&tree),
                    OsStr::new("-p"),
                    OsStr::new(&self.target_oid),
                ],
                message.as_bytes(),
                4096,
            )
        })?;
        let oid = String::from_utf8(oid.bytes)
            .map_err(|_| error("Invalid commit encoding."))?
            .trim()
            .to_owned();
        let reference = format!("refs/heads/{}", self.request.branch);
        GitCli.text(
            &root,
            &["update-ref", &reference, &oid, &"0".repeat(oid.len())],
        )?;
        let lease = format!("--force-with-lease={reference}:");
        let refspec = format!("{oid}:{reference}");
        run_fetch(
            &root,
            &[
                "push".into(),
                "--porcelain".into(),
                lease.into(),
                "--".into(),
                self.url.clone().into(),
                refspec.into(),
            ],
        )?;
        if remote_tip(&root, &self.url, &self.request.branch)?.as_deref() != Some(&oid) {
            return Err(error("Publication could not be verified. Inspect the remote and retained repository before retrying."));
        }
        Ok(oid)
    }
}
