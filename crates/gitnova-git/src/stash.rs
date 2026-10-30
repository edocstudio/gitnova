//! Explicit stash writes with immutable identity and recoverable reflog publication.
use crate::{merge::scratch::Scratch, working::run_stash_write, *};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::PathBuf,
};
const PATCH: usize = 512 * 1024;
// Preview text is bounded independently from the complete path/content identity.
fn append_patch(target: &mut String, value: &str) -> bool {
    let available = PATCH.saturating_sub(target.len());
    let mut end = value.len().min(available);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    target.push_str(&value[..end]);
    end < value.len()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn bytes(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AppError::new(
            "stash_blocked",
            "Symlinked stash metadata requires inspection with Git.",
        ));
    }
    let mut value = vec![];
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut value)?;
    if value.len() > limit {
        return Err(AppError::new(
            "stash_budget",
            "Stash metadata exceeds the complete review budget.",
        ));
    }
    Ok(value)
}
fn write(git: &GitCli, root: &Path, args: &[&str]) -> Result<()> {
    let _ = git;
    let mut native = vec![
        OsString::from("-c"),
        OsString::from("submodule.recurse=false"),
        OsString::from("-c"),
        OsString::from("fetch.recurseSubmodules=false"),
    ];
    native.extend(args.iter().map(OsString::from));
    run_stash_write(root, &native)
}
struct Metadata {
    tip: Vec<u8>,
    log: Vec<u8>,
    root: PathBuf,
}
impl GitCli {
    pub fn stashes(&self, root: &Path) -> Result<StashList> {
        if self
            .optional_text(root, &["rev-parse", "--verify", "--quiet", "refs/stash"])?
            .is_none()
        {
            return Ok(StashList {
                entries: vec![],
                more: false,
            });
        }
        let raw = self
            .read(
                root,
                &[
                    "stash".as_ref(),
                    "list".as_ref(),
                    "--no-show-signature".as_ref(),
                    "--no-decorate".as_ref(),
                    "-z".as_ref(),
                    "--format=%H%x00%gs%x00%at%x00%P".as_ref(),
                    "--max-count=201".as_ref(),
                ],
                1024 * 1024,
            )?
            .bytes;
        let fields = raw.split(|b| *b == 0).collect::<Vec<_>>();
        let fields = fields.strip_suffix(&[&[][..]]).unwrap_or(&fields);
        if !fields.len().is_multiple_of(4) {
            return Err(AppError::new("stash_parse", "Invalid stash listing."));
        }
        let mut entries = vec![];
        for (index, row) in fields.chunks(4).enumerate() {
            let oid = std::str::from_utf8(row[0])
                .map_err(|_| AppError::new("stash_parse", "Invalid stash identity."))?;
            validate_oid(oid)?;
            let timestamp = std::str::from_utf8(row[2])
                .ok()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| AppError::new("stash_parse", "Invalid stash date."))?;
            let parents = std::str::from_utf8(row[3])
                .map_err(|_| AppError::new("stash_parse", "Invalid stash parent."))?;
            let base_oid = parents.split_whitespace().next().map(str::to_owned);
            if let Some(base) = &base_oid {
                validate_oid(base)?;
            }
            entries.push(StashEntry {
                base_oid,
                index,
                oid: oid.into(),
                message: String::from_utf8_lossy(row[1]).into(),
                timestamp,
            });
        }
        let more = entries.len() > 200;
        entries.truncate(200);
        Ok(StashList { entries, more })
    }
    fn stash_metadata(&self, root: &Path) -> Result<Metadata> {
        let d = self.discover(root)?;
        if self
            .optional_text(root, &["config", "--get", "extensions.refStorage"])?
            .is_some_and(|s| s != "files")
        {
            return Err(AppError::new(
                "stash_blocked",
                "Pop/Drop currently require Git's files ref storage.",
            ));
        }
        for directory in ["refs", "logs", "logs/refs"] {
            let path = d.common_dir.join(directory);
            if fs::symlink_metadata(path)?.file_type().is_symlink() {
                return Err(AppError::new(
                    "stash_blocked",
                    "Symlinked stash metadata requires inspection with Git.",
                ));
            }
        }
        let tip=bytes(&d.common_dir.join("refs/stash"),256).map_err(|_|AppError::new("stash_blocked","Pop/Drop require a loose stash ref and complete local reflog. Inspect packed or missing metadata with Git."))?;
        let log = bytes(&d.common_dir.join("logs/refs/stash"), 2 * 1024 * 1024)?;
        let packed = d.common_dir.join("packed-refs");
        if packed.exists()
            && bytes(&packed, 8 * 1024 * 1024)?
                .split(|b| *b == b'\n')
                .any(|l| l.ends_with(b" refs/stash"))
        {
            return Err(AppError::new(
                "stash_blocked",
                "Packed stash refs require inspection with Git before Pop/Drop.",
            ));
        }
        let hook = self.text(
            root,
            &["rev-parse", "--git-path", "hooks/reference-transaction"],
        )?;
        let hook = PathBuf::from(hook.trim_end());
        let hook = if hook.is_absolute() {
            hook
        } else {
            root.join(hook)
        };
        if hook.exists() {
            return Err(AppError::new(
                "stash_blocked",
                "Reference-transaction hooks require native Git for Pop/Drop.",
            ));
        }
        let last = log
            .split(|b| *b == b'\n')
            .rfind(|line| !line.is_empty())
            .and_then(|line| line.split(|b| *b == b' ').nth(1));
        if last != Some(tip.trim_ascii()) {
            return Err(AppError::new(
                "stash_blocked",
                "Stash ref and reflog disagree. Inspect their recovery metadata with Git.",
            ));
        }
        Ok(Metadata {
            tip,
            log,
            root: d.common_dir,
        })
    }
    pub fn preview_stash(&self, root: &Path, request: &StashRequest) -> Result<StashPreview> {
        let mut request = request.clone();
        if request.id.is_empty() {
            request.id = uuid::Uuid::new_v4().to_string();
        }
        let id = uuid::Uuid::parse_str(&request.id)
            .map_err(|_| AppError::new("stash_request", "Invalid stash operation identity."))?;
        if request.message.len() > 1024 || request.message.contains(['\0', '\n', '\r']) {
            return Err(AppError::new(
                "stash_request",
                "Use a single-line stash message up to 1,024 UTF-8 bytes.",
            ));
        }
        let discovery = self.discover(root)?;
        let changes = self.working_changes(root)?;
        let list = self.stashes(root)?;
        let mut blocked = self.write_state(root).err().map(|e| e.message);
        if self
            .optional_text(root, &["symbolic-ref", "--quiet", "refs/stash"])?
            .is_some()
        {
            blocked =
                Some("Symbolic stash refs require inspection with Git before stash writes.".into());
        }
        if discovery.git_dir.join("index.lock").exists()
            || discovery.git_dir.join("HEAD.lock").exists()
        {
            blocked = Some(
                "Another Git operation holds an index or HEAD lock. Wait and review again.".into(),
            );
        }
        if request.kind == StashKind::Push
            && changes.files.iter().any(|f| {
                f.index_status == "D"
                    && f.path
                        .to_path()
                        .is_ok_and(|p| fs::symlink_metadata(root.join(p)).is_ok())
            })
        {
            blocked=Some("Removed index paths still contain local files. Preserve those files with Git before stashing.".into());
        }
        if changes.status.head.is_none() {
            blocked = Some("Create the first commit before using stashes.".into());
        }
        if changes.files.iter().any(|f| f.blocked_reason.is_some()) {
            blocked=Some("Resolve conflicts and review submodule/nested repository changes before using stash writes.".into());
        }
        let flags = self
            .read(
                root,
                &["ls-files".as_ref(), "-v".as_ref(), "-z".as_ref()],
                4 * 1024 * 1024,
            )?
            .bytes;
        if flags.split(|b| *b == 0).any(|r| {
            r.first()
                .is_some_and(|b| b.is_ascii_lowercase() || *b == b'S')
        }) {
            blocked = Some(
                "Hidden index entries require inspection with Git before using stash writes."
                    .into(),
            );
        }
        let mut files = BTreeMap::new();
        let mut diff = String::new();
        let mut index_diff = String::new();
        let mut truncated = false;
        let mut index_truncated = false;
        let mut metadata_hash = None;
        if request.kind == StashKind::Push {
            if request.index.is_some() || request.oid.is_some() {
                return Err(AppError::new(
                    "stash_request",
                    "Push captures working changes, not an existing stash.",
                ));
            }
            for file in &changes.files {
                if file.index_status != "?" || request.include_untracked {
                    files.insert(file.path.hex.clone(), file.path.clone());
                }
            }
            if files.is_empty() {
                blocked = Some("No changes match this stash scope.".into());
            }
            if let Some(head) = &changes.status.head {
                diff = self.stash_diff(
                    root,
                    &[
                        "diff",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--no-renames",
                        "--no-color",
                        "--submodule=short",
                        "--src-prefix=a/",
                        "--dst-prefix=b/",
                        head,
                        "--",
                    ],
                    &mut truncated,
                )?;
                index_diff = self.stash_diff(
                    root,
                    &[
                        "diff",
                        "--cached",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--no-renames",
                        "--no-color",
                        "--submodule=short",
                        "--src-prefix=a/",
                        "--dst-prefix=b/",
                        head,
                        "--",
                    ],
                    &mut index_truncated,
                )?;
                if request.include_untracked {
                    for (position, file) in changes
                        .files
                        .iter()
                        .filter(|f| f.index_status == "?")
                        .enumerate()
                    {
                        if position >= 16 || diff.len() >= PATCH {
                            truncated = true;
                            break;
                        }
                        let p = self.file_preview(root, &file.path, false)?;
                        truncated |= p.truncated;
                        truncated |= append_patch(&mut diff, &p.diff);
                    }
                }
            }
        } else {
            let index = request
                .index
                .ok_or_else(|| AppError::new("stash_request", "Choose an exact stash entry."))?;
            let entry = list
                .entries
                .get(index)
                .filter(|e| Some(&e.oid) == request.oid.as_ref())
                .ok_or_else(|| {
                    AppError::new(
                        "stale_preview",
                        "The stash list changed. Select the entry again.",
                    )
                })?;
            let detail = self.detail(root, &entry.oid)?;
            if !(2..=3).contains(&detail.commit.parents.len()) {
                return Err(AppError::new(
                    "stash_shape",
                    "The selected object is not a supported stash.",
                ));
            }
            let saved_index = self.detail(root, &detail.commit.parents[1])?;
            if saved_index.commit.parents != vec![detail.commit.parents[0].clone()] {
                return Err(AppError::new(
                    "stash_shape",
                    "The stash index parent has an unsupported shape.",
                ));
            }
            index_truncated = saved_index.truncated;
            index_truncated |= append_patch(&mut index_diff, &saved_index.diff);
            truncated = detail.truncated;
            truncated |= append_patch(&mut diff, &detail.diff);
            for file in saved_index.files.iter().chain(detail.files.iter()) {
                files.insert(file.path.hex.clone(), file.path.clone());
            }
            if let Some(untracked) = detail.commit.parents.get(2) {
                let third = self.detail(root, untracked)?;
                if !third.commit.parents.is_empty() {
                    return Err(AppError::new(
                        "stash_shape",
                        "The stash untracked parent has an unsupported shape.",
                    ));
                }
                truncated |= third.truncated;
                truncated |= append_patch(&mut diff, &third.diff);
                for file in third.files {
                    files.insert(file.path.hex.clone(), file.path);
                }
            }
            let mut versions = vec![
                (Some(detail.commit.parents[0].as_str()), entry.oid.as_str()),
                (
                    Some(detail.commit.parents[0].as_str()),
                    detail.commit.parents[1].as_str(),
                ),
            ];
            if let Some(third) = detail.commit.parents.get(2) {
                versions.push((None, third.as_str()));
            }
            for (base, tip) in versions {
                let mut args: Vec<&OsStr> = vec![
                    "diff-tree".as_ref(),
                    "--root".as_ref(),
                    "--no-commit-id".as_ref(),
                    "--no-ext-diff".as_ref(),
                    "--no-textconv".as_ref(),
                    "--no-renames".as_ref(),
                    "--raw".as_ref(),
                    "-r".as_ref(),
                    "-z".as_ref(),
                ];
                if let Some(base) = base {
                    args.push(base.as_ref());
                }
                args.push(OsStr::new(tip));
                args.push(OsStr::new("--"));
                let raw = self.read(root, &args, 4 * 1024 * 1024)?.bytes;
                if raw.split(|b| *b == 0).any(|r| {
                    r.starts_with(b":160000 ")
                        || r.starts_with(b":") && r.get(8..14) == Some(b"160000")
                }) {
                    blocked = Some(
                        "Stashes that change submodule pointers require separate review with Git."
                            .into(),
                    );
                }
            }
            if request.kind != StashKind::Drop && changes.status.dirty {
                blocked=Some("Stash or commit current work before applying saved work. The working tree must be clean.".into());
            }
            if matches!(request.kind, StashKind::Pop | StashKind::Drop) {
                match self.stash_metadata(root) {
                    Ok(m) => metadata_hash = Some(hash(&[m.tip, m.log].concat())),
                    Err(e) => blocked = Some(e.message),
                };
                if list.more {
                    blocked = Some(
                        "Pop/Drop require a complete stash list of at most 200 entries.".into(),
                    );
                }
            }
        }
        for file in files.values() {
            crate::working::relative(file)?;
        }
        if matches!(request.kind, StashKind::Apply | StashKind::Pop) {
            let tracked = self
                .read(root, &["ls-files".as_ref(), "-z".as_ref()], 4 * 1024 * 1024)?
                .bytes;
            let tracked = tracked
                .split(|b| *b == 0)
                .filter(|p| !p.is_empty())
                .map(|p| crate::path_from_bytes(p.to_vec()).map(|p| NativePath::new(&p).hex))
                .collect::<Result<std::collections::BTreeSet<_>>>()?;
            for file in files.values() {
                let relative = crate::working::relative(file)?;
                let mut parent = root.to_path_buf();
                for part in relative.parent().unwrap_or(Path::new("")).components() {
                    parent.push(part);
                    if fs::symlink_metadata(&parent)
                        .is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir())
                    {
                        blocked=Some("Saved paths have symlink or file-directory collisions. Preserve local contents with Git first.".into());
                    }
                }
                if let Ok(metadata) = fs::symlink_metadata(root.join(&relative)) {
                    if metadata.is_dir() || !tracked.contains(&file.hex) {
                        blocked=Some("Local files occupy saved paths. Preserve them before applying this stash.".into());
                    }
                }
            }
        }
        if files.len() > 20_000 {
            return Err(AppError::new(
                "stash_budget",
                "Stash file list exceeds 20,000 paths.",
            ));
        }
        let mut contents = vec![];
        if request.kind == StashKind::Push {
            for file in changes
                .files
                .iter()
                .filter(|f| f.index_status != "?" || request.include_untracked)
            {
                if file.blocked_reason.is_none() {
                    contents.push((
                        file.path.hex.clone(),
                        crate::working::file_digest(root, &file.path.to_path()?)?,
                    ));
                }
            }
        }
        let configuration = self
            .read(
                root,
                &["config".as_ref(), "--null".as_ref(), "--list".as_ref()],
                1024 * 1024,
            )?
            .bytes;
        let token = hash(
            &serde_json::to_vec(&(
                &request,
                &diff,
                &index_diff,
                &truncated,
                &index_truncated,
                &files,
                &contents,
                &changes.token,
                self.write_state(root).ok().map(|s| s.1),
                &configuration,
                &list,
                &metadata_hash,
            ))
            .map_err(|e| AppError::new("stash", e.to_string()))?,
        );
        Ok(StashPreview {
            version: 2,
            request,
            stash_snapshot: metadata_hash.clone(),
            files: files.into_values().collect(),
            diff,
            index_diff,
            truncated,
            index_truncated,
            branch: changes.status.branch,
            head: changes.status.head,
            recovery_ref: format!("refs/gitnova/recovery/stash-{}", id.simple()),
            recovery_dir: NativePath::new(
                &discovery
                    .common_dir
                    .join(format!("gitnova-stash-{}", id.simple())),
            ),
            blocked_reason: blocked,
            token,
        })
    }
    fn stash_diff(&self, root: &Path, args: &[&str], truncated: &mut bool) -> Result<String> {
        let output = self.execute(
            root,
            &args.iter().map(OsStr::new).collect::<Vec<_>>(),
            PATCH,
            true,
        )?;
        let mut text = String::new();
        *truncated |= output.truncated;
        *truncated |= append_patch(&mut text, &String::from_utf8_lossy(&output.bytes));
        Ok(text)
    }
    pub fn stash_patch(
        &self,
        root: &Path,
        request: &StashRequest,
        token: &str,
        path: &NativePath,
        index: bool,
    ) -> Result<HistoricalPatch> {
        let preview = self.preview_stash(root, request)?;
        if preview.token != token {
            return Err(AppError::new("stale_preview", "Working state, stash list or configuration changed. Review the stash operation again."));
        }
        if !preview.files.contains(path) {
            return Err(AppError::new(
                "stash_request",
                "Choose a file from this stash review.",
            ));
        }
        let relative = crate::working::relative(path)?;
        let mut patch = if request.kind == StashKind::Push {
            let changes = self.working_changes(root)?;
            let file = changes
                .files
                .iter()
                .find(|file| file.path == *path)
                .ok_or_else(|| {
                    AppError::new(
                        "stale_preview",
                        "Selected stash path changed. Review again.",
                    )
                })?;
            if file.index_status == "?" {
                if index {
                    HistoricalPatch {
                        diff: String::new(),
                        truncated: false,
                    }
                } else {
                    let p = self.file_preview(root, path, false)?;
                    HistoricalPatch {
                        diff: p.diff,
                        truncated: p.truncated,
                    }
                }
            } else {
                let head = preview.head.as_deref().ok_or_else(|| {
                    AppError::new(
                        "stash_request",
                        "Create the first commit before using stashes.",
                    )
                })?;
                let mut args: Vec<&OsStr> = vec![
                    "--literal-pathspecs".as_ref(),
                    "diff".as_ref(),
                    "--no-ext-diff".as_ref(),
                    "--no-textconv".as_ref(),
                    "--no-renames".as_ref(),
                    "--no-color".as_ref(),
                    "--submodule=short".as_ref(),
                    "--src-prefix=a/".as_ref(),
                    "--dst-prefix=b/".as_ref(),
                ];
                if index {
                    args.push("--cached".as_ref());
                }
                args.extend([head.as_ref(), "--".as_ref(), relative.as_os_str()]);
                let output = self.execute(root, &args, PATCH, true)?;
                HistoricalPatch {
                    diff: String::from_utf8_lossy(&output.bytes).into_owned(),
                    truncated: output.truncated,
                }
            }
        } else {
            let oid = request
                .oid
                .as_deref()
                .ok_or_else(|| AppError::new("stash_request", "Missing stash identity."))?;
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
            let parents = parents.split_whitespace().collect::<Vec<_>>();
            if index {
                let parent = parents.get(1).ok_or_else(|| {
                    AppError::new(
                        "stash_shape",
                        "The selected object is not a supported stash.",
                    )
                })?;
                self.historical_patch(root, parent, path)?
            } else {
                let mut result = self.historical_patch(root, oid, path)?;
                if let Some(untracked) = parents.get(2) {
                    let third = self.historical_patch(root, untracked, path)?;
                    result.truncated |= third.truncated;
                    result.truncated |= append_patch(&mut result.diff, &third.diff);
                }
                result
            }
        };
        let mut bounded = String::new();
        patch.truncated |= append_patch(&mut bounded, &patch.diff);
        patch.diff = bounded;
        // Current files can change during a read; never label another snapshot's
        // selected patch as belonging to the reviewed token.
        if self.preview_stash(root, request)?.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Selected stash path changed. Review again.",
            ));
        }
        Ok(patch)
    }
    fn pin_stash(&self, root: &Path, reference: &str, oid: &str) -> Result<()> {
        write(
            self,
            root,
            &["update-ref", "--no-deref", reference, oid, ""],
        )
    }
    pub fn apply_stash(&self, root: &Path, request: &StashRequest, token: &str) -> Result<()> {
        let preview = self.preview_stash(root, request)?;
        if preview.token != token {
            return Err(AppError::new("stale_preview","Working state, stash list or configuration changed. Review the stash operation again."));
        }
        if let Some(reason) = preview.blocked_reason {
            return Err(AppError::new("stash_blocked", reason));
        }
        if request.kind == StashKind::Push {
            let label = format!(
                "{} [GitNova {}]",
                if request.message.is_empty() {
                    "Saved work"
                } else {
                    &request.message
                },
                request.id
            );
            let mut args = vec!["stash", "push", "--message", &label];
            if request.include_untracked {
                args.push("--include-untracked");
            }
            args.push("--");
            let outcome = write(self, root, &args);
            let entries = self.stashes(root)?;
            if let Some(created) = entries
                .entries
                .iter()
                .find(|e| e.message.ends_with(&format!("[GitNova {}]", request.id)))
            {
                self.pin_stash(root, &preview.recovery_ref, &created.oid)?;
            } else {
                return Err(AppError::new("stash_partial","The stash creation could not be verified. Inspect saved entries and local files before retrying."));
            }
            outcome?;
            if self.status(root)?.head != preview.head {
                return Err(AppError::new(
                    "stash_partial",
                    "HEAD changed during stash creation. Inspect the saved work before continuing.",
                ));
            }
            let remaining = self.working_changes(root)?;
            if remaining
                .files
                .iter()
                .any(|f| request.include_untracked || f.index_status != "?")
            {
                return Err(AppError::new("stash_partial", "Saved work is retained, but local changes remain after stashing. Inspect them before continuing."));
            }
            return Ok(());
        }
        let oid = request
            .oid
            .as_deref()
            .ok_or_else(|| AppError::new("stash_request", "Missing stash identity."))?;
        let removal = if matches!(request.kind, StashKind::Pop | StashKind::Drop) {
            let metadata = self.stash_metadata(root)?;
            if Some(hash(&[metadata.tip.clone(), metadata.log.clone()].concat()))
                != preview.stash_snapshot
            {
                return Err(AppError::new(
                    "stale_preview",
                    "Stash metadata changed before applying. Review again.",
                ));
            }
            Some(metadata)
        } else {
            None
        };
        self.pin_stash(root, &preview.recovery_ref, oid)?;
        if request.kind != StashKind::Drop {
            let mut args = vec!["stash", "apply"];
            if request.restore_index {
                args.push("--index");
            }
            args.push(oid);
            if let Err(error) = write(self, root, &args) {
                let conflicts = self
                    .read(
                        root,
                        &["ls-files".as_ref(), "--unmerged".as_ref(), "-z".as_ref()],
                        4 * 1024 * 1024,
                    )?
                    .bytes;
                return Err(AppError::new(if conflicts.is_empty(){"stash_partial"}else{"stash_conflict"},format!("Stash retained. Application may have changed files or the index. Inspect Working changes and resolve with Git before retrying. {}",error.message)));
            }
            if self.status(root)?.head != preview.head {
                return Err(AppError::new("stash_partial","HEAD changed while applying the stash. The stash was retained; inspect the result."));
            }
        }
        if let Some(metadata) = removal {
            self.remove_stash_record(root, &preview, metadata)?;
        }
        Ok(())
    }
    fn remove_stash_record(
        &self,
        root: &Path,
        preview: &StashPreview,
        before: Metadata,
    ) -> Result<()> {
        let index = preview
            .request
            .index
            .ok_or_else(|| AppError::new("stash_request", "Missing stash position."))?;
        let listing = self.stashes(root)?;
        if listing.more
            || listing
                .entries
                .get(index)
                .is_none_or(|e| Some(&e.oid) != preview.request.oid.as_ref())
        {
            return Err(AppError::new("stash_partial","Saved work was retained because the stash list changed before removal. Do not apply it again without inspecting local files."));
        }
        let scratch = Scratch::new(self, root)?;
        let (tip, log) = scratch.remove_stash_record(&before.tip, &before.log, index)?;
        let archive = preview.recovery_dir.to_path()?;
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&archive)?;
        for (name, content) in [
            ("original-ref", &before.tip),
            ("original-reflog", &before.log),
            ("proposed-reflog", &log),
        ] {
            let mut file = fs::File::create(archive.join(name))?;
            file.write_all(content)?;
            file.sync_all()?;
        }
        if let Some(tip) = &tip {
            let mut file = fs::File::create(archive.join("proposed-ref"))?;
            file.write_all(tip)?;
            file.sync_all()?;
        }
        #[cfg(unix)]
        fs::File::open(&archive)?.sync_all()?;
        let _packed_lock = MetadataLock::new(before.root.join("packed-refs.lock"))?;
        let mut ref_lock = MetadataLock::new(before.root.join("refs/stash.lock"))?;
        let mut log_lock = MetadataLock::new(before.root.join("logs/refs/stash.lock"))?;
        let now = self.stash_metadata(root)?;
        if now.tip != before.tip || now.log != before.log {
            return Err(AppError::new("stash_partial","Stash metadata changed before removal. The stash and recovery snapshot were retained."));
        }
        ref_lock.preserve_permissions(&before.root.join("refs/stash"))?;
        log_lock.preserve_permissions(&before.root.join("logs/refs/stash"))?;
        ref_lock.prepare(tip.as_deref().unwrap_or(&[]))?;
        log_lock.prepare(&log)?;
        log_lock.publish(&before.root.join("logs/refs/stash"))?;
        if tip.is_some() {
            ref_lock.publish(&before.root.join("refs/stash"))?;
        } else {
            fs::remove_file(before.root.join("refs/stash"))?;
        }
        Ok(())
    }
}
struct MetadataLock {
    path: PathBuf,
    file: Option<fs::File>,
    owned: bool,
}
impl MetadataLock {
    fn new(path: PathBuf) -> Result<Self> {
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| {
                AppError::new(
                    "stash_locked",
                    format!("Stash metadata is locked or unavailable: {e}"),
                )
            })?;
        Ok(Self {
            path,
            file: Some(file),
            owned: true,
        })
    }
    fn preserve_permissions(&self, original: &Path) -> Result<()> {
        self.file
            .as_ref()
            .ok_or_else(|| AppError::new("stash_locked", "Missing metadata lock."))?
            .set_permissions(fs::metadata(original)?.permissions())?;
        Ok(())
    }
    fn prepare(&mut self, bytes: &[u8]) -> Result<()> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| AppError::new("stash_locked", "Missing metadata lock."))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    }
    fn publish(&mut self, target: &Path) -> Result<()> {
        self.file.take();
        fs::rename(&self.path, target)?;
        self.owned = false;
        Ok(())
    }
}
impl Drop for MetadataLock {
    fn drop(&mut self) {
        self.file.take();
        if self.owned {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn published_metadata_lock_never_removes_a_later_owners_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ref.lock");
        let mut lock = MetadataLock::new(path.clone()).unwrap();
        lock.prepare(b"reviewed").unwrap();
        lock.publish(&dir.path().join("ref")).unwrap();
        fs::write(&path, b"another writer").unwrap();
        drop(lock);
        assert_eq!(fs::read(&path).unwrap(), b"another writer");
    }
}
