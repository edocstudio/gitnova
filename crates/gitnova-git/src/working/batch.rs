use super::*;
impl GitCli {
    pub fn preview_stage_all(&self, root: &Path) -> Result<StageAllPreview> {
        let (_, before) = self.write_state(root)?;
        let changes = self.working_changes(root)?;
        let config = self.config_snapshot(root)?;
        let attributes = crate::merge::scratch::attributes(self, root)?;
        let mut files = vec![];
        let mut excluded = vec![];
        let mut contents = vec![];
        let mut total = 0u64;
        let conflict = changes.files.iter().any(|f| {
            f.index_status == "U"
                || f.worktree_status == "U"
                || matches!(
                    (f.index_status.as_str(), f.worktree_status.as_str()),
                    ("A", "A") | ("D", "D")
                )
        });
        for file in changes.files.iter().filter(|f| f.unstaged) {
            if file.blocked_reason.is_some() {
                excluded.push(file.clone());
                continue;
            }
            let relative = relative(&file.path)?;
            if let Ok(metadata) = fs::symlink_metadata(root.join(&relative)) {
                total = total.saturating_add(metadata.len());
            }
            if total > 256 * 1024 * 1024 {
                return Err(AppError::new("safety_budget", "Stage all exceeds its 256 MiB working-content budget. Stage a smaller selection."));
            }
            contents.push((&file.path.hex, file_digest(root, &relative)?));
            files.push(file.clone());
        }
        if self.write_state(root)?.1 != before {
            return Err(AppError::new(
                "stale_preview",
                "Index or HEAD changed while reviewing Stage all. Review again.",
            ));
        }
        let data = serde_json::to_vec(&(
            &before,
            &changes.token,
            &config,
            &attributes,
            &files,
            &excluded,
            contents,
        ))
        .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        if data.len() > 512 * 1024 {
            return Err(AppError::new(
                "safety_budget",
                "Stage all file selection exceeds its review budget.",
            ));
        }
        Ok(StageAllPreview {
            files,
            excluded,
            token: digest(&data),
            blocked_reason: if conflict {
                Some("Resolve index conflicts before staging all files.".into())
            } else if total == 0
                && !changes
                    .files
                    .iter()
                    .any(|f| f.unstaged && f.blocked_reason.is_none())
            {
                Some("No eligible unstaged files to stage.".into())
            } else {
                None
            },
        })
    }
    pub fn stage_all(&self, root: &Path, token: &str) -> Result<()> {
        let p = self.preview_stage_all(root)?;
        if p.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Stage all selection or file contents changed. Review again.",
            ));
        }
        if let Some(reason) = p.blocked_reason {
            return Err(AppError::new("stage_blocked", reason));
        }
        if p.files.is_empty() {
            return Err(AppError::new(
                "stage_blocked",
                "No eligible unstaged files to stage.",
            ));
        }
        let (d, state) = self.write_state(root)?;
        let mut lock = OperationLock::acquire(d.git_dir.join("index.lock"))?;
        let original = index_bytes(&d.git_dir)?;
        let temporary = tempfile::NamedTempFile::new_in(&d.git_dir)?.into_temp_path();
        if original.is_empty() {
            fs::remove_file(&temporary)?;
        } else {
            fs::write(&temporary, &original)?;
            preserve_index_timestamp(&d.git_dir.join("index"), &temporary)?;
        }
        if self.preview_stage_all(root)?.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Stage all changed while acquiring the index lock. Review again.",
            ));
        }
        let mut paths = vec![];
        for file in &p.files {
            paths.extend(relative(&file.path)?.as_os_str().as_encoded_bytes());
            paths.push(0);
        }
        run_git_write(
            root,
            &[
                "add".into(),
                "--pathspec-from-file=-".into(),
                "--pathspec-file-nul".into(),
            ],
            Some(temporary.as_ref()),
            None,
            Some(&paths),
        )?;
        if self.write_state(root)?.1 != state || self.preview_stage_all(root)?.token != token {
            return Err(AppError::new("stale_preview","Files or configuration changed during staging. The original index was preserved; review again."));
        }
        let bytes = read_bounded(&temporary, INDEX_LIMIT)?;
        lock.publish(
            &d.git_dir.join("index"),
            &bytes,
            Some(fs::metadata(&temporary)?.modified()?),
        )
    }
    pub fn commit_file_preview(
        &self,
        root: &Path,
        file: &NativePath,
        token: &str,
    ) -> Result<FilePreview> {
        if self.write_state(root)?.1 != token {
            return Err(AppError::new(
                "stale_preview",
                "The staged commit changed. Review the staged file list again.",
            ));
        }
        let preview = self.file_preview(root, file, true)?;
        if self.write_state(root)?.1 != token {
            return Err(AppError::new(
                "stale_preview",
                "The staged commit changed while loading this file. Review again.",
            ));
        }
        Ok(preview)
    }
}
