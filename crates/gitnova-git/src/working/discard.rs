//! Exact leaf-path discard with pre-write raw recovery, never directory cleanup.
use super::*;
use crate::progress::emit;
use std::collections::{BTreeMap, BTreeSet};
type StreamIdentity = (u64, String);
type CapturedFile = (String, u32, Vec<u8>, Option<StreamIdentity>);
struct SavedFile {
    path: NativePath,
    kind: String,
    mode: u32,
    bytes: Vec<u8>,
    large: Option<StreamIdentity>,
    index_entry: Vec<u8>,
    staged_blob: Vec<u8>,
}
fn blocked(message: &str) -> AppError {
    AppError::new("discard_blocked", message)
}
fn stream_file(source: &Path, mut output: Option<&mut File>) -> Result<(u64, String)> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut input = options.open(source)?;
    let before = input.metadata()?;
    if !before.is_file() || before.len() > 256 * 1024 * 1024 {
        return Err(blocked("A working file exceeds the complete 256 MiB discard budget. Preserve it externally before discarding."));
    }
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    let started = Instant::now();
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > before.len() {
            return Err(blocked(
                "Files changed during discard. Inspect saved recovery before retrying.",
            ));
        }
        if total > 256 * 1024 * 1024 || started.elapsed() > Duration::from_secs(20) {
            return Err(blocked("A working file exceeds the complete 256 MiB discard budget. Preserve it externally before discarding."));
        }
        hash.update(&buffer[..count]);
        if let Some(file) = output.as_deref_mut() {
            file.write_all(&buffer[..count])?;
        }
        if output.is_some() {
            emit("backing-up-file", total, before.len(), "bytes");
        }
    }
    let after = input.metadata()?;
    if total != before.len()
        || after.len() != before.len()
        || after.modified()? != before.modified()?
    {
        return Err(blocked(
            "Files changed during discard. Inspect saved recovery before retrying.",
        ));
    }
    Ok((total, format!("{:x}", hash.finalize())))
}
fn capture(root: &Path, path: &NativePath, allow_large: bool) -> Result<CapturedFile> {
    let relative = relative(path)?;
    let mut parent = root.to_path_buf();
    for part in relative.parent().unwrap_or(Path::new("")).components() {
        parent.push(part);
        if fs::symlink_metadata(&parent).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(blocked("Cannot write through a symbolic-link directory"));
        }
    }
    let absolute = root.join(relative);
    let meta = match fs::symlink_metadata(&absolute) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(("missing".into(), 0, vec![], None))
        }
        Err(e) => return Err(e.into()),
    };
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode()
    };
    #[cfg(not(unix))]
    let mode = if meta.permissions().readonly() {
        0o444
    } else {
        0o644
    };
    if meta.file_type().is_symlink() {
        let target = NativePath::new(&fs::read_link(absolute)?);
        Ok((
            "symlink".into(),
            mode,
            serde_json::to_vec(&target).map_err(|e| blocked(&e.to_string()))?,
            None,
        ))
    } else if meta.is_file() {
        if allow_large && meta.len() > FILE_LIMIT {
            Ok((
                "file".into(),
                mode,
                vec![],
                Some(stream_file(&absolute, None)?),
            ))
        } else {
            Ok((
                "file".into(),
                mode,
                read_bounded(&absolute, FILE_LIMIT)?,
                None,
            ))
        }
    } else {
        Err(blocked("Directories and submodules are not discarded here. Use Restore recorded version for a submodule."))
    }
}
fn work_hash(item: &SavedFile) -> String {
    item.large
        .as_ref()
        .map(|(_, hash)| hash.clone())
        .unwrap_or_else(|| digest(&item.bytes))
}
fn identity(path: &Path) -> Result<String> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err(blocked(
            "Repository folder changed. Reopen it before discarding.",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("{}:{}", m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        Ok(format!("{:?}", m.created()?))
    }
}
fn discard_watch(root: &Path, d: &crate::Discovery, branch: Option<&str>) -> Result<String> {
    let mut hash = Sha256::new();
    let mut paths = vec![
        root.join(".git"),
        d.git_dir.join("commondir"),
        d.common_dir.join("config"),
        d.git_dir.join("config.worktree"),
    ];
    for name in [
        "index",
        "HEAD",
        "gitnova-merge.json",
        "MERGE_HEAD",
        "ORIG_HEAD",
        "MERGE_MSG",
        "MERGE_MODE",
        "AUTO_MERGE",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "MERGE_AUTOSTASH",
        "rebase-apply",
        "rebase-merge",
        "sequencer",
    ] {
        paths.push(d.git_dir.join(name));
    }
    if let Some(branch) = branch {
        paths.push(d.common_dir.join(format!("refs/heads/{branch}")));
    }
    paths.push(d.common_dir.join("packed-refs"));
    for path in paths {
        hash.update(NativePath::new(&path).hex.as_bytes());
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => hash.update(b"directory"),
            Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
                hash.update(b"file");
                hash.update(read_bounded(&path, INDEX_LIMIT)?);
            }
            Ok(_) => {
                return Err(blocked(
                    "Repository changed during discard. Inspect saved recovery before retrying.",
                ))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => hash.update(b"missing"),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
fn private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
// Never remove a lock that was recreated by a later owner.
struct DiscardLock {
    path: PathBuf,
    file: Option<File>,
    marker: Vec<u8>,
}
impl DiscardLock {
    fn acquire(path: PathBuf) -> Result<Self> {
        let marker = format!("GitNova discard {}\n", uuid::Uuid::new_v4()).into_bytes();
        let mut file=OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e|if e.kind()==std::io::ErrorKind::AlreadyExists {blocked("A native Git lock exists. Wait for its owner; locks are never removed automatically.")} else {e.into()})?;
        file.write_all(&marker)?;
        file.sync_all()?;
        Ok(Self {
            path,
            file: Some(file),
            marker,
        })
    }
}
impl Drop for DiscardLock {
    fn drop(&mut self) {
        self.file.take();
        if fs::read(&self.path).is_ok_and(|bytes| bytes == self.marker) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
impl GitCli {
    fn discard_state(
        &self,
        root: &Path,
        operation: Option<&str>,
    ) -> Result<(crate::Discovery, String)> {
        if let Some(token) = operation {
            let state = self.merge_state(root)?;
            let d = self.discover(root)?;
            if state.phase != "pending"
                || state.token != token
                || !state.conflicts.is_empty()
                || state.blocked_reason.is_some()
                || [
                    "rebase-apply",
                    "rebase-merge",
                    "sequencer",
                    "MERGE_AUTOSTASH",
                ]
                .iter()
                .any(|name| d.git_dir.join(name).exists())
            {
                return Err(blocked("Discard requires the unchanged owned operation with every conflict resolved. Review the current operation again."));
            }
            Ok((d, state.token))
        } else {
            self.write_state(root)
        }
    }
    fn discard_attributes(&self, root: &Path, request: &DiscardRequest) -> Result<Vec<u8>> {
        let paths = request
            .paths
            .iter()
            .map(NativePath::to_path)
            .collect::<Result<Vec<_>>>()?;
        let mut args = vec![
            OsStr::new("check-attr"),
            OsStr::new("--all"),
            OsStr::new("-z"),
            OsStr::new("--"),
        ];
        args.extend(paths.iter().map(|p| p.as_os_str()));
        Ok(self.read(root, &args, 4 * 1024 * 1024)?.bytes)
    }

    fn discard_snapshot(
        &self,
        root: &Path,
        request: &DiscardRequest,
        check_locks: bool,
        operation: Option<&str>,
        phase: &str,
    ) -> Result<(DiscardPreview, Vec<SavedFile>)> {
        let uuid = uuid::Uuid::parse_str(&request.id)
            .map_err(|_| blocked("Review the discard selection before confirming."))?;
        if request.paths.is_empty() || request.paths.len() > 2000 {
            return Err(blocked(
                "Select between 1 and 2,000 changed files to discard.",
            ));
        }
        emit("preparing", 0, 0, "files");
        let (d, state) = self.discard_state(root, operation)?;
        let changes = self.working_changes(root)?;
        let mut seen = BTreeSet::new();
        let mut saved = vec![];
        let mut files = vec![];
        let mut reason = None;
        let mut total = index_bytes(&d.git_dir)?.len();
        let mut disk_total = 0u64;
        if changes.files.iter().any(|f| {
            f.index_status == "U"
                || f.worktree_status == "U"
                || matches!(
                    (f.index_status.as_str(), f.worktree_status.as_str()),
                    ("A", "A") | ("D", "D")
                )
        }) {
            reason = Some("Resolve existing conflicts before discarding ordinary files.".into());
        }
        emit(phase, 0, request.paths.len() as u64, "files");
        let selected_paths = request
            .paths
            .iter()
            .map(relative)
            .collect::<Result<Vec<_>>>()?;
        let mut args = vec![
            OsStr::new("ls-files"),
            OsStr::new("-v"),
            OsStr::new("-z"),
            OsStr::new("--"),
        ];
        args.extend(selected_paths.iter().map(|p| p.as_os_str()));
        let flags = self.read(root, &args, 4 * 1024 * 1024)?.bytes;
        let hidden = flags
            .split(|b| *b == 0)
            .filter(|r| r.len() > 2 && (r[0].is_ascii_lowercase() || r[0] == b'S'))
            .map(|r| path_from_bytes(r[2..].to_vec()).map(|p| NativePath::new(&p).hex))
            .collect::<Result<BTreeSet<_>>>()?;
        let mut args = vec![
            OsStr::new("ls-files"),
            OsStr::new("--stage"),
            OsStr::new("-z"),
            OsStr::new("--"),
        ];
        args.extend(selected_paths.iter().map(|p| p.as_os_str()));
        let raw = self.read(root, &args, 4 * 1024 * 1024)?.bytes;
        let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for row in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let tab = row
                .iter()
                .position(|b| *b == b'\t')
                .ok_or_else(|| blocked("Invalid index entry"))?;
            let key = NativePath::new(&path_from_bytes(row[tab + 1..].to_vec())?).hex;
            let value = entries.entry(key).or_default();
            value.extend(row);
            value.push(0);
        }
        for (position, path) in request.paths.iter().enumerate() {
            relative(path)?;
            if !seen.insert(path.hex.clone()) {
                return Err(blocked("Select each changed file only once."));
            }
            let file = changes
                .files
                .iter()
                .find(|f| {
                    f.path.hex == path.hex
                        && f.path.encoding == path.encoding
                        && (request.scope == DiscardScope::All || f.unstaged)
                })
                .ok_or_else(|| {
                    AppError::new(
                        "stale_preview",
                        "Discard selection changed. Review the current files again.",
                    )
                })?
                .clone();
            if let Some(why) = &file.blocked_reason {
                reason = Some(why.clone());
                files.push(file);
                continue;
            }
            if hidden.contains(&path.hex) {
                reason = Some(
                    "Hidden or sparse index entries need Git inspection before discard.".into(),
                );
            }
            let raw = entries.get(&path.hex).cloned().unwrap_or_default();
            let records: Vec<_> = raw.split(|b| *b == 0).filter(|s| !s.is_empty()).collect();
            if records.len() > 1 || records.first().is_some_and(|s| s.starts_with(b"160000 ")) {
                reason = Some(
                    "Submodule pointers and conflicts require their own reviewed workflow.".into(),
                );
                files.push(file);
                continue;
            }
            let staged_blob = if let Some(record) = records.first() {
                let fields: Vec<_> = record.split(|b| *b == b' ').collect();
                let oid = String::from_utf8_lossy(
                    fields
                        .get(1)
                        .ok_or_else(|| blocked("Invalid index entry"))?,
                )
                .to_string();
                crate::validate_oid(&oid)?;
                self.read(
                    root,
                    &["cat-file".as_ref(), "blob".as_ref(), oid.as_ref()],
                    FILE_LIMIT as usize,
                )?
                .bytes
            } else {
                vec![]
            };
            let estimate = fs::symlink_metadata(root.join(path.to_path()?)).map_or(0, |m| {
                if m.is_file() {
                    m.len()
                } else {
                    0
                }
            });
            if disk_total.saturating_add(estimate) > 2 * 1024 * 1024 * 1024 {
                return Err(blocked("Discard recovery exceeds its complete 2 GiB streaming budget. Select fewer files."));
            }
            let (kind, mode, bytes, large) = capture(root, path, operation.is_some())?;
            disk_total += large.as_ref().map_or(bytes.len() as u64, |v| v.0);
            total += bytes.len() + staged_blob.len();
            if total > 128 * 1024 * 1024 {
                return Err(blocked(
                    "Discard recovery exceeds its complete size budget. Select fewer files.",
                ));
            }
            saved.push(SavedFile {
                path: path.clone(),
                kind,
                mode,
                bytes,
                large,
                index_entry: raw,
                staged_blob,
            });
            files.push(file);
            emit(
                phase,
                (position + 1) as u64,
                request.paths.len() as u64,
                "files",
            );
        }
        if saved.iter().map(|s| s.bytes.len()).sum::<usize>() > 64 * 1024 * 1024 {
            return Err(blocked(
                "Discard recovery exceeds its complete size budget. Select fewer files.",
            ));
        }
        if saved
            .iter()
            .map(|s| s.large.as_ref().map_or(s.bytes.len() as u64, |v| v.0))
            .sum::<u64>()
            > 2 * 1024 * 1024 * 1024
        {
            return Err(blocked(
                "Discard recovery exceeds its complete 2 GiB streaming budget. Select fewer files.",
            ));
        }
        let recovery = d
            .common_dir
            .join("gitnova-discard-recovery")
            .join(uuid.to_string());
        if recovery.exists()
            || fs::symlink_metadata(d.common_dir.join("gitnova-discard-recovery"))
                .is_ok_and(|m| !m.is_dir() || m.file_type().is_symlink())
        {
            reason = Some(
                "Recovery location already exists or needs inspection. Start a new discard review."
                    .into(),
            );
        }
        for lock in [d.git_dir.join("index.lock"), d.git_dir.join("HEAD.lock")] {
            if check_locks && lock.exists() {
                reason=Some("A native Git lock exists. Wait for its owner; locks are never removed automatically.".into());
            }
        }
        // Hash raw content once; never expand recovery bytes into JSON number arrays.
        let identities: Vec<_> = saved
            .iter()
            .map(|s| {
                (
                    &s.path,
                    &s.kind,
                    s.mode,
                    work_hash(s),
                    &s.index_entry,
                    digest(&s.staged_blob),
                )
            })
            .collect();
        let token = digest(
            &serde_json::to_vec(&(
                request,
                &state,
                &identities,
                self.config_snapshot(root)?,
                self.discard_attributes(root, request)?,
                identity(root)?,
                identity(&d.git_dir)?,
                &reason,
            ))
            .map_err(|e| blocked(&e.to_string()))?,
        );
        Ok((
            DiscardPreview {
                version: 1,
                source: NativePath::new(d.root.as_ref().ok_or_else(|| {
                    blocked("Repository folder changed. Reopen it before discarding.")
                })?),
                git_dir: NativePath::new(&d.git_dir),
                request: request.clone(),
                branch: changes.status.branch,
                head: changes.status.head,
                files,
                recovery_dir: NativePath::new(&recovery),
                token,
                blocked_reason: reason,
            },
            saved,
        ))
    }
    pub fn preview_discard(&self, root: &Path, request: &DiscardRequest) -> Result<DiscardPreview> {
        Ok(self
            .discard_snapshot(root, request, true, None, "checking")?
            .0)
    }
    pub fn preview_operation_discard(
        &self,
        root: &Path,
        request: &DiscardRequest,
        operation: &str,
    ) -> Result<DiscardPreview> {
        if self.merge_state(root)?.operation == "rebase" {
            return Err(AppError::new(
                "rebase",
                "Use the owned rebase resolution or abort review instead of Discard.",
            ));
        }
        Ok(self
            .discard_snapshot(root, request, true, Some(operation), "checking")?
            .0)
    }
    pub fn discard_operation_changes(
        &self,
        root: &Path,
        request: &DiscardRequest,
        token: &str,
        operation: &str,
    ) -> Result<()> {
        self.discard_context(root, request, token, Some(operation))
    }
    pub fn discard_changes(
        &self,
        root: &Path,
        request: &DiscardRequest,
        token: &str,
    ) -> Result<()> {
        self.discard_context(root, request, token, None)
    }
    fn discard_context(
        &self,
        root: &Path,
        request: &DiscardRequest,
        token: &str,
        operation: Option<&str>,
    ) -> Result<()> {
        let (p, saved) = self.discard_snapshot(root, request, true, operation, "validating")?;
        if p.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Discard selection changed. Review the current files again.",
            ));
        }
        if let Some(reason) = p.blocked_reason {
            return Err(blocked(&reason));
        }
        let (d, state) = self.discard_state(root, operation)?;
        let mut lock = DiscardLock::acquire(d.git_dir.join("index.lock"))?;
        let _head = DiscardLock::acquire(d.git_dir.join("HEAD.lock"))?;
        let _branch = if let Some(branch) = &p.branch {
            let path = d.common_dir.join(format!("refs/heads/{branch}.lock"));
            fs::create_dir_all(path.parent().unwrap())?;
            Some(DiscardLock::acquire(path)?)
        } else {
            None
        };
        if self
            .discard_snapshot(root, request, false, operation, "rechecking")?
            .0
            .token
            != token
        {
            return Err(AppError::new(
                "stale_preview",
                "Discard selection changed. Review the current files again.",
            ));
        }
        let mut head_paths = BTreeSet::new();
        if let Some(head) = &p.head {
            let owned = request
                .paths
                .iter()
                .map(relative)
                .collect::<Result<Vec<_>>>()?;
            let mut args = vec![
                OsStr::new("ls-tree"),
                OsStr::new("-r"),
                OsStr::new("-z"),
                OsStr::new(head),
                OsStr::new("--"),
            ];
            args.extend(owned.iter().map(|p| p.as_os_str()));
            let raw = self.read(root, &args, 4 * 1024 * 1024)?.bytes;
            for row in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
                let tab = row
                    .iter()
                    .position(|b| *b == b'\t')
                    .ok_or_else(|| blocked("Invalid tree entry"))?;
                head_paths.insert(NativePath::new(&path_from_bytes(row[tab + 1..].to_vec())?).hex);
            }
        }
        let only_untracked = saved
            .iter()
            .all(|s| s.index_entry.is_empty() && !head_paths.contains(&s.path.hex));
        let watch = discard_watch(root, &d, p.branch.as_deref())?;
        let configuration = self.config_snapshot(root)?;
        let attributes = self.discard_attributes(root, request)?;
        let original = index_bytes(&d.git_dir)?;
        let temp = tempfile::NamedTempFile::new_in(&d.git_dir)?.into_temp_path();
        if original.is_empty() {
            fs::remove_file(&temp)?;
        } else {
            fs::write(&temp, &original)?;
            preserve_index_timestamp(&d.git_dir.join("index"), &temp)?;
        }
        let recovery = p.recovery_dir.to_path()?;
        let parent = recovery.parent().unwrap();
        if !parent.exists() {
            private_dir(parent)?;
        }
        if fs::symlink_metadata(parent)?.file_type().is_symlink() {
            return Err(blocked(
                "Recovery location already exists or needs inspection. Start a new discard review.",
            ));
        }
        private_dir(&recovery)?;
        private_file(&recovery.join("index"), &original)?;
        let mut metadata = vec![];
        emit("backing-up", 0, saved.len() as u64, "files");
        for (i, item) in saved.iter().enumerate() {
            let work = recovery.join(format!("work-{i}"));
            if let Some(expected) = &item.large {
                private_file(&work, &[])?;
                let mut output = OpenOptions::new().write(true).open(&work)?;
                if &stream_file(&root.join(item.path.to_path()?), Some(&mut output))? != expected {
                    return Err(blocked(
                        "Files changed during discard. Inspect saved recovery before retrying.",
                    ));
                }
                output.sync_all()?;
            } else {
                private_file(&work, &item.bytes)?;
            }

            private_file(&recovery.join(format!("staged-{i}")), &item.staged_blob)?;
            emit("backing-up", (i + 1) as u64, saved.len() as u64, "files");
            metadata.push(serde_json::json!({"path":item.path,"kind":item.kind,"mode":item.mode,"workFile":format!("work-{i}"),"stagedFile":format!("staged-{i}"),"indexEntry":item.index_entry,"workHash":work_hash(item),"stagedHash":digest(&item.staged_blob)}));
        }
        private_file(
            &recovery.join("files.json"),
            &serde_json::to_vec(&metadata).map_err(|e| blocked(&e.to_string()))?,
        )?;
        private_file(
            &recovery.join("manifest.json"),
            &serde_json::to_vec(&p).map_err(|e| blocked(&e.to_string()))?,
        )?;
        #[cfg(unix)]
        {
            File::open(&recovery)?.sync_all()?;
            File::open(parent)?.sync_all()?;
        }
        let folder = identity(root)?;
        let admin = identity(&d.git_dir)?;
        let mut started = false;
        let mut needs_index_publication = !original.is_empty();
        let result = (|| -> Result<()> {
            emit("discarding", 0, saved.len() as u64, "files");
            for (position, item) in saved.iter().enumerate() {
                if !started && self.discard_attributes(root, request)? != attributes {
                    return Err(blocked(
                        "Files changed during discard. Inspect saved recovery before retrying.",
                    ));
                }
                if (only_untracked && discard_watch(root, &d, p.branch.as_deref())? != watch)
                    || (!only_untracked || !started)
                        && (self.discard_state(root, operation)?.1 != state
                            || self.config_snapshot(root)? != configuration)
                    || identity(root)? != folder
                    || identity(&d.git_dir)? != admin
                {
                    return Err(blocked("Repository changed during discard. Inspect saved recovery before retrying."));
                }
                let (kind, mode, bytes, large) = capture(root, &item.path, operation.is_some())?;
                if kind != item.kind
                    || mode != item.mode
                    || bytes != item.bytes
                    || large != item.large
                {
                    return Err(blocked(
                        "Files changed during discard. Inspect saved recovery before retrying.",
                    ));
                }
                let path = item.path.to_path()?;
                let head_exists = head_paths.contains(&item.path.hex);
                let indexed = !item.index_entry.is_empty();
                started = true;
                if request.scope == DiscardScope::Unstaged && indexed
                    || request.scope == DiscardScope::All
                        && p.head.is_some()
                        && (indexed || head_exists)
                {
                    let mut args: Vec<OsString> = vec![
                        "-c".into(),
                        "submodule.recurse=false".into(),
                        "restore".into(),
                        "--worktree".into(),
                    ];
                    if request.scope == DiscardScope::All {
                        args.extend(["--source=HEAD".into(), "--staged".into()]);
                    }
                    args.extend(["--".into(), path.as_os_str().into()]);
                    if request.scope == DiscardScope::All {
                        needs_index_publication = true;
                    }
                    run_git_write(root, &args, Some(temp.as_ref()), Some(""), None)?;
                } else {
                    if request.scope == DiscardScope::All && indexed {
                        needs_index_publication = true;
                        run_git_write(
                            root,
                            &[
                                "rm".into(),
                                "--cached".into(),
                                "--force".into(),
                                "--".into(),
                                path.as_os_str().into(),
                            ],
                            Some(temp.as_ref()),
                            Some(""),
                            None,
                        )?;
                    }
                    // Only the exact previously captured leaf; never recursive clean/remove_dir.
                    match fs::remove_file(root.join(&path)) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(e.into()),
                    }
                }
                emit(
                    "discarding",
                    (position + 1) as u64,
                    saved.len() as u64,
                    "files",
                );
            }
            emit("finalizing", 0, 0, "files");
            if self.discard_state(root, operation)?.1 != state
                || self.config_snapshot(root)? != configuration
            {
                return Err(blocked(
                    "Repository changed during discard. Inspect saved recovery before retrying.",
                ));
            }
            if request.scope == DiscardScope::All && needs_index_publication {
                let new_index = read_bounded(&temp, INDEX_LIMIT)?;
                if !fs::read(&lock.path).is_ok_and(|bytes| bytes == lock.marker) {
                    return Err(blocked("Repository changed during discard. Inspect saved recovery before retrying."));
                }
                let handle = lock
                    .file
                    .as_mut()
                    .ok_or_else(|| blocked("Index lock unavailable"))?;
                if let Ok(m) = fs::metadata(d.git_dir.join("index")) {
                    handle.set_permissions(m.permissions())?;
                }
                handle.set_len(0)?;
                handle.rewind()?;
                handle.write_all(&new_index)?;
                handle.set_times(
                    fs::FileTimes::new().set_modified(fs::metadata(&temp)?.modified()?),
                )?;
                handle.sync_all()?;
                lock.file.take();
                fs::rename(&lock.path, d.git_dir.join("index"))?;
                lock.path = PathBuf::new();
            }
            Ok(())
        })();
        result.map_err(|e| if started {AppError::new("operation_unknown",format!("Discard may have applied partially. Original files and staging are saved for recovery: {}",e.message))} else {e})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_byte_copy_reports_exact_bytes_and_retains_content() {
        use std::sync::{Arc, Mutex};
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let content = vec![7u8; 65536 * 3 + 123];
        fs::write(&source, &content).unwrap();
        let mut output = File::create(&target).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let collected = events.clone();
        let identity = crate::with_progress(
            move |event| collected.lock().unwrap().push(event),
            || stream_file(&source, Some(&mut output)),
        )
        .unwrap();
        assert_eq!(identity.0, content.len() as u64);
        assert_eq!(fs::read(target).unwrap(), content);
        let events = events.lock().unwrap();
        let last = events.last().unwrap();
        assert_eq!(last.phase, "backing-up-file");
        assert_eq!(last.unit, "bytes");
        assert_eq!(last.completed, identity.0);
        assert_eq!(last.total, identity.0);
        assert!(events.windows(2).all(|e| e[0].completed < e[1].completed));
    }
    #[test]
    fn discard_lock_preserves_replacement_owner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.lock");
        let guard = DiscardLock::acquire(path.clone()).unwrap();
        fs::remove_file(&path).unwrap();
        fs::write(&path, "external owner").unwrap();
        drop(guard);
        assert_eq!(fs::read_to_string(path).unwrap(), "external owner");
    }
}
