//! Owned, previewed merges; active worktrees are never used for merge prediction.
mod preserved;
mod rebase;
mod recovery;
pub(crate) mod scratch;
mod source_cleanup;
mod unstage;
use crate::{
    working::run_offline_write, GitCli, RepositoryReader, StatusReader, WorkingTreeReader,
};
use gitnova_domain::*;
use scratch::{attributes, read_file, Scratch};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};
const MARKER: &str = "gitnova-merge.json";
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Parse exact NUL-delimited raw records; paths can contain tabs/newlines.
fn gitlink_changes(raw: &[u8]) -> Result<Vec<MergeSubmoduleChange>> {
    let mut rows = raw.split(|b| *b == 0).filter(|r| !r.is_empty());
    let mut changes = vec![];
    while let Some(header) = rows.next() {
        let path = rows
            .next()
            .ok_or_else(|| AppError::new("merge_preview", "Invalid raw merge path record."))?;
        let fields = header.split(|b| *b == b' ').collect::<Vec<_>>();
        if fields.len() != 5 || !fields[0].starts_with(b":") {
            return Err(AppError::new(
                "merge_preview",
                "Invalid raw merge path record.",
            ));
        }
        if fields[0] != b":160000" && fields[1] != b"160000" {
            continue;
        }
        let oid = |mode: &[u8], bytes: &[u8]| -> Result<Option<String>> {
            if mode != b"160000" {
                return Ok(None);
            }
            let value = String::from_utf8(bytes.to_vec())
                .map_err(|_| AppError::new("merge_preview", "Invalid gitlink revision."))?;
            crate::validate_oid(&value)?;
            Ok(Some(value))
        };
        let current = oid(&fields[0][1..], fields[2])?;
        let incoming = oid(fields[1], fields[3])?;
        changes.push(MergeSubmoduleChange {
            path: NativePath::new(&crate::path_from_bytes(path.to_vec())?),
            current_oid: current,
            incoming_oid: incoming,
            result_oid: None,
            conflicted: false,
        });
        if changes.len() > 2000 {
            return Err(AppError::new(
                "safety_budget",
                "Merge preview exceeds 2,000 paths.",
            ));
        }
    }
    Ok(changes)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Marker {
    #[serde(default)]
    rebase: Option<rebase::Journal>,
    version: u32,
    request: MergeRequest,
    branch: String,
    original_oid: String,
    recovery_ref: String,
    session_dir: NativePath,
    #[serde(default)]
    prepared: bool,
    #[serde(default)]
    native_metadata: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    preserved_submodules: Vec<PreservedSubmodule>,
    #[serde(default)]
    suggested_message: Option<String>,
}
fn marker_bytes(marker: &Marker) -> Result<Vec<u8>> {
    let bytes =
        serde_json::to_vec(marker).map_err(|e| AppError::new("merge_metadata", e.to_string()))?;
    if bytes.len() > 65536 {
        return Err(AppError::new(
            "safety_budget",
            "Owned operation metadata exceeds its complete 64 KiB budget.",
        ));
    }
    Ok(bytes)
}
fn safe_path(root: &Path, file: &NativePath) -> Result<PathBuf> {
    let relative = file.to_path()?;
    if relative.as_os_str().is_empty()
        || relative.components().any(|part| {
            !matches!(part, Component::Normal(_))
                || part
                    .as_os_str()
                    .eq_ignore_ascii_case(std::ffi::OsStr::new(".git"))
        })
    {
        return Err(AppError::new(
            "merge_path",
            "Choose an exact repository-relative path.",
        ));
    }
    let mut parent = root.to_path_buf();
    for part in relative.parent().unwrap_or(Path::new("")).components() {
        parent.push(part);
        if fs::symlink_metadata(&parent).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(AppError::new(
                "merge_path",
                "Inspect symlinked parent directories before changing this merge path.",
            ));
        }
    }
    Ok(root.join(relative))
}
fn metadata(path: &Path, limit: usize) -> Result<Vec<u8>> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(AppError::new(
            "merge_metadata",
            "Symlinked merge metadata requires separate inspection.",
        ));
    }
    read_file(path, limit)
}
fn load_marker(git: &GitCli, root: &Path) -> Result<Option<Marker>> {
    let d = git.discover(root)?;
    let bytes = metadata(&d.git_dir.join(MARKER), 65536)?;
    if bytes.is_empty() {
        return Ok(None);
    }
    let marker: Marker = serde_json::from_slice(&bytes).map_err(|_| {
        AppError::new(
            "merge_metadata",
            "Merge ownership metadata is incomplete; inspect the repository with Git.",
        )
    })?;
    crate::validate_oid(&marker.original_oid)?;
    crate::validate_oid(&marker.request.source_oid)?;
    git.valid_name(root, &marker.branch, "heads")?;
    let id = uuid::Uuid::parse_str(&marker.request.session_id)
        .map_err(|_| AppError::new("merge_metadata", "Invalid merge session identity."))?;
    let expected = d.git_dir.join(format!("gitnova-merge-{}", id.simple()));
    let actual = marker.session_dir.to_path()?;
    if !["merge", "cherryPick", "revert", "rebase"].contains(&marker.request.operation.as_str())
        || marker.version != 1
        || actual != expected
        || fs::symlink_metadata(&actual).is_err()
        || fs::symlink_metadata(&actual)?.file_type().is_symlink()
        || !fs::metadata(&actual)?.is_dir()
        || marker.recovery_ref != format!("refs/gitnova/recovery/merge-{}", id.simple())
    {
        return Err(AppError::new(
            "merge_metadata",
            "Merge recovery location does not identify this worktree's session.",
        ));
    }
    Ok(Some(marker))
}
fn hidden_entries(git: &GitCli, root: &Path) -> Result<bool> {
    Ok(git
        .read(
            root,
            &["ls-files".as_ref(), "-v".as_ref(), "-z".as_ref()],
            8 * 1024 * 1024,
        )?
        .bytes
        .split(|b| *b == 0)
        .filter(|v| !v.is_empty())
        .any(|v| v[0].is_ascii_lowercase() || v[0] == b'S'))
}
fn conflicts(git: &GitCli, root: &Path) -> Result<Vec<MergeConflict>> {
    let raw = git
        .read(
            root,
            &["ls-files".as_ref(), "--unmerged".as_ref(), "-z".as_ref()],
            8 * 1024 * 1024,
        )?
        .bytes;
    let mut entries = BTreeMap::<String, MergeConflict>::new();
    for record in raw.split(|b| *b == 0).filter(|v| !v.is_empty()) {
        let tab = record
            .iter()
            .position(|b| *b == b'\t')
            .ok_or_else(|| AppError::new("git_parse", "Invalid conflict record"))?;
        let header = std::str::from_utf8(&record[..tab])
            .map_err(|_| AppError::new("git_parse", "Invalid conflict stage"))?;
        let fields = header.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(AppError::new("git_parse", "Invalid conflict stage"));
        }
        crate::validate_oid(fields[1])?;
        let path = NativePath::new(&crate::path_from_bytes(record[tab + 1..].to_vec())?);
        safe_path(root, &path)?;
        entries
            .entry(path.hex.clone())
            .or_insert_with(|| MergeConflict {
                path,
                stages: vec![],
            })
            .stages
            .push(MergeStage {
                mode: fields[0].into(),
                oid: fields[1].into(),
                stage: fields[2]
                    .parse()
                    .map_err(|_| AppError::new("git_parse", "Invalid conflict stage"))?,
            });
        if entries.len() > 2000 {
            return Err(AppError::new(
                "safety_budget",
                "More than 2,000 conflict paths exceed the inspection budget.",
            ));
        }
    }
    Ok(entries.into_values().collect())
}
fn changed_paths(git: &GitCli, root: &Path, original: &str) -> Result<Vec<NativePath>> {
    let raw = git
        .read(
            root,
            &[
                "diff".as_ref(),
                "--cached".as_ref(),
                "--name-only".as_ref(),
                "-z".as_ref(),
                "--no-ext-diff".as_ref(),
                "--no-textconv".as_ref(),
                "--no-renames".as_ref(),
                original.as_ref(),
                "--".as_ref(),
            ],
            8 * 1024 * 1024,
        )?
        .bytes;
    let mut paths = BTreeMap::new();
    for bytes in raw.split(|b| *b == 0).filter(|b| !b.is_empty()) {
        let path = NativePath::new(&crate::path_from_bytes(bytes.to_vec())?);
        safe_path(root, &path)?;
        paths.insert(path.hex.clone(), path);
    }
    for file in git.working_changes(root)?.files {
        paths.insert(file.path.hex.clone(), file.path);
    }
    if paths.len() > 2000 {
        return Err(AppError::new(
            "safety_budget",
            "Merge touches more than 2,000 paths.",
        ));
    }
    Ok(paths.into_values().collect())
}
// Honor custom marker widths and diff3 ancestor markers as well as the
// default seven-character markers. Do not mistake inline comparison text
// or an ordinary run of punctuation for a marker.
fn has_conflict_marker(line: &str) -> bool {
    let first = line.as_bytes().first().copied();
    if !matches!(first, Some(b'<' | b'>' | b'|')) {
        return false;
    }
    let width = line
        .as_bytes()
        .iter()
        .take_while(|&&b| Some(b) == first)
        .count();
    width >= 7
        && line
            .as_bytes()
            .get(width)
            .is_none_or(u8::is_ascii_whitespace)
}

