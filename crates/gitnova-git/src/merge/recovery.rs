//! Raw filesystem/index snapshots, including staged blobs, before abort/replacement.
use super::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    path: NativePath,
    kind: String,
    mode: u32,
    file: Option<String>,
    hash: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    token: String,
    entries: Vec<Entry>,
    index_hash: String,
    stage_blobs: Vec<String>,
    metadata: BTreeMap<String, Vec<u8>>,
}
pub(super) fn private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
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
pub(super) fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub(super) fn work_entry(root: &Path, path: &NativePath) -> Result<(String, u32, Option<Vec<u8>>)> {
    let absolute = safe_path(root, path)?;
    let meta = match fs::symlink_metadata(&absolute) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(("missing".into(), 0, None))
        }
        Err(e) => return Err(e.into()),
    };
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode()
    };
    #[cfg(windows)]
    let mode = if meta.permissions().readonly() {
        0o444
    } else {
        0o644
    };
    if meta.file_type().is_symlink() {
        let target = fs::read_link(&absolute)?;
        #[cfg(unix)]
        let bytes = {
            use std::os::unix::ffi::OsStrExt;
            target.as_os_str().as_bytes().to_vec()
        };
        #[cfg(windows)]
        let bytes = target
            .to_str()
            .ok_or_else(|| AppError::new("encoding", "Cannot snapshot symlink target"))?
            .as_bytes()
            .to_vec();
        Ok(("symlink".into(), mode, Some(bytes)))
    } else if meta.is_file() {
        Ok((
            "file".into(),
            mode,
            Some(read_file(&absolute, 16 * 1024 * 1024)?),
        ))
    } else if meta.is_dir() {
        Ok(("directory".into(), mode, None))
    } else {
        Err(AppError::new(
            "merge_path",
            "Special filesystem objects need manual inspection before changing merge state.",
        ))
    }
}
pub(super) fn paths(git: &GitCli, root: &Path, marker: &Marker) -> Result<Vec<NativePath>> {
    super::preserved::verify(git, root, marker)?;
    let initial = changed_paths(git, root, &marker.original_oid)?
        .into_iter()
        .filter(|p| !marker.preserved_submodules.iter().any(|v| v.path == *p))
        .collect::<Vec<_>>();
    let mut paths = initial
        .iter()
        .cloned()
        .map(|p| (p.hex.clone(), p))
        .collect::<BTreeMap<_, _>>();
    if !initial.is_empty() {
        let owned = initial
            .iter()
            .map(NativePath::to_path)
            .collect::<Result<Vec<_>>>()?;
        for ignored in [false, true] {
            let mut args: Vec<OsString> = vec![
                "ls-files".into(),
                "--others".into(),
                "--exclude-standard".into(),
                "-z".into(),
            ];
            if ignored {
                args.push("--ignored".into());
            }
            args.push("--".into());
            args.extend(owned.iter().map(|p| p.as_os_str().into()));
            let raw = git
                .read(
                    root,
                    &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                    8 * 1024 * 1024,
                )?
                .bytes;
            for bytes in raw.split(|b| *b == 0).filter(|b| !b.is_empty()) {
                let path = NativePath::new(&crate::path_from_bytes(bytes.to_vec())?);
                safe_path(root, &path)?;
                paths.insert(path.hex.clone(), path);
            }
        }
    }
    if paths.len() > 2000 {
        return Err(AppError::new("safety_budget","Merge recovery exceeds 2,000 paths. Preserve work externally before recovering with Git."));
    }
    Ok(paths.into_values().collect())
}
pub(super) fn fingerprint(
    git: &GitCli,
    root: &Path,
    marker: &Marker,
) -> Result<(String, Vec<NativePath>)> {
    let paths = paths(git, root, marker)?;
    let mut hash = Sha256::new();
    use std::io::Read;
    let mut total = 0usize;
    let started = std::time::Instant::now();
    for path in &paths {
        let absolute = safe_path(root, path)?;
        if fs::symlink_metadata(&absolute)
            .is_ok_and(|meta| meta.is_file() && !meta.file_type().is_symlink())
        {
            let mut options = fs::OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NOFOLLOW);
            }
            let mut file = options.open(&absolute)?;
            let meta = file.metadata()?;
            if !meta.is_file() {
                return Err(AppError::new(
                    "stale_preview",
                    "Merge file type changed. Review again.",
                ));
            }
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                meta.permissions().mode()
            };
            #[cfg(windows)]
            let mode: u32 = if meta.permissions().readonly() {
                0o444
            } else {
                0o644
            };
            hash.update(path.hex.as_bytes());
            hash.update(b"file");
            hash.update(mode.to_le_bytes());
            let mut content = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                total += count;
                if total > 2 * 1024 * 1024 * 1024
                    || started.elapsed() > std::time::Duration::from_secs(20)
                {
                    return Err(AppError::new("safety_budget", "Merge identity scan exceeds 2 GiB or 20 seconds. Inspect this operation with Git."));
                }
                content.update(&buffer[..count]);
            }
            hash.update(format!("{:x}", content.finalize()).as_bytes());
        } else {
            let (kind, mode, data) = work_entry(root, path)?;
            hash.update(path.hex.as_bytes());
            hash.update(kind.as_bytes());
            hash.update(mode.to_le_bytes());
            if let Some(bytes) = data {
                total += bytes.len();
                hash.update(digest(&bytes).as_bytes());
            }
        }
    }
    let d = git.discover(root)?;
    hash.update(metadata(&d.git_dir.join("index"), 64 * 1024 * 1024)?);
    hash.update(git.config_snapshot(root)?);
    hash.update(
        serde_json::to_vec(marker).map_err(|e| AppError::new("merge_metadata", e.to_string()))?,
    );
    for name in [
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "MERGE_HEAD",
        "ORIG_HEAD",
        "MERGE_MSG",
        "MERGE_MODE",
        "AUTO_MERGE",
    ] {
        hash.update(name.as_bytes());
        hash.update(metadata(&d.git_dir.join(name), 1024 * 1024)?);
    }
    Ok((format!("{:x}", hash.finalize()), paths))
}
pub(super) fn save(
    git: &GitCli,
    root: &Path,
    marker: &Marker,
    preview: &MergeActionPreview,
) -> Result<()> {
    let directory = preview
        .recovery_dir
        .as_ref()
        .ok_or_else(|| AppError::new("merge_recovery", "No reviewed recovery location"))?
        .to_path()?;
    let session = marker.session_dir.to_path()?;
    if directory.parent() != Some(session.as_path())
        || fs::symlink_metadata(&directory).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(AppError::new("merge_recovery", "Invalid recovery location"));
    }
    if directory.exists() {
        let old: Manifest = serde_json::from_slice(&metadata(
            &directory.join("manifest.json"),
            4 * 1024 * 1024,
        )?)
        .map_err(|_| {
            AppError::new(
                "merge_recovery",
                "Existing snapshot is incomplete. Inspect it before retrying.",
            )
        })?;
        if old.token != preview.token
            || digest(&metadata(&directory.join("index"), 64 * 1024 * 1024)?) != old.index_hash
        {
            return Err(AppError::new(
                "merge_recovery",
                "Existing recovery snapshot differs from this review.",
            ));
        }
        for entry in old.entries {
            if let Some(file) = entry.file {
                if digest(&metadata(&directory.join(file), 16 * 1024 * 1024)?) != entry.hash {
                    return Err(AppError::new(
                        "merge_recovery",
                        "Recovery content needs inspection.",
                    ));
                }
            }
        }
        for oid in old.stage_blobs {
            crate::validate_oid(&oid)?;
            if digest(&metadata(
                &directory.join(format!("blob-{oid}")),
                16 * 1024 * 1024,
            )?) != digest(
                &git.read(
                    root,
                    &["cat-file".as_ref(), "blob".as_ref(), oid.as_ref()],
                    16 * 1024 * 1024,
                )?
                .bytes,
            ) {
                return Err(AppError::new(
                    "merge_recovery",
                    "Recovery stage blob differs from the current index.",
                ));
            }
        }
        return Ok(());
    }
    private_dir(&directory)?;
    let d = git.discover(root)?;
    let index = metadata(&d.git_dir.join("index"), 64 * 1024 * 1024)?;
    private_file(&directory.join("index"), &index)?;
    let mut entries = vec![];
    let mut total = 0;
    for path in &preview.paths {
        let (kind, mode, data) = work_entry(root, path)?;
        let file = data
            .as_ref()
            .map(|_| format!("work-{}", digest(path.hex.as_bytes())));
        let hash = data.as_ref().map_or_else(String::new, |v| digest(v));
        if let (Some(name), Some(data)) = (&file, &data) {
            total += data.len();
            if total > 64 * 1024 * 1024 {
                return Err(AppError::new(
                    "safety_budget",
                    "Recovery working data exceeds 64 MiB.",
                ));
            }
            private_file(&directory.join(name), data)?;
        }
        entries.push(Entry {
            path: path.clone(),
            kind,
            mode,
            file,
            hash,
        });
    }
    let mut stage_blobs = vec![];
    if !preview.paths.is_empty() {
        let owned = preview
            .paths
            .iter()
            .map(NativePath::to_path)
            .collect::<Result<Vec<_>>>()?;
        let mut args: Vec<OsString> = vec![
            "ls-files".into(),
            "--stage".into(),
            "-z".into(),
            "--".into(),
        ];
        args.extend(owned.iter().map(|p| p.as_os_str().into()));
        let raw = git
            .read(
                root,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                8 * 1024 * 1024,
            )?
            .bytes;
        let mut oids = std::collections::BTreeSet::new();
        for record in raw.split(|b| *b == 0).filter(|v| !v.is_empty()) {
            let header = record.split(|b| *b == b'\t').next().unwrap_or_default();
            let header = std::str::from_utf8(header)
                .map_err(|_| AppError::new("merge_recovery", "Invalid index stage"))?;
            let fields = header.split_whitespace().collect::<Vec<_>>();
            if fields.len() != 3 {
                return Err(AppError::new("merge_recovery", "Invalid index stage"));
            }
            if fields[0] != "160000" {
                crate::validate_oid(fields[1])?;
                oids.insert(fields[1].to_owned());
            }
        }
        for oid in oids {
            let bytes = git
                .read(
                    root,
                    &["cat-file".as_ref(), "blob".as_ref(), oid.as_ref()],
                    16 * 1024 * 1024,
                )?
                .bytes;
            total += bytes.len();
            if total > 128 * 1024 * 1024 {
                return Err(AppError::new(
                    "safety_budget",
                    "Recovery working/staged data exceeds 128 MiB.",
                ));
            }
            private_file(&directory.join(format!("blob-{oid}")), &bytes)?;
            stage_blobs.push(oid);
        }
    }
    let mut fields = BTreeMap::new();
    for name in [
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "MERGE_HEAD",
        "ORIG_HEAD",
        "MERGE_MSG",
        "MERGE_MODE",
        "AUTO_MERGE",
    ] {
        fields.insert(
            name.to_owned(),
            metadata(&d.git_dir.join(name), 1024 * 1024)?,
        );
    }
    let manifest = Manifest {
        version: 1,
        token: preview.token.clone(),
        entries,
        index_hash: digest(&index),
        stage_blobs,
        metadata: fields,
    };
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| AppError::new("merge_recovery", e.to_string()))?;
    private_file(&directory.join("manifest.json"), &bytes)?;
    Ok(())
}

