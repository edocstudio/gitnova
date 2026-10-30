//! Coupled manifest/gitlink index transaction; checkout files are never changed.
use super::*;
use std::collections::BTreeMap;
fn error(s: &str) -> AppError {
    AppError::new("submodule_registration_index", s)
}
fn entry(
    git: &GitCli,
    root: &Path,
    tree: Option<&str>,
    path: &Path,
) -> Result<Option<(String, String)>> {
    let Some(tree) = tree else { return Ok(None) };
    let raw = git
        .read(
            root,
            &[
                "--literal-pathspecs".as_ref(),
                "ls-tree".as_ref(),
                "-z".as_ref(),
                tree.as_ref(),
                "--".as_ref(),
                path.as_os_str(),
            ],
            65536,
        )?
        .bytes;
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.iter().filter(|b| **b == 0).count() != 1 {
        return Err(error("Select one exact registration path."));
    }
    let header = raw.split(|b| *b == b'\t').next().unwrap_or_default();
    let fields = header.split(|b| *b == b' ').collect::<Vec<_>>();
    if fields.len() != 3 {
        return Err(error("Invalid registration tree entry."));
    }
    let mode = std::str::from_utf8(fields[0]).map_err(|_| error("Invalid registration mode."))?;
    let oid =
        std::str::from_utf8(fields[2]).map_err(|_| error("Invalid registration revision."))?;
    crate::validate_oid(oid)?;
    Ok(Some((mode.into(), oid.into())))
}
fn manifest(git: &GitCli, root: &Path, bytes: &[u8]) -> Result<BTreeMap<String, String>> {
    let raw = git
        .read_input(
            root,
            &[
                "config".as_ref(),
                "--no-includes".as_ref(),
                "--null".as_ref(),
                "--file".as_ref(),
                "-".as_ref(),
                "--list".as_ref(),
            ],
            bytes,
            131072,
        )?
        .bytes;
    let mut entries = BTreeMap::new();
    for field in raw.split(|b| *b == 0).filter(|f| !f.is_empty()) {
        let at = field
            .iter()
            .position(|b| *b == b'\n')
            .ok_or_else(|| error("Invalid submodule manifest entry."))?;
        let key = std::str::from_utf8(&field[..at])
            .map_err(|_| error("Registration manifest must be UTF-8."))?
            .to_owned();
        let value = std::str::from_utf8(&field[at + 1..])
            .map_err(|_| error("Registration manifest must be UTF-8."))?
            .to_owned();
        if !key.starts_with("submodule.") || entries.insert(key, value).is_some() {
            return Err(error(
                "Duplicate or non-submodule manifest settings need manual review.",
            ));
        }
    }
    Ok(entries)
}
fn section(map: &BTreeMap<String, String>, path: &str) -> Result<Option<String>> {
    let names = map
        .iter()
        .filter(|(k, v)| k.ends_with(".path") && *v == path)
        .map(|(k, _)| k.trim_end_matches(".path").to_owned())
        .collect::<Vec<_>>();
    if names.len() > 1 {
        return Err(error(
            "Ambiguous submodule manifest paths need manual review.",
        ));
    }
    Ok(names.into_iter().next())
}
fn without(map: &BTreeMap<String, String>, name: Option<&str>) -> BTreeMap<String, String> {
    map.iter()
        .filter(|(k, _)| !name.is_some_and(|n| k.starts_with(&format!("{n}."))))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
impl GitCli {
    pub fn preview_registration_index(
        &self,
        root: &Path,
        r: &SubmoduleLifecycleRequest,
    ) -> Result<SubmoduleLifecyclePreview> {
        if !["stage", "unstage"].contains(&r.kind.as_str())
            || !r.source.is_empty()
            || r.id.len() != 32
            || !r.id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(error(
                "Choose Stage or Unstage registration and review again.",
            ));
        }
        let path = relative(&NativePath::new(Path::new(&r.path)))?;
        if r.path.len() > 512
            || r.path.chars().any(char::is_control)
            || path == Path::new(".gitmodules")
        {
            return Err(error(
                "Choose a regular relative submodule registration path.",
            ));
        }
        if fs::symlink_metadata(root.join(&path)).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(error(
                "Submodule paths through symbolic links need manual inspection.",
            ));
        }
        file_digest(root, &path).or_else(|e| {
            if e.code == "unsupported_write" {
                Ok(String::new())
            } else {
                Err(e)
            }
        })?;
        let (d, state) = self.write_state(root)?;
        let status = self.status(root)?;
        let head = entry(self, root, status.head.as_deref(), &path)?;
        if head.as_ref().is_some_and(|(m, _)| m != "160000") {
            return Err(error("This HEAD path is not a submodule registration."));
        }
        let baseline = entry(self, root, status.head.as_deref(), Path::new(".gitmodules"))?;
        let before = if let Some((mode, oid)) = baseline {
            if !matches!(mode.as_str(), "100644" | "100755") {
                return Err(error("Manifest must be a regular file."));
            }
            self.read(
                root,
                &["cat-file".as_ref(), "blob".as_ref(), oid.as_ref()],
                65536,
            )?
            .bytes
        } else {
            vec![]
        };
        let after = match fs::symlink_metadata(root.join(".gitmodules")) {
            Ok(m) if m.is_file() && !m.file_type().is_symlink() => {
                read_bounded(&root.join(".gitmodules"), 65536)?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            _ => return Err(error("Manifest must be a regular file.")),
        };
        let old = manifest(self, root, &before)?;
        let new = manifest(self, root, &after)?;
        let old_section = section(&old, &r.path)?;
        let new_section = section(&new, &r.path)?;
        if without(&old, old_section.as_deref()) != without(&new, new_section.as_deref()) {
            return Err(error("Review other registration changes separately before staging this manifest and pointer."));
        }
        let addition = head.is_none() && old_section.is_none() && new_section.is_some();
        let removal = head.is_some() && old_section.is_some() && new_section.is_none();
        if !addition && !removal {
            return Err(error("Choose a pending registration addition or removal. Existing pointers use pointer review."));
        }
        let changes = self.working_changes(root)?;
        if changes.files.iter().any(|f| {
            f.index_status == "U"
                || f.worktree_status == "U"
                || matches!(
                    (f.index_status.as_str(), f.worktree_status.as_str()),
                    ("A", "A") | ("D", "D")
                )
        }) {
            return Err(error(
                "Resolve index conflicts before changing registration staging.",
            ));
        }
        let mut target = None;
        let mut child = None;
        if addition {
            let checkout = root.join(&path);
            let discovered = self.discover(&checkout)?;
            if discovered.root.as_ref() != Some(&checkout.canonicalize()?)
                || discovered.common_dir == d.common_dir
            {
                return Err(error(
                    "The registration child identity changed. Inspect before staging.",
                ));
            }
            let child_status = self.status(&checkout)?;
            target = child_status.head.clone();
            if target.is_none() {
                return Err(error(
                    "The child needs a committed revision before registration staging.",
                ));
            }
            child = Some(self.head_index_snapshot(&checkout)?);
        }
        let index = self
            .read(
                root,
                &[
                    "ls-files".as_ref(),
                    "--stage".as_ref(),
                    "-z".as_ref(),
                    "--".as_ref(),
                    path.as_os_str(),
                    ".gitmodules".as_ref(),
                ],
                131072,
            )?
            .bytes;
        let mut indexed_manifest = vec![];
        for record in index.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let tab = record
                .iter()
                .position(|b| *b == b'\t')
                .ok_or_else(|| error("Invalid registration index entry."))?;
            let fields = record[..tab].split(|b| *b == b' ').collect::<Vec<_>>();
            if fields.len() != 3 || fields[2] != b"0" {
                return Err(error("Resolve registration index conflicts first."));
            }
            if &record[tab + 1..] == b".gitmodules" {
                if !matches!(fields[0], b"100644" | b"100755") {
                    return Err(error("Manifest must be a regular file."));
                }
                let oid = std::str::from_utf8(fields[1])
                    .map_err(|_| error("Invalid registration revision."))?;
                crate::validate_oid(oid)?;
                indexed_manifest = self
                    .read(
                        root,
                        &["cat-file".as_ref(), "blob".as_ref(), oid.as_ref()],
                        65536,
                    )?
                    .bytes;
            } else if fields[0] != b"160000" {
                return Err(error("The index path is not a submodule registration."));
            }
        }
        let indexed = manifest(self, root, &indexed_manifest)?;
        let indexed_section = section(&indexed, &r.path)?;
        if without(&indexed, indexed_section.as_deref()) != without(&old, old_section.as_deref()) {
            return Err(error(
                "Other staged manifest changes must be reviewed separately.",
            ));
        }
        let index_manifest = self
            .read(
                root,
                &[
                    "ls-files".as_ref(),
                    "-v".as_ref(),
                    "-z".as_ref(),
                    "--".as_ref(),
                    path.as_os_str(),
                    ".gitmodules".as_ref(),
                ],
                65536,
            )?
            .bytes;
        if index_manifest
            .split(|b| *b == 0)
            .filter(|v| !v.is_empty())
            .any(|v| v.first() != Some(&b'H'))
        {
            return Err(error("Hidden registration index flags need manual review."));
        }
        let staged = changes.files.iter().any(|f| {
            f.staged
                && (f.path == NativePath::new(&path)
                    || f.path == NativePath::new(Path::new(".gitmodules")))
        });
        let blocked =
            (r.kind == "unstage" && !staged).then(|| "This registration is not staged.".into());
        let token = digest(
            &serde_json::to_vec(&(
                r,
                &state,
                &before,
                &after,
                &target,
                &child,
                &index,
                self.config_snapshot(root)?,
            ))
            .map_err(|e| error(&e.to_string()))?,
        );
        let recovery_dir = NativePath::new(
            &d.git_dir
                .join(format!("gitnova-submodule-registration-{}", r.id)),
        );
        let blocked = if recovery_dir.to_path()?.exists() {
            Some("This registration change has a recovery record. Inspect its previous result before retrying.".into())
        } else {
            blocked
        };
        Ok(SubmoduleLifecyclePreview {
            request: r.clone(),
            branch: status.branch,
            child_head: target,
            download: false,
            recovery_dir,
            token,
            blocked_reason: blocked,
            manifest_before: Some(
                String::from_utf8(indexed_manifest)
                    .map_err(|_| error("Registration manifest must be UTF-8."))?,
            ),
            manifest_after: Some(
                String::from_utf8(if r.kind == "stage" { after } else { before })
                    .map_err(|_| error("Registration manifest must be UTF-8."))?,
            ),
        })
    }
    pub fn change_registration_index(
        &self,
        root: &Path,
        r: &SubmoduleLifecycleRequest,
        token: &str,
    ) -> Result<()> {
        let p = self.preview_registration_index(root, r)?;
        if p.token != token {
            return Err(error("Registration staging changed. Review again."));
        }
        if let Some(reason) = p.blocked_reason {
            return Err(error(&reason));
        }
        let (d, state) = self.write_state(root)?;
        let mut lock = OperationLock::acquire(d.git_dir.join("index.lock"))?;
        let original = index_bytes(&d.git_dir)?;
        let temporary = tempfile::NamedTempFile::new_in(&d.git_dir)?.into_temp_path();
        if original.is_empty() {
            fs::remove_file(&temporary)?;
        } else {
            fs::write(&temporary, &original)?;
        }
        let path = relative(&NativePath::new(Path::new(&r.path)))?;
        if r.kind == "unstage" {
            if self.status(root)?.head.is_some() {
                run_git_write(
                    root,
                    &[
                        "reset".into(),
                        "-q".into(),
                        "HEAD".into(),
                        "--".into(),
                        ".gitmodules".into(),
                        path.clone().into_os_string(),
                    ],
                    Some(&temporary),
                    Some(""),
                    None,
                )?;
            } else {
                run_git_write(
                    root,
                    &[
                        "update-index".into(),
                        "--force-remove".into(),
                        "--".into(),
                        ".gitmodules".into(),
                        path.clone().into_os_string(),
                    ],
                    Some(&temporary),
                    Some(""),
                    None,
                )?;
            }
        } else {
            if root.join(".gitmodules").is_file() || self.status(root)?.head.is_some() {
                run_git_write(
                    root,
                    &["add".into(), "--".into(), ".gitmodules".into()],
                    Some(&temporary),
                    Some(""),
                    None,
                )?;
            }
            let mut input = if let Some(oid) = &p.child_head {
                format!("160000 {oid}\t").into_bytes()
            } else {
                format!(
                    "0 {}\t",
                    "0".repeat(if d.object_format == "sha256" { 64 } else { 40 })
                )
                .into_bytes()
            };
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt;
                input.extend(path.as_os_str().as_bytes());
            }
            #[cfg(windows)]
            {
                input.extend(r.path.as_bytes());
            }
            input.push(0);
            run_git_write(
                root,
                &["update-index".into(), "-z".into(), "--index-info".into()],
                Some(&temporary),
                Some(""),
                Some(&input),
            )?;
        }
        if self.write_state(root)?.1 != state
            || self.preview_registration_index(root, r)?.token != token
        {
            return Err(error(
                "Registration changed during index preparation. Original staging preserved.",
            ));
        }
        let recovery = p.recovery_dir.to_path()?;
        fs::create_dir(&recovery)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&recovery, fs::Permissions::from_mode(0o700))?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut backup = options.open(recovery.join("index"))?;
        backup.write_all(&original)?;
        backup.sync_all()?;
        if self.write_state(root)?.1 != state {
            return Err(error(
                "Parent changed while saving index recovery. Original staging preserved.",
            ));
        }
        let prepared = read_bounded(&temporary, INDEX_LIMIT)?;
        lock.publish(&d.git_dir.join("index"), &prepared, None)?;
        Ok(())
    }
}