impl GitCli {
    pub fn merge_state(&self, root: &Path) -> Result<MergeState> {
        let d = self.discover(root)?;
        if let Ok(Some(marker)) = load_marker(self, root) {
            if marker.request.operation == "rebase" {
                return rebase::state(self, root, &marker);
            }
        }
        let head = metadata(&d.git_dir.join("MERGE_HEAD"), 4096)?;
        let marker = load_marker(self, root);
        let mut state = MergeState {
            delete_source_branch: false,
            source_ref: None,
            operation: "merge".into(),
            mainline: None,
            suggested_message: None,
            phase: "none".into(),
            session_id: None,
            branch: None,
            original_oid: None,
            source_oid: None,
            recovery_ref: None,
            conflicts: vec![],
            token: String::new(),
            blocked_reason: None,
        };
        let marker = match marker {
            Ok(value) => value,
            Err(e) => {
                state.phase = "incomplete".into();
                state.blocked_reason = Some(e.message);
                return Ok(state);
            }
        };
        let pick = metadata(&d.git_dir.join("CHERRY_PICK_HEAD"), 4096)?;
        let revert = metadata(&d.git_dir.join("REVERT_HEAD"), 4096)?;
        if marker.is_none()
            && (d.git_dir.join("rebase-merge").exists() || d.git_dir.join("rebase-apply").exists())
        {
            state.operation = "rebase".into();
            state.phase = "external".into();
            state.conflicts = conflicts(self, root)?;
            state.blocked_reason = Some(
                "This rebase was started outside GitNova. Finish or recover it with Git.".into(),
            );
            return Ok(state);
        }
        if head.is_empty() && pick.is_empty() && revert.is_empty() && marker.is_none() {
            return Ok(state);
        }
        let status = self.status(root)?;
        state.branch = status.branch.clone();
        state.conflicts = conflicts(self, root)?;
        if let Some(marker) = marker {
            state.delete_source_branch = marker.request.delete_source_branch;
            state.source_ref = marker.request.source_ref.clone();
            state.operation = marker.request.operation.clone();
            state.mainline = marker.request.mainline;
            state.suggested_message = marker.suggested_message.clone();
            state.phase = "pending".into();
            state.session_id = Some(marker.request.session_id.clone());
            state.original_oid = Some(marker.original_oid.clone());
            state.source_oid = Some(marker.request.source_oid.clone());
            state.recovery_ref = Some(marker.recovery_ref.clone());
            let valid_native = if marker.request.operation == "merge" {
                let orig = metadata(&d.git_dir.join("ORIG_HEAD"), 4096)?;
                head.strip_suffix(b"\n").unwrap_or(&head) == marker.request.source_oid.as_bytes()
                    && orig.strip_suffix(b"\n").unwrap_or(&orig) == marker.original_oid.as_bytes()
                    && pick.is_empty()
                    && revert.is_empty()
            } else {
                let mut valid = marker.prepared;
                for name in ["MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "ORIG_HEAD"] {
                    valid &= marker.native_metadata.get(name)
                        == Some(&metadata(&d.git_dir.join(name), 4096)?);
                }
                valid && !d.git_dir.join("sequencer").exists()
            };
            if !valid_native
                || status.branch.as_ref() != Some(&marker.branch)
                || status.head.as_ref() != Some(&marker.original_oid)
                || d.git_dir.join("MERGE_AUTOSTASH").exists()
            {
                state.phase = "incomplete".into();
                state.blocked_reason=Some("Operation metadata or checkout changed outside the owned session. Inspect with Git before resolving, committing or aborting here.".into());
            }
        } else {
            state.operation = if !pick.is_empty() {
                "cherryPick"
            } else if !revert.is_empty() {
                "revert"
            } else {
                "merge"
            }
            .into();
            state.phase = "external".into();
            state.blocked_reason=Some("This Git operation was started outside GitNova. Inspect its files here; finish or recover it with Git.".into());
        }
        let mut bytes = head;
        bytes.extend(pick);
        bytes.extend(revert);
        bytes.extend(metadata(&d.git_dir.join("ORIG_HEAD"), 4096)?);
        bytes.extend(metadata(&d.git_dir.join("index"), 64 * 1024 * 1024)?);
        bytes.extend(self.config_snapshot(root)?);
        bytes.extend(metadata(&d.git_dir.join(MARKER), 65536)?);
        state.token = digest(&bytes);
        Ok(state)
    }
    pub fn preview_merge(&self, root: &Path, request: &MergeRequest) -> Result<MergeStartPreview> {
        if request.operation == "rebase" {
            return rebase::preview(self, root, request);
        }
        if request.operation != "merge" && request.method != MergeMethod::Auto {
            return Err(AppError::new(
                "merge_method",
                "Merge method applies only to merge operations.",
            ));
        }
        if !["merge", "cherryPick", "revert"].contains(&request.operation.as_str()) {
            return Err(AppError::new(
                "operation",
                "Choose merge, cherry-pick or revert.",
            ));
        }
        crate::validate_oid(&request.source_oid)?;
        if self
            .text(root, &["cat-file", "-t", &request.source_oid])?
            .trim()
            != "commit"
        {
            return Err(AppError::new("merge", "Choose a local commit object."));
        }
        let parents: Vec<String> = self
            .text(
                root,
                &[
                    "show",
                    "-s",
                    "--no-show-signature",
                    "--format=%P",
                    &request.source_oid,
                ],
            )?
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        if parents.len() > 256 {
            return Err(AppError::new(
                "safety_budget",
                "Mainline selection exceeds 256 parents.",
            ));
        }
        if request.operation == "merge" && request.mainline.is_some()
            || parents.len() <= 1 && request.mainline.is_some()
            || request
                .mainline
                .is_some_and(|n| n == 0 || n as usize > parents.len())
        {
            return Err(AppError::new(
                "mainline",
                "Select a valid parent only for a cherry-pick/revert of a merge commit.",
            ));
        }
        let source_message = self
            .read(
                root,
                &[
                    "show".as_ref(),
                    "-s".as_ref(),
                    "--no-show-signature".as_ref(),
                    "--format=%B".as_ref(),
                    request.source_oid.as_ref(),
                ],
                65536,
            )?
            .bytes;
        let source_message = String::from_utf8_lossy(&source_message)
            .trim_end()
            .to_owned();
        let mut request = request.clone();
        if request.session_id.is_empty() {
            request.session_id = uuid::Uuid::new_v4().to_string();
        }
        let id = uuid::Uuid::parse_str(&request.session_id)
            .map_err(|_| AppError::new("merge", "Invalid merge session identity"))?;
        if let Some(source) = &request.source_ref {
            if !["refs/heads/", "refs/remotes/", "refs/tags/"]
                .iter()
                .any(|prefix| source.starts_with(prefix))
            {
                return Err(AppError::new("merge", "Choose a graph branch or tag ref."));
            }
            self.text(root, &["check-ref-format", source])?;
            if self
                .text(
                    root,
                    &["rev-parse", "--verify", &format!("{source}^{{commit}}")],
                )?
                .trim()
                != request.source_oid
            {
                return Err(AppError::new(
                    "stale_preview",
                    "The source ref moved. Refresh and review again.",
                ));
            }
        }
        if self.merge_state(root)?.phase != "none" {
            return Err(AppError::new(
                "merge_blocked",
                "Finish or recover the current GitNova operation before starting another.",
            ));
        }
        let (d, index) = self.write_state(root)?;
        let status = self.status(root)?;
        let config = self.config_snapshot(root)?;
        let attrs = attributes(self, root)?;
        let mut blocked = None;
        if request.operation != "merge" && d.common_dir.join("shallow").exists() {
            blocked = Some("Shallow repositories can hide the selected parent delta. Use Git to complete the history, then review this operation again.".into());
        }
        if request.operation != "merge" && parents.len() > 1 && request.mainline.is_none() {
            blocked = Some("Select the mainline parent used as the change baseline.".into());
        }
        let preserved_submodules = if status.dirty && request.operation != "merge" {
            preserved::discover(self, root)?.unwrap_or_default()
        } else {
            Vec::new()
        };
        if status.dirty && preserved_submodules.is_empty() {
            blocked.get_or_insert_with(||"Commit or preserve uncommitted work before starting this operation. GitNova will not stash or discard it automatically.".into());
        }
        if status.branch.is_none() || status.head.is_none() {
            blocked =
                Some("Check out a committed local branch before starting this operation.".into());
        }
        let branch = status.branch.clone().unwrap_or_default();
        let original = status.head.clone().unwrap_or_default();
        if request.delete_source_branch {
            let action = source_cleanup::action(&request)?;
            if action.name == branch {
                return Err(AppError::new(
                    "merge_cleanup",
                    "Choose a different local source branch for post-merge deletion.",
                ));
            }
            if let Some(reason) = self.preview_ref_operation(root, &action)?.blocked_reason {
                blocked = Some(reason);
            }
        }
        if hidden_entries(self, root)? {
            blocked = Some(
                "Hidden assume-unchanged/skip-worktree entries require review before starting this operation."
                    .into(),
            );
        }
        if self.multiple_branch_worktrees(root, &branch)? {
            blocked=Some("This destination branch is checked out in multiple worktrees. Review them before applying this operation.".into());
        }
        if self
            .optional_text(root, &["config", "--get-regexp", r"^merge\..*\.driver$"])?
            .is_some()
            || self
                .optional_text(
                    root,
                    &[
                        "config",
                        "--get-all",
                        &format!("branch.{branch}.mergeOptions"),
                    ],
                )?
                .is_some_and(|v| !v.is_empty())
            || self
                .optional_text(root, &["config", "--bool", "--get", "merge.renormalize"])?
                .as_deref()
                == Some("true")
        {
            blocked=Some("Custom merge drivers, branch merge options or renormalization need a separately validated preview.".into());
        }
        if d.git_dir.join("index.lock").exists()
            || d.git_dir.join("HEAD.lock").exists()
            || d.common_dir.join("config.lock").exists()
        {
            blocked =
                Some("Another Git operation holds a native lock. Wait and review again.".into());
        }
        let suggested_message = match request.operation.as_str() {
            "cherryPick" => source_message.clone(),
            "revert" => format!(
                "Revert \"{}\"\n\nThis reverts commit {}.{}\n",
                source_message.lines().next().unwrap_or(""),
                request.source_oid,
                request.mainline.map_or_else(String::new, |n| format!(
                    " Mainline parent {n}: {}.",
                    parents[n as usize - 1]
                ))
            ),
            _ => format!(
                "Merge {} into {}",
                request
                    .source_ref
                    .as_deref()
                    .map(|r| r
                        .trim_start_matches("refs/heads/")
                        .trim_start_matches("refs/remotes/")
                        .trim_start_matches("refs/tags/"))
                    .unwrap_or(&request.source_oid),
                branch
            ),
        };
        if suggested_message.contains('\0')
            || serde_json::to_vec(&suggested_message)
                .map_err(|e| AppError::new("snapshot", e.to_string()))?
                .len()
                > 16384
        {
            return Err(AppError::new("safety_budget", "The prepared commit message exceeds its 16 KiB encoded budget or contains NUL. Inspect this operation with Git."));
        }
        let mut mode = if request.operation == "merge" {
            "merge_commit"
        } else {
            request.operation.as_str()
        }
        .to_owned();
        let mut files = vec![];
        let mut submodules = vec![];
        let mut conflict_paths = vec![];
        let mut result_tree = None;
        if blocked.is_none() {
            if request.operation == "merge" {
                if self
                    .text(
                        root,
                        &[
                            "merge-base",
                            "--is-ancestor",
                            &request.source_oid,
                            &original,
                        ],
                    )
                    .is_ok()
                {
                    mode = "up_to_date".into();
                    blocked =
                        Some("The selected history is already included in this branch.".into());
                } else if self
                    .text(
                        root,
                        &[
                            "merge-base",
                            "--is-ancestor",
                            &original,
                            &request.source_oid,
                        ],
                    )
                    .is_ok()
                {
                    if request.method != MergeMethod::MergeCommit {
                        mode = "fast_forward".into();
                    }
                } else if self
                    .optional_text(
                        root,
                        &["merge-base", "--all", &original, &request.source_oid],
                    )?
                    .is_none()
                {
                    blocked =
                        Some("Unrelated histories require a separate explicit review.".into());
                }
                if request.method == MergeMethod::FastForwardOnly
                    && mode != "fast_forward"
                    && blocked.is_none()
                {
                    blocked = Some("These branches have diverged. Fast-forward cannot create a merge commit; choose Create merge commit and review again.".into());
                }
            }
            if blocked.is_none() {
                let scratch = Scratch::new(self, root)?;
                let original_tree = self
                    .text(root, &["rev-parse", &format!("{original}^{{tree}}")])?
                    .trim()
                    .to_owned();
                let source_tree = self
                    .text(
                        root,
                        &["rev-parse", &format!("{}^{{tree}}", request.source_oid)],
                    )?
                    .trim()
                    .to_owned();
                let base_tree =
                    if let Some(parent) = parents.get(request.mainline.unwrap_or(1) as usize - 1) {
                        self.text(root, &["rev-parse", &format!("{parent}^{{tree}}")])?
                            .trim()
                            .to_owned()
                    } else {
                        scratch.text(&["read-tree", "--empty"])?;
                        scratch.text(&["write-tree"])?.trim().to_owned()
                    };
                let (delta_before, delta_after) = if request.operation == "merge" {
                    (original.clone(), request.source_oid.clone())
                } else if request.operation == "revert" {
                    (source_tree.clone(), base_tree.clone())
                } else {
                    (base_tree.clone(), source_tree.clone())
                };
                let raw = self
                    .read(
                        root,
                        &[
                            "diff".as_ref(),
                            "--raw".as_ref(),
                            "--no-abbrev".as_ref(),
                            "--no-renames".as_ref(),
                            "--no-ext-diff".as_ref(),
                            "--no-textconv".as_ref(),
                            "-z".as_ref(),
                            delta_before.as_ref(),
                            delta_after.as_ref(),
                            "--".as_ref(),
                        ],
                        4 * 1024 * 1024,
                    )?
                    .bytes;
                let compared_submodules = gitlink_changes(&raw)?;
                if request.operation != "merge" && !compared_submodules.is_empty() {
                    blocked=Some("This operation changes submodule pointers or layout. GitNova does not yet support applying those changes; no operation has started.".into());
                }
                if blocked.is_none() {
                    scratch.text(&["update-ref", "refs/heads/preview", &original])?;
                    scratch.text(&["symbolic-ref", "HEAD", "refs/heads/preview"])?;
                    scratch.text(&["read-tree", &original])?;
                    let (ours, incoming) = if request.operation == "merge" {
                        (original.clone(), request.source_oid.clone())
                    } else {
                        let base = scratch.commit(&delta_before, None)?;
                        (
                            scratch.commit(&original_tree, Some(&base))?,
                            scratch.commit(&delta_after, Some(&base))?,
                        )
                    };
                    let output = scratch.run(
                        &[
                            "merge-tree".into(),
                            "--write-tree".into(),
                            "--name-only".into(),
                            "--no-messages".into(),
                            "-z".into(),
                            ours.into(),
                            incoming.into(),
                        ],
                        None,
                        true,
                    )?;
                    let mut parts = output.split(|b| *b == 0);
                    let tree = String::from_utf8(parts.next().unwrap_or_default().to_vec())
                        .map_err(|_| AppError::new("merge_preview", "Invalid result tree"))?;
                    crate::validate_oid(&tree)?;
                    for path in parts.take_while(|v| !v.is_empty()) {
                        let path = NativePath::new(&crate::path_from_bytes(path.to_vec())?);
                        safe_path(root, &path)?;
                        conflict_paths.push(path);
                    }
                    let raw = scratch.run(
                        &[
                            "diff".into(),
                            "--name-status".into(),
                            "--no-renames".into(),
                            "-z".into(),
                            original.clone().into(),
                            tree.clone().into(),
                            "--".into(),
                        ],
                        None,
                        false,
                    )?;
                    let mut rows = raw.split(|b| *b == 0).filter(|v| !v.is_empty());
                    while let Some(status) = rows.next() {
                        let name = rows.next().ok_or_else(|| {
                            AppError::new("merge_preview", "Invalid merge path record")
                        })?;
                        files.push(BranchFileChange {
                            path: NativePath::new(&crate::path_from_bytes(name.to_vec())?),
                            status: String::from_utf8_lossy(status).into_owned(),
                        });
                    }
                    if request.operation == "merge" {
                        let result_raw = scratch.run(
                            &[
                                "diff".into(),
                                "--raw".into(),
                                "--no-abbrev".into(),
                                "--no-renames".into(),
                                "--no-ext-diff".into(),
                                "--no-textconv".into(),
                                "-z".into(),
                                original.clone().into(),
                                tree.clone().into(),
                                "--".into(),
                            ],
                            None,
                            false,
                        )?;
                        let result_changes = gitlink_changes(&result_raw)?;
                        submodules = compared_submodules;
                        for changed in &result_changes {
                            if !submodules.iter().any(|s| s.path.hex == changed.path.hex) {
                                submodules.push(changed.clone());
                            }
                        }
                        for item in &mut submodules {
                            item.conflicted = conflict_paths.iter().any(|p| p.hex == item.path.hex);
                            item.result_oid = if item.conflicted {
                                None
                            } else {
                                result_changes
                                    .iter()
                                    .find(|s| s.path.hex == item.path.hex)
                                    .map_or_else(
                                        || item.current_oid.clone(),
                                        |s| s.incoming_oid.clone(),
                                    )
                            };
                        }
                        if !result_changes.is_empty() || submodules.iter().any(|s| s.conflicted) {
                            blocked=Some("The predicted merge changes submodule pointers or layout, or has a submodule conflict. GitNova does not yet support applying this result; no merge has started.".into());
                        }
                    }
                    if files.len() > 2000 || conflict_paths.len() > 2000 {
                        return Err(AppError::new(
                            "safety_budget",
                            "Merge preview exceeds 2,000 paths.",
                        ));
                    }
                    if request.operation != "merge"
                        && tree == original_tree
                        && conflict_paths.is_empty()
                    {
                        blocked = Some("This operation has no changes on the current branch. Nothing will be committed.".into());
                    }
                    if request.operation != "merge" && !files.is_empty() {
                        let mut args: Vec<OsString> = vec![
                            "ls-files".into(),
                            "--others".into(),
                            "--ignored".into(),
                            "--exclude-standard".into(),
                            "-z".into(),
                            "--".into(),
                        ];
                        for file in &files {
                            safe_path(root, &file.path)?;
                            args.push(file.path.to_path()?.into_os_string());
                        }
                        if !self
                            .read(
                                root,
                                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                                4 * 1024 * 1024,
                            )?
                            .bytes
                            .is_empty()
                        {
                            blocked = Some("Ignored local files occupy affected paths. Preserve them before starting this operation.".into());
                        }
                    }
                    result_tree = Some(tree);
                }
            }
        }
        if !preserved_submodules.is_empty()
            && files.iter().any(|f| {
                f.path.display == ".gitmodules"
                    || preserved_submodules.iter().any(|p| {
                        let a = p.path.to_path().ok();
                        let b = f.path.to_path().ok();
                        matches!((a,b),(Some(a),Some(b)) if a.starts_with(&b)||b.starts_with(&a))
                    })
            })
        {
            blocked=Some("This operation overlaps a preserved submodule checkout. Review the submodule separately before starting.".into());
        }
        let recovery_ref = format!("refs/gitnova/recovery/merge-{}", id.simple());
        let data = serde_json::to_vec(&(
            &request,
            &status,
            index,
            config,
            attrs,
            &mode,
            &files,
            &conflict_paths,
            &result_tree,
            &submodules,
            &preserved_submodules,
        ))
        .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        Ok(MergeStartPreview {
            rebase: None,
            preserved_submodules,
            submodules,
            parents,
            suggested_message,
            request,
            branch,
            original_oid: original,
            result_tree,
            mode,
            files,
            conflicts: conflict_paths,
            recovery_ref,
            token: digest(&data),
            blocked_reason: blocked,
        })
    }
    pub fn begin_merge(&self, root: &Path, request: &MergeRequest, token: &str) -> Result<()> {
        if request.operation == "rebase" {
            return rebase::begin(self, root, request, token);
        }
        let preview = self.preview_merge(root, request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Operation source, destination, working state or configuration changed. Review again.",
            ));
        }
        if let Some(reason) = preview.blocked_reason {
            return Err(AppError::new("merge_blocked", reason));
        }
        let d = self.discover(root)?;
        let id = uuid::Uuid::parse_str(&preview.request.session_id)
            .map_err(|_| AppError::new("merge", "Invalid session"))?;
        let dir = d.git_dir.join(format!("gitnova-merge-{}", id.simple()));
        recovery::private_dir(&dir)?;
        let mut marker = Marker {
            rebase: None,
            version: 1,
            request: preview.request.clone(),
            branch: preview.branch.clone(),
            original_oid: preview.original_oid.clone(),
            recovery_ref: preview.recovery_ref.clone(),
            session_dir: NativePath::new(&dir),
            prepared: false,
            preserved_submodules: preview.preserved_submodules.clone(),
            native_metadata: BTreeMap::new(),
            suggested_message: Some(preview.suggested_message.clone()),
        };
        let bytes = marker_bytes(&marker)?;
        recovery::private_file(&d.git_dir.join(MARKER), &bytes)?;
        crate::working::run_ref_transaction(
            root,
            &format!(
                "start\ncreate {} {}\ncreate {}-incoming {}\nprepare\ncommit\n",
                preview.recovery_ref,
                preview.original_oid,
                preview.recovery_ref,
                preview.request.source_oid
            ),
        )?;
        let mut args = vec![
            "-c".into(),
            "submodule.recurse=false".into(),
            "-c".into(),
            "rerere.enabled=false".into(),
            "-c".into(),
            "rerere.autoupdate=false".into(),
            "merge".into(),
            "--no-autostash".into(),
            "--no-overwrite-ignore".into(),
            "--no-edit".into(),
            "--strategy=ort".into(),
        ];
        if preview.mode == "fast_forward" {
            args.push("--ff-only".into());
        } else {
            args.extend(["--no-ff".into(), "--no-commit".into()]);
        }
        if request.operation != "merge" {
            args = vec![
                "-c".into(),
                "submodule.recurse=false".into(),
                "-c".into(),
                "rerere.enabled=false".into(),
                "-c".into(),
                "rerere.autoupdate=false".into(),
                if request.operation == "cherryPick" {
                    "cherry-pick".into()
                } else {
                    "revert".into()
                },
                "--no-commit".into(),
                "--strategy=ort".into(),
            ];
            if let Some(n) = request.mainline {
                args.extend(["--mainline".into(), n.to_string().into()]);
            }
        }
        args.push(preview.request.source_oid.clone().into());
        let result = run_offline_write(root, &args);
        preserved::verify(self,root,&marker).map_err(|_|AppError::new("operation_unknown","A preserved submodule changed during the operation. Inspect the child and recovery refs; no rollback was attempted."))?;
        if request.operation != "merge" && (result.is_ok() || !conflicts(self, root)?.is_empty()) {
            marker.prepared = true;
            for name in ["MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "ORIG_HEAD"] {
                marker
                    .native_metadata
                    .insert(name.into(), metadata(&d.git_dir.join(name), 4096)?);
            }
            // Atomically publish readiness; a crash before this leaves an inspect-only session.
            let temporary = d
                .git_dir
                .join(format!("gitnova-operation-{}.tmp", id.simple()));
            recovery::private_file(&temporary, &marker_bytes(&marker)?)?;
            fs::rename(temporary, d.git_dir.join(MARKER))?;
        }
        let state = self.merge_state(root)?;
        if preview.mode == "fast_forward"
            && result.is_ok()
            && self.status(root)?.head.as_deref() == Some(&preview.request.source_oid)
            && self.status(root)?.branch.as_deref() == Some(&preview.branch)
        {
            fs::remove_file(d.git_dir.join(MARKER))?;
            return Ok(());
        }
        if state.phase == "pending" {
            if state.conflicts.is_empty() {
                let (_, tree) = index_tree(self, root)?;
                if Some(&tree) != preview.result_tree.as_ref() {
                    return Err(AppError::new("operation_unknown","Native staged result differs from prediction. Inspect the actual staged result before finishing."));
                }
            }
            if !preview.conflicts.is_empty() || state.conflicts.is_empty() {
                return Ok(());
            }
            return Err(AppError::new(
                "operation_unknown",
                "Git found additional conflicts. Inspect the active operation before continuing.",
            ));
        }
        if metadata(&d.git_dir.join("MERGE_HEAD"), 4096)?.is_empty()
            && self.status(root)?.head.as_deref() == Some(&preview.original_oid)
            && !self.status(root)?.dirty
        {
            fs::remove_file(d.git_dir.join(MARKER))?;
        }
        result.and_then(|_|Err(AppError::new("operation_unknown","Operation outcome differs from its preview. Inspect the repository and recovery ref before continuing.")))
    }
}
fn index_tree(git: &GitCli, root: &Path) -> Result<(Scratch, String)> {
    let scratch = Scratch::new(git, root)?;
    scratch.text(&["read-tree", "--empty"])?;
    let entries = git
        .read(
            root,
            &["ls-files".as_ref(), "--stage".as_ref(), "-z".as_ref()],
            16 * 1024 * 1024,
        )?
        .bytes;
    scratch.run(
        &["update-index".into(), "-z".into(), "--index-info".into()],
        Some(&entries),
        false,
    )?;
    let tree = scratch.text(&["write-tree"])?.trim().to_owned();
    crate::validate_oid(&tree)?;
    Ok((scratch, tree))
}
fn candidate(
    git: &GitCli,
    root: &Path,
    conflict: &MergeConflict,
    action: &MergeAction,
) -> Result<(String, Option<Vec<u8>>)> {
    let choice = action.choice.as_deref().unwrap_or("");
    let path = &conflict.path;
    if choice == "ours" || choice == "theirs" {
        let stage = if choice == "ours" { 2 } else { 3 };
        let entry = conflict.stages.iter().find(|s| s.stage == stage);
        return if let Some(entry) = entry {
            if !["100644", "100755", "120000"].contains(&entry.mode.as_str()) {
                Err(AppError::new(
                    "merge_resolve",
                    "This conflict's object mode needs manual inspection.",
                ))
            } else {
                Ok((
                    entry.mode.clone(),
                    Some(
                        git.read(
                            root,
                            &["cat-file".as_ref(), "blob".as_ref(), entry.oid.as_ref()],
                            16 * 1024 * 1024,
                        )?
                        .bytes,
                    ),
                ))
            }
        } else {
            Ok(("100644".into(), None))
        };
    }
    let (kind, mode, data) = recovery::work_entry(root, path)?;
    if kind == "directory" {
        return Err(AppError::new(
            "merge_resolve",
            "Directory conflicts need manual inspection.",
        ));
    }
    let mode = if kind == "symlink" {
        "120000"
    } else if mode & 0o111 != 0 {
        "100755"
    } else {
        "100644"
    }
    .to_owned();
    if choice == "working" {
        if let Some(bytes) = &data {
            if let Ok(text) = std::str::from_utf8(bytes) {
                if text.lines().any(has_conflict_marker) {
                    return Err(AppError::new(
                        "merge_resolve",
                        "Conflict markers remain. Edit the result before marking it resolved.",
                    ));
                }
            }
        }
        Ok((mode, data))
    } else if choice == "edited" {
        if kind != "file"
            || data
                .as_ref()
                .is_some_and(|bytes| bytes.contains(&0) || std::str::from_utf8(bytes).is_err())
            || conflict.stages.iter().any(|s| s.mode == "120000")
        {
            return Err(AppError::new(
                "merge_resolve",
                "Only regular UTF-8 text results can be edited here.",
            ));
        }
        let text = action
            .content
            .as_ref()
            .ok_or_else(|| AppError::new("merge_resolve", "Provide the edited result."))?;
        if text.len() > 512 * 1024 || text.contains('\0') || text.lines().any(has_conflict_marker) {
            return Err(AppError::new(
                "merge_resolve",
                "The edited result exceeds its text budget or still contains conflict markers.",
            ));
        }
        Ok((mode, Some(text.as_bytes().to_vec())))
    } else {
        Err(AppError::new(
            "merge_resolve",
            "Choose current, incoming or an explicitly edited result.",
        ))
    }
}
impl GitCli {
    pub fn merge_file(&self, root: &Path, path: &NativePath) -> Result<MergeFile> {
        self.merge_file_scope(root, path, true)
    }
    pub fn merge_file_scope(
        &self,
        root: &Path,
        path: &NativePath,
        staged: bool,
    ) -> Result<MergeFile> {
        let state = self.merge_state(root)?;
        if state.phase == "none" {
            return Err(AppError::new("merge", "No active merge."));
        }
        safe_path(root, path)?;
        let conflict = state.conflicts.iter().find(|c| &c.path == path);
        let mut args: Vec<OsString> = vec![
            "diff".into(),
            "--no-ext-diff".into(),
            "--no-textconv".into(),
            "--no-renames".into(),
            "--src-prefix=a/".into(),
            "--dst-prefix=b/".into(),
        ];
        if conflict.is_some() {
            args.push("--ours".into());
        } else if staged {
            args.push("--cached".into());
            if let Some(ref oid) = state.original_oid {
                args.push(oid.into());
            }
        }
        args.push("--".into());
        args.push(path.to_path()?.into_os_string());
        let output = self.execute(
            root,
            &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            512 * 1024,
            true,
        )?;
        let truncated = output.truncated;
        let mut diff = String::from_utf8_lossy(&output.bytes).into_owned();
        let absolute = safe_path(root, path)?;
        let oversized = fs::symlink_metadata(&absolute)
            .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink() && m.len() > 512 * 1024);
        let (kind, mode, data) = if oversized {
            ("file".into(), 0, None)
        } else {
            recovery::work_entry(root, path)?
        };
        if !staged
            && conflict.is_none()
            && self
                .working_changes(root)?
                .files
                .iter()
                .any(|f| f.path == *path && f.index_status == "?")
        {
            if let Some(bytes) = data.as_deref() {
                let scratch = Scratch::new(self, root)?;
                let before = scratch.blob_tree(&path.to_path()?, "100644", None)?;
                let after = scratch.blob_tree(
                    &path.to_path()?,
                    if kind == "symlink" {
                        "120000"
                    } else if mode & 0o111 != 0 {
                        "100755"
                    } else {
                        "100644"
                    },
                    Some(bytes),
                )?;
                diff = scratch.diff_preview(&before, &after)?.0;
            } else {
                let old = crate::working::quoted_diff_path(&path.to_path()?, b"a/")?;
                let new = crate::working::quoted_diff_path(&path.to_path()?, b"b/")?;
                diff=format!("diff --git {old} {new}\nnew file mode 100644\nBinary files /dev/null and {new} differ\n");
            }
        }
        let working_text = data.and_then(|bytes| {
            if bytes.len() <= 512 * 1024 && !bytes.contains(&0) {
                String::from_utf8(bytes).ok()
            } else {
                None
            }
        });
        // Stage 2 is the current checkout; stage 3 is the incoming side
        // (the inverse/pre-commit side for a revert). Never substitute HEAD
        // or the selected commit for these actual conflict blobs.
        let mut sides = Vec::with_capacity(2);
        for stage in [2, 3] {
            let text = if let Some(entry) =
                conflict.and_then(|c| c.stages.iter().find(|s| s.stage == stage))
            {
                let output = self.execute(
                    root,
                    &["cat-file".as_ref(), "blob".as_ref(), entry.oid.as_ref()],
                    512 * 1024,
                    true,
                )?;
                if output.truncated || output.bytes.contains(&0) {
                    None
                } else {
                    String::from_utf8(output.bytes).ok()
                }
            } else {
                Some(String::new())
            };
            sides.push(text);
        }
        let theirs_text = sides.pop().flatten();
        let ours_text = sides.pop().flatten();
        if self.merge_state(root)?.token != state.token {
            return Err(AppError::new(
                "stale_preview",
                "Conflict versions changed while reading. Select the file again.",
            ));
        }
        let editable = !oversized
            && kind == "file"
            && working_text.is_some()
            && conflict.is_some_and(|c| {
                !c.stages
                    .iter()
                    .any(|s| s.mode == "120000" || s.mode == "160000")
            });
        Ok(MergeFile{truncated,path:path.clone(),diff,working_text,ours_text,theirs_text,editable,reason:(!editable).then(||"Binary, symlink, deleted or large results can be selected by version or inspected externally.".into()),token:state.token})
    }
    fn preview_merge_release(
        &self,
        root: &Path,
        action: &MergeAction,
        state: MergeState,
    ) -> Result<MergeActionPreview> {
        let marker = load_marker(self, root)?
            .ok_or_else(|| AppError::new("merge_metadata", "No owned session"))?;
        let discovery = self.discover(root)?;
        let status = self.status(root)?;
        let mut blocked = None;
        for name in [
            "MERGE_HEAD",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "MERGE_AUTOSTASH",
            "rebase-apply",
            "rebase-merge",
            "sequencer",
            "index.lock",
            "HEAD.lock",
        ] {
            if discovery.git_dir.join(name).exists() {
                blocked = Some("A native Git operation or lock is still active. Finish or recover it before closing this session.".into());
            }
        }
        if state.phase != "incomplete" {
            blocked = Some("Only an interrupted owned session without an active Git operation can be closed here.".into());
        }
        if !state.conflicts.is_empty() {
            blocked = Some(
                "Resolve the remaining index conflicts with Git before closing this session."
                    .into(),
            );
        }
        let destination = marker.session_dir.to_path()?.join("closed-session.json");
        if destination.exists() || destination.symlink_metadata().is_ok() {
            blocked = Some("A prior session archive exists. Inspect recovery metadata with Git before closing this session.".into());
        }
        let token = digest(
            &serde_json::to_vec(&(
                action,
                &state,
                marker_bytes(&marker)?,
                &status.head,
                &status.branch,
                &blocked,
            ))
            .map_err(|e| AppError::new("snapshot", e.to_string()))?,
        );
        Ok(MergeActionPreview {
            truncated: false,
            action: action.clone(),
            state,
            diff: String::new(),
            // Closing ownership changes no Git files or refs, and must not depend
            // on historical objects still being available after external recovery.
            paths: vec![],
            recovery_dir: Some(NativePath::new(&destination)),
            expected_tree: None,
            token,
            blocked_reason: blocked,
        })
    }
    pub fn preview_merge_action(
        &self,
        root: &Path,
        action: &MergeAction,
    ) -> Result<MergeActionPreview> {
        if let Some(marker) = load_marker(self, root)? {
            if marker.request.operation == "rebase"
                && !matches!(action.kind.as_str(), "resolve" | "release")
            {
                return rebase::preview_action(self, root, &marker, action);
            }
        }
        if action.kind == "unstage" {
            return unstage::preview(self, root, action, false);
        }
        if !["resolve", "finish", "abort", "release"].contains(&action.kind.as_str()) {
            return Err(AppError::new(
                "merge",
                "Choose resolve, finish, abort or session cleanup.",
            ));
        }
        if action.kind != "resolve"
            && (action.path.is_some() || action.choice.is_some() || action.content.is_some())
            || action.kind != "finish" && !action.message.is_empty()
        {
            return Err(AppError::new(
                "merge",
                "Unexpected fields for this merge action.",
            ));
        }
        let state = self.merge_state(root)?;
        if action.kind == "release" {
            return self.preview_merge_release(root, action, state);
        }
        if state.phase != "pending" {
            return Err(AppError::new(
                "merge_blocked",
                state
                    .blocked_reason
                    .unwrap_or_else(|| "No owned merge is ready for this action.".into()),
            ));
        }
        let marker = load_marker(self, root)?
            .ok_or_else(|| AppError::new("merge_metadata", "No owned session"))?;
        let d = self.discover(root)?;
        let (snapshot, mut paths) = recovery::fingerprint(self, root, &marker)?;
        let mut blocked = None;
        let mut diff = String::new();
        let mut expected_tree = None;
        let mut truncated = false;
        if hidden_entries(self, root)? {
            blocked =
                Some("Hidden index entries need inspection before changing this operation.".into());
        }
        if d.git_dir.join("index.lock").exists()
            || d.git_dir.join("HEAD.lock").exists()
            || d.common_dir.join("config.lock").exists()
        {
            blocked =
                Some("Another Git operation holds a native lock. Wait and review again.".into());
        }
        for name in [
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "rebase-apply",
            "rebase-merge",
            "sequencer",
        ] {
            if d.git_dir.join(name).exists()
                && !(name == "CHERRY_PICK_HEAD" && marker.request.operation == "cherryPick"
                    || name == "REVERT_HEAD" && marker.request.operation == "revert"
                    || name == "rebase-merge" && marker.request.operation == "rebase")
            {
                blocked = Some(
                    "Another Git operation is active. Inspect it before changing this operation."
                        .into(),
                );
            }
        }
        if action.kind == "resolve" {
            let path = action
                .path
                .as_ref()
                .ok_or_else(|| AppError::new("merge_resolve", "Select a conflict path."))?;
            let staged_path;
            let conflict = if let Some(conflict) = state.conflicts.iter().find(|c| &c.path == path)
            {
                conflict
            } else {
                if action.choice.as_deref() != Some("working")
                    || !self
                        .working_changes(root)?
                        .files
                        .iter()
                        .any(|file| file.path == *path && file.unstaged)
                {
                    return Err(AppError::new(
                        "stale_preview",
                        "Select an unresolved or unstaged merge path, then review again.",
                    ));
                }
                staged_path = MergeConflict {
                    path: path.clone(),
                    stages: vec![],
                };
                &staged_path
            };
            let (kind, mode, bytes) = recovery::work_entry(root, path)?;
            if kind == "directory" {
                blocked = Some("Directory conflicts need manual inspection.".into());
            } else {
                let scratch = Scratch::new(self, root)?;
                let before = scratch.blob_tree(
                    &path.to_path()?,
                    if kind == "symlink" {
                        "120000"
                    } else if mode & 0o111 != 0 {
                        "100755"
                    } else {
                        "100644"
                    },
                    bytes.as_deref(),
                )?;
                let (mode, data) = candidate(self, root, conflict, action)?;
                let after = scratch.blob_tree(&path.to_path()?, &mode, data.as_deref())?;
                diff = scratch.diff(&before, &after)?;
                // Pin the proposed bytes/mode too: a working-file candidate may
                // change between the recovery fingerprint and its own read.
                expected_tree = Some(after);
            }
            paths = vec![path.clone()];
        } else if action.kind == "finish" {
            if !state.conflicts.is_empty() {
                blocked =
                    Some("Resolve and stage every conflict before finishing this commit.".into());
            }
            if action.message.trim().is_empty()
                || action.message.len() > 65536
                || action.message.contains('\0')
            {
                blocked = Some("Enter a nonempty commit message within 64 KiB.".into());
            }
            if state.conflicts.is_empty() {
                let (scratch, tree) = index_tree(self, root)?;
                (diff, truncated) = scratch.diff_preview(&marker.original_oid, &tree)?;
                if marker.request.operation != "merge"
                    && tree
                        == self
                            .text(
                                root,
                                &["rev-parse", &format!("{}^{{tree}}", marker.original_oid)],
                            )?
                            .trim()
                {
                    blocked = Some("The staged result is empty. Review abort instead of creating an empty commit.".into());
                }
                expected_tree = Some(tree);
                let raw = self
                    .read(
                        root,
                        &[
                            "diff".as_ref(),
                            "--cached".as_ref(),
                            "--raw".as_ref(),
                            "--no-renames".as_ref(),
                            marker.original_oid.as_ref(),
                            "--".as_ref(),
                        ],
                        4 * 1024 * 1024,
                    )?
                    .bytes;
                if raw.split(|b| *b == b'\n').any(|r| {
                    r.starts_with(b":160000 ")
                        || r.split(|b| *b == b' ').nth(1) == Some(&b"160000"[..])
                }) {
                    blocked = Some(
                        "Staged submodule pointer changes need separate review before finishing."
                            .into(),
                    );
                }
            }
        }
        if action.kind != "finish" {
            if let Some(reason) = recovery::backup_blocker(self, root, &paths)? {
                blocked = Some(reason);
            }
        }
        if diff.len() > 512 * 1024 {
            return Err(AppError::new(
                "safety_budget",
                "Operation patch exceeds the complete 512 KiB review budget.",
            ));
        }
        let token = digest(
            &serde_json::to_vec(&(action, &state, snapshot, &expected_tree))
                .map_err(|e| AppError::new("snapshot", e.to_string()))?,
        );
        let recovery_dir = (action.kind != "finish").then(|| {
            NativePath::new(
                &marker
                    .session_dir
                    .to_path()
                    .unwrap_or_default()
                    .join(format!("{}-{token}", action.kind)),
            )
        });
        Ok(MergeActionPreview {
            truncated,
            action: action.clone(),
            state,
            diff,
            paths,
            recovery_dir,
            expected_tree,
            token,
            blocked_reason: blocked,
        })
    }
    pub fn apply_merge_action(&self, root: &Path, action: &MergeAction, token: &str) -> Result<()> {
        if let Some(marker) = load_marker(self, root)? {
            if marker.request.operation == "rebase"
                && !matches!(action.kind.as_str(), "resolve" | "release")
            {
                return rebase::apply_action(self, root, &marker, action, token);
            }
        }
        if action.kind == "unstage" {
            return unstage::apply(self, root, action, token);
        }
        let preview = self.preview_merge_action(root, action)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Operation index, files or configuration changed. Review again.",
            ));
        }
        if let Some(reason) = preview.blocked_reason.clone() {
            return Err(AppError::new("merge_blocked", reason));
        }
        let marker = load_marker(self, root)?
            .ok_or_else(|| AppError::new("merge_metadata", "Missing owned operation"))?;
        let d = self.discover(root)?;
        if action.kind == "release" {
            let archive = preview
                .recovery_dir
                .as_ref()
                .ok_or_else(|| AppError::new("merge_metadata", "Missing session archive"))?
                .to_path()?;
            let original = metadata(&d.git_dir.join(MARKER), 65536)?;
            let checkout = self.status(root)?;
            // Bind an external commit/checkout even when its index tree is identical.
            if self
                .preview_merge_release(root, action, self.merge_state(root)?)?
                .token
                != token
            {
                return Err(AppError::new(
                    "stale_preview",
                    "The checkout or session changed. Review session cleanup again.",
                ));
            }
            recovery::private_file(&archive, &original)?;
            // Revalidate the live marker and native operation indicators before removing only our marker.
            let latest = self.preview_merge_release(root, action, self.merge_state(root)?)?;
            // The newly written archive is an expected difference; compare the reviewed native state separately.
            let status = self.status(root)?;
            if status.head != checkout.head
                || status.branch != checkout.branch
                || latest.state.token != preview.state.token
                || metadata(&d.git_dir.join(MARKER), 65536)? != original
            {
                return Err(AppError::new("operation_unknown", "Session metadata changed while saving its archive. Inspect the archive and native Git state before retrying."));
            }
            for name in [
                "MERGE_HEAD",
                "CHERRY_PICK_HEAD",
                "REVERT_HEAD",
                "MERGE_AUTOSTASH",
                "rebase-apply",
                "rebase-merge",
                "sequencer",
                "index.lock",
                "HEAD.lock",
            ] {
                if d.git_dir.join(name).exists() {
                    return Err(AppError::new("operation_unknown", "A native Git operation started while saving session metadata. Inspect it before retrying."));
                }
            }
            fs::remove_file(d.git_dir.join(MARKER))?;
            return Ok(());
        }
        if action.kind != "finish" {
            recovery::save(self, root, &marker, &preview)?;
            let latest = self.preview_merge_action(root, action)?;
            if latest.token != token || latest.blocked_reason.is_some() {
                return Err(AppError::new("stale_preview","Files changed while capturing recovery data. Nothing was replaced; review again."));
            }
        }
        if action.kind == "abort" {
            let args = if marker.request.operation == "merge" {
                vec![
                    "-c".into(),
                    "submodule.recurse=false".into(),
                    "merge".into(),
                    "--abort".into(),
                ]
            } else {
                vec![
                    "-c".into(),
                    "submodule.recurse=false".into(),
                    "reset".into(),
                    "--merge".into(),
                    marker.original_oid.clone().into(),
                ]
            };
            run_offline_write(root, &args)?;
            let status = self.status(root)?;
            if status.branch.as_ref() != Some(&marker.branch)
                || status.head.as_ref() != Some(&marker.original_oid)
                || d.git_dir.join("MERGE_HEAD").exists()
                || d.git_dir.join("CHERRY_PICK_HEAD").exists()
                || d.git_dir.join("REVERT_HEAD").exists()
                || d.git_dir.join("sequencer").exists()
            {
                return Err(AppError::new("operation_unknown","Abort result differs from the original checkout. Inspect the recovery snapshot before retrying."));
            }
            preserved::verify(self,root,&marker).map_err(|_|AppError::new("operation_unknown","A preserved submodule changed during the operation. Inspect the child and recovery refs; no rollback was attempted."))?;
            fs::remove_file(d.git_dir.join(MARKER))?;
            return Ok(());
        }
        if action.kind == "finish" {
            self.commit_owned_merge(root, &action.message, &preview.state.token)?;
            let status = self.status(root)?;
            let head = status
                .head
                .as_deref()
                .ok_or_else(|| AppError::new("operation_unknown", "Missing reviewed commit"))?;
            let parents = self.text(
                root,
                &["show", "-s", "--no-show-signature", "--format=%P", head],
            )?;
            let tree = self.text(root, &["rev-parse", &format!("{head}^{{tree}}")])?;
            if status.branch.as_ref() != Some(&marker.branch)
                || parents.trim()
                    != if marker.request.operation == "merge" {
                        format!("{} {}", marker.original_oid, marker.request.source_oid)
                    } else {
                        marker.original_oid.clone()
                    }
                || Some(tree.trim()) != preview.expected_tree.as_deref()
                || d.git_dir.join("MERGE_HEAD").exists()
                || d.git_dir.join("CHERRY_PICK_HEAD").exists()
                || d.git_dir.join("REVERT_HEAD").exists()
                || d.git_dir.join("sequencer").exists()
            {
                return Err(AppError::new("operation_unknown","Git committed a result different from the reviewed tree or parents. Inspect HEAD and hooks; no rollback was attempted."));
            }
            preserved::verify(self,root,&marker).map_err(|_|AppError::new("operation_unknown","A preserved submodule changed during the operation. Inspect the child and recovery refs; no rollback was attempted."))?;
            fs::remove_file(d.git_dir.join(MARKER))?;
            return Ok(());
        }
        let path = action
            .path
            .as_ref()
            .ok_or_else(|| AppError::new("merge_resolve", "Missing path"))?;
        let state = self.merge_state(root)?;
        let staged_path;
        let conflict = if let Some(conflict) = state.conflicts.iter().find(|c| &c.path == path) {
            conflict
        } else {
            if action.choice.as_deref() != Some("working")
                || !self
                    .working_changes(root)?
                    .files
                    .iter()
                    .any(|f| f.path == *path && f.unstaged)
            {
                return Err(AppError::new("stale_preview", "Operation path changed."));
            }
            staged_path = MergeConflict {
                path: path.clone(),
                stages: vec![],
            };
            &staged_path
        };
        let (_, data) = candidate(self, root, conflict, action)?;
        let absolute = safe_path(root, path)?;
        if action.choice.as_deref() == Some("edited") {
            let bytes = data
                .as_deref()
                .ok_or_else(|| AppError::new("merge_resolve", "No edited content"))?;
            let mut options = fs::OpenOptions::new();
            options.write(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NOFOLLOW);
            }
            if !fs::symlink_metadata(&absolute)?.file_type().is_file() {
                return Err(AppError::new(
                    "stale_preview",
                    "Merge result is no longer a regular file.",
                ));
            }
            let mut file = options.open(&absolute)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        } else if matches!(action.choice.as_deref(), Some("ours" | "theirs")) && data.is_some() {
            run_offline_write(
                root,
                &[
                    "checkout".into(),
                    if action.choice.as_deref() == Some("ours") {
                        "--ours".into()
                    } else {
                        "--theirs".into()
                    },
                    "--".into(),
                    path.to_path()?.into_os_string(),
                ],
            )?;
        }
        if data.is_none() {
            if fs::symlink_metadata(&absolute).is_ok() {
                if fs::symlink_metadata(&absolute)?.is_dir() {
                    return Err(AppError::new(
                        "merge_resolve",
                        "Directory removal is not enabled.",
                    ));
                }
                fs::remove_file(&absolute)?;
            }
            run_offline_write(
                root,
                &[
                    "rm".into(),
                    "--force".into(),
                    "--".into(),
                    path.to_path()?.into_os_string(),
                ],
            )?;
        } else {
            run_offline_write(
                root,
                &["add".into(), "--".into(), path.to_path()?.into_os_string()],
            )?;
        }
        let state = self.merge_state(root)?;
        if state.phase != "pending" || state.conflicts.iter().any(|c| c.path == *path) {
            return Err(AppError::new("operation_unknown","Resolution did not produce the reviewed staged state. Inspect the active merge and recovery snapshot."));
        }
        Ok(())
    }
}