/// Preflight recovery storage separately from streaming identity or visible patch limits.
pub(super) fn backup_blocker(
    git: &GitCli,
    root: &Path,
    paths: &[NativePath],
) -> Result<Option<String>> {
    let reason = "Recovery needs more than 16 MiB per file, 64 MiB of working data or 128 MiB total. Preserve these files externally and recover with Git; staged commit review remains available.";
    let mut total = 0u64;
    for path in paths {
        if let Ok(meta) = fs::symlink_metadata(safe_path(root, path)?) {
            if !meta.is_dir() {
                total += meta.len();
                if meta.len() > 16 * 1024 * 1024 || total > 64 * 1024 * 1024 {
                    return Ok(Some(reason.into()));
                }
            }
        }
    }
    if paths.is_empty() {
        return Ok(None);
    }
    let mut args: Vec<OsString> = vec![
        "ls-files".into(),
        "--stage".into(),
        "-z".into(),
        "--".into(),
    ];
    for path in paths {
        args.push(path.to_path()?.into_os_string());
    }
    let raw = git
        .read(
            root,
            &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            8 * 1024 * 1024,
        )?
        .bytes;
    let mut oids = std::collections::BTreeSet::new();
    for record in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let header = std::str::from_utf8(record.split(|b| *b == b'\t').next().unwrap_or_default())
            .map_err(|_| AppError::new("merge_recovery", "Invalid index stage"))?;
        let fields = header.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(AppError::new("merge_recovery", "Invalid index stage"));
        }
        if fields[0] != "160000" {
            crate::validate_oid(fields[1])?;
            oids.insert(fields[1].to_owned());
        }
    }
    for oid in oids {
        let size: u64 = git
            .text(root, &["cat-file", "-s", &oid])?
            .trim()
            .parse()
            .map_err(|_| AppError::new("merge_recovery", "Invalid blob size"))?;
        total += size;
        if size > 16 * 1024 * 1024 || total > 128 * 1024 * 1024 {
            return Ok(Some(reason.into()));
        }
    }
    Ok(None)
}
