use super::*;

impl GitCli {
    fn pointer_index_oid(&self, root: &Path, path: &NativePath) -> Result<String> {
        let path = relative(path)?;
        let raw = self
            .read(
                root,
                &[
                    OsStr::new("ls-files"),
                    OsStr::new("--stage"),
                    OsStr::new("-z"),
                    OsStr::new("--"),
                    path.as_os_str(),
                ],
                1024 * 1024,
            )?
            .bytes;
        let records: Vec<_> = raw
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty())
            .collect();
        if records.len() != 1 {
            return Err(AppError::new(
                "submodule_pointer",
                "Resolve the submodule index conflict before changing its pointer.",
            ));
        }
        let header = records[0]
            .split(|byte| *byte == b'\t')
            .next()
            .unwrap_or_default();
        let fields: Vec<_> = header.split(|byte| *byte == b' ').collect();
        if fields.len() != 3 || fields[0] != b"160000" || fields[2] != b"0" {
            return Err(AppError::new(
                "submodule_pointer",
                "Only an existing submodule pointer can be changed here.",
            ));
        }
        let oid = String::from_utf8(fields[1].to_vec())
            .map_err(|_| AppError::new("submodule_pointer", "Invalid submodule index revision."))?;
        crate::validate_oid(&oid)?;
        Ok(oid)
    }
    pub fn preview_submodule_pointer(
        &self,
        root: &Path,
        sub: &Submodule,
        stage: bool,
    ) -> Result<SubmodulePointerPreview> {
        let (_, before) = self.write_state(root)?;
        let relative = relative(&sub.path)?;
        if sub.parent_relation_id.is_some() || sub.depth != 0 {
            return Err(AppError::new(
                "submodule_pointer",
                "Open the owning parent repository to change a nested submodule pointer.",
            ));
        }
        let index_oid = self.pointer_index_oid(root, &sub.path)?;
        let flags = self
            .read(
                root,
                &[
                    OsStr::new("ls-files"),
                    OsStr::new("-v"),
                    OsStr::new("-z"),
                    OsStr::new("--"),
                    relative.as_os_str(),
                ],
                1024 * 1024,
            )?
            .bytes;
        if flags.first() != Some(&b'H') {
            return Err(AppError::new(
                "submodule_pointer",
                "Inspect skip-worktree or assume-unchanged flags before changing this pointer.",
            ));
        }
        let head = self
            .read(
                root,
                &[
                    OsStr::new("ls-tree"),
                    OsStr::new("-z"),
                    OsStr::new("HEAD"),
                    OsStr::new("--"),
                    relative.as_os_str(),
                ],
                1024 * 1024,
            )?
            .bytes;
        let head_fields: Vec<_> = head
            .split(|byte| *byte == b'\t')
            .next()
            .unwrap_or_default()
            .split(|byte| *byte == b' ')
            .collect();
        if head_fields.len() != 3
            || head_fields[0] != b"160000"
            || sub.head_oid.as_deref().map(str::as_bytes) != Some(head_fields[2])
        {
            return Err(AppError::new(
                "submodule_pointer",
                "Adding or removing submodules needs its own reviewed workflow.",
            ));
        }
        if index_oid != sub.index_oid {
            return Err(AppError::new(
                "stale_preview",
                "Submodule pointer changed. Refresh and review again.",
            ));
        }
        let mut blocked = if sub.inspection_error.is_some()
            || !sub.initialized
            || sub.checked_out_oid.is_none()
        {
            Some("Initialize and inspect this submodule before changing its pointer.".into())
        } else {
            None
        };
        let mut child_identity = String::new();
        if sub.initialized && sub.inspection_error.is_none() {
            let mut child = root.to_path_buf();
            for part in relative.components() {
                child.push(part);
                if fs::symlink_metadata(&child)?.file_type().is_symlink() {
                    return Err(AppError::new(
                        "submodule_pointer",
                        "Submodule paths through symbolic links need manual inspection.",
                    ));
                }
            }
            let discovered = self.discover(&child)?;
            if discovered.root.as_deref() != Some(fs::canonicalize(&child)?.as_path()) {
                return Err(AppError::new(
                    "submodule_pointer",
                    "The selected path is not this submodule's worktree.",
                ));
            }
            let status = self.status(&child)?;
            if status.head != sub.checked_out_oid || Some(status.dirty) != sub.dirty {
                return Err(AppError::new(
                    "stale_preview",
                    "Submodule checkout changed. Refresh and review again.",
                ));
            }
            child_identity = format!(
                "{}:{}",
                NativePath::new(&discovered.common_dir).hex,
                NativePath::new(&discovered.git_dir).hex
            );
        }
        let target = if stage {
            sub.checked_out_oid.clone()
        } else {
            sub.head_oid.clone()
        };
        if sub.head_oid.is_none() {
            blocked = Some("Adding or removing submodules needs its own reviewed workflow.".into());
        }
        let target = target.unwrap_or_else(|| index_oid.clone());
        if target == index_oid && blocked.is_none() {
            blocked=Some(if stage {"No new submodule commit to stage. Open the submodule to commit its files first."} else {"This submodule pointer is not staged."}.into());
        }
        let changes = self.working_changes(root)?;
        if changes.files.iter().any(|file| {
            file.index_status == "U"
                || file.worktree_status == "U"
                || matches!(
                    (file.index_status.as_str(), file.worktree_status.as_str()),
                    ("A", "A") | ("D", "D")
                )
        }) {
            blocked = Some(
                "Resolve the existing index conflicts before changing submodule pointers.".into(),
            );
        }
        if self.write_state(root)?.1 != before {
            return Err(AppError::new(
                "stale_preview",
                "Parent index changed. Review the submodule pointer again.",
            ));
        }
        let input = serde_json::to_vec(&(1, before, sub, stage, &target, child_identity))
            .map_err(|error| AppError::new("submodule_pointer", error.to_string()))?;
        Ok(SubmodulePointerPreview {
            version: 1,
            submodule: sub.clone(),
            stage,
            target_oid: target,
            parent_status: changes.status,
            blocked_reason: blocked,
            token: digest(&input),
        })
    }
    pub fn apply_submodule_pointer(
        &self,
        root: &Path,
        sub: &Submodule,
        stage: bool,
        token: &str,
        recovery_index: &Path,
    ) -> Result<()> {
        let preview = self.preview_submodule_pointer(root, sub, stage)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Submodule pointer review changed. Review again.",
            ));
        }
        if let Some(reason) = preview.blocked_reason {
            return Err(AppError::new("submodule_pointer", reason));
        }
        crate::validate_oid(&preview.target_oid)?;
        let (discovery, state) = self.write_state(root)?;
        let mut lock = OperationLock::acquire(discovery.git_dir.join("index.lock"))?;
        let original = index_bytes(&discovery.git_dir)?;
        if self.preview_submodule_pointer(root, sub, stage)?.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Parent or child changed while acquiring the index lock.",
            ));
        }
        let temporary = tempfile::NamedTempFile::new_in(&discovery.git_dir)?.into_temp_path();
        fs::write(&temporary, &original)?;
        preserve_index_timestamp(&discovery.git_dir.join("index"), &temporary)?;
        #[cfg(unix)]
        let raw_path = {
            use std::os::unix::ffi::OsStrExt;
            relative(&sub.path)?.as_os_str().as_bytes().to_vec()
        };
        #[cfg(windows)]
        let raw_path = relative(&sub.path)?
            .to_str()
            .ok_or_else(|| AppError::new("encoding", "Submodule path is not UTF-8."))?
            .as_bytes()
            .to_vec();
        let mut input = format!("160000 {}\t", preview.target_oid).into_bytes();
        input.extend(raw_path);
        input.push(0);
        run_git_write(
            root,
            &["update-index".into(), "-z".into(), "--index-info".into()],
            Some(&temporary),
            Some(""),
            Some(&input),
        )?;
        if self.write_state(root)?.1 != state
            || self.preview_submodule_pointer(root, sub, stage)?.token != token
        {
            return Err(AppError::new(
                "stale_preview",
                "Parent or child changed during preparation. Original index preserved.",
            ));
        }
        let prepared = read_bounded(&temporary, INDEX_LIMIT)?;
        let parent = recovery_index
            .parent()
            .ok_or_else(|| AppError::new("submodule_pointer", "Invalid index recovery path."))?;
        fs::create_dir_all(parent)?;
        let mut backup = tempfile::NamedTempFile::new_in(parent)?;
        backup.write_all(&original)?;
        backup.flush()?;
        backup.as_file().sync_all()?;
        backup.persist_noclobber(recovery_index).map_err(|error| {
            AppError::new(
                "submodule_pointer",
                format!("Cannot preserve the original parent index: {error}"),
            )
        })?;
        let handle = lock
            .file
            .as_mut()
            .ok_or_else(|| AppError::new("lock", "Index lock unavailable"))?;
        handle.set_permissions(fs::metadata(discovery.git_dir.join("index"))?.permissions())?;
        handle.set_len(0)?;
        handle.rewind()?;
        handle.write_all(&prepared)?;
        handle
            .set_times(fs::FileTimes::new().set_modified(fs::metadata(&temporary)?.modified()?))?;
        handle.sync_all()?;
        lock.file.take();
        fs::rename(&lock.path, discovery.git_dir.join("index"))?;
        lock.path = PathBuf::new();
        if self.pointer_index_oid(root, &sub.path)? != preview.target_oid {
            return Err(AppError::new("operation_unknown","Parent index changed after pointer update. Inspect the saved index before retrying."));
        }
        Ok(())
    }
}
