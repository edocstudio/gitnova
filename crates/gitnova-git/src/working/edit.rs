//! Bounded current-file editing; no staging, historical restore or Git filters.
use super::{digest, file_digest, read_bounded, relative};
use crate::{GitCli, RepositoryReader};
use gitnova_domain::{AppError, NativePath, Result, WorkingFileText};
use std::{ffi::OsStr, fs, io::Write, path::Path};
use tempfile::NamedTempFile;

const LIMIT: usize = 4 * 1024 * 1024;
fn blocked(message: &str) -> AppError {
    AppError::new("edit_blocked", message)
}
impl GitCli {
    pub fn working_file_text(&self, root: &Path, file: &NativePath) -> Result<WorkingFileText> {
        let (discovery, state) = self.write_state(root)?;
        let root = discovery
            .root
            .ok_or_else(|| blocked("A working checkout is required."))?;
        let path = relative(file)?;
        let mut parent = root.clone();
        for component in path.parent().unwrap_or(Path::new("")).components() {
            parent.push(component);
            if fs::symlink_metadata(&parent)?.file_type().is_symlink()
                || parent.join(".git").exists()
            {
                return Err(blocked(
                    "Open nested repositories or submodules separately to edit their files.",
                ));
            }
        }
        let absolute = root.join(&path);
        let metadata = fs::symlink_metadata(&absolute)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.permissions().readonly()
        {
            return Err(blocked(
                "Only writable regular working files can be edited here.",
            ));
        }
        if metadata.len() > LIMIT as u64 {
            return Err(blocked(
                "Only UTF-8 text files up to 4 MiB can be edited here.",
            ));
        }
        let entries = self
            .read(
                &root,
                &[
                    OsStr::new("--literal-pathspecs"),
                    OsStr::new("ls-files"),
                    OsStr::new("--stage"),
                    OsStr::new("-z"),
                    OsStr::new("--"),
                    path.as_os_str(),
                ],
                16384,
            )?
            .bytes;
        let records: Vec<_> = entries
            .split(|b| *b == 0)
            .filter(|v| !v.is_empty())
            .collect();
        if records.len() != 1
            || !(records[0].starts_with(b"100644 ") || records[0].starts_with(b"100755 "))
            || !records[0]
                .split(|b| *b == b'\t')
                .next()
                .is_some_and(|v| v.ends_with(b" 0"))
        {
            return Err(blocked(
                "Select an existing tracked text file without unresolved conflicts.",
            ));
        }
        let before = file_digest(&root, &path)?;
        let bytes = read_bounded(&absolute, LIMIT as u64)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| blocked("Only UTF-8 text files can be edited here."))?;
        if text.contains('\0')
            || text.replace("\r\n", "").contains('\r')
            || (text.contains("\r\n") && text.replace("\r\n", "").contains('\n'))
        {
            return Err(blocked(
                "Binary files and mixed line endings require an external editor.",
            ));
        }
        let content = file_digest(&root, &path)?;
        if before != content {
            return Err(AppError::new(
                "stale_preview",
                "The working file changed while reading. Try again.",
            ));
        }
        Ok(WorkingFileText {
            path: file.clone(),
            max_bytes: LIMIT,
            text: text.replace("\r\n", "\n"),
            token: digest(
                format!(
                    "{}\0{state}\0{}\0{content}",
                    NativePath::new(&root).hex,
                    file.hex
                )
                .as_bytes(),
            ),
        })
    }
    pub fn save_working_file_text(
        &self,
        root: &Path,
        file: &NativePath,
        text: &str,
        token: &str,
    ) -> Result<()> {
        if text.len() > LIMIT || text.contains('\0') || text.contains('\r') {
            return Err(blocked(
                "Only UTF-8 text up to 4 MiB with normalized line endings can be saved.",
            ));
        }
        let _operation = self.acquire_operation(root)?;
        let _index = self.acquire_index_guard(root)?;
        let preview = self.working_file_text(root, file)?;
        if preview.token != token {
            return Err(AppError::new("stale_preview", "The working file, branch or staging changed. Review the current file before saving; your draft is preserved."));
        }
        let discovery = self.discover(root)?;
        let root = discovery
            .root
            .ok_or_else(|| blocked("A working checkout is required."))?;
        let absolute = root.join(relative(file)?);
        let bytes = read_bounded(&absolute, LIMIT as u64)?;
        let replacement = if bytes.windows(2).any(|v| v == b"\r\n") {
            text.replace('\n', "\r\n").into_bytes()
        } else {
            text.as_bytes().to_vec()
        };
        if replacement.len() > LIMIT {
            return Err(blocked("The encoded file exceeds 4 MiB."));
        }
        let mut output = NamedTempFile::new_in(
            absolute
                .parent()
                .ok_or_else(|| blocked("Missing file parent."))?,
        )?;
        output
            .as_file()
            .set_permissions(fs::metadata(&absolute)?.permissions())?;
        output.write_all(&replacement)?;
        output.as_file().sync_all()?;
        // Retain original raw bytes privately, including line endings and BOM.
        let recovery = discovery.git_dir.join("gitnova-file-edits");
        if fs::symlink_metadata(&recovery)
            .is_ok_and(|meta| meta.file_type().is_symlink() || !meta.is_dir())
        {
            return Err(blocked(
                "The edit recovery directory is not a regular directory.",
            ));
        }
        fs::create_dir_all(&recovery)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&recovery, fs::Permissions::from_mode(0o700))?;
        }
        let mut backup = NamedTempFile::new_in(&recovery)?;
        backup.write_all(&bytes)?;
        backup.as_file().sync_all()?;
        if self.working_file_text(&root, file)?.token != token {
            return Err(AppError::new(
                "stale_preview",
                "The working file changed before saving. Your draft is preserved.",
            ));
        }
        backup
            .keep()
            .map_err(|e| AppError::new("io", e.error.to_string()))?;
        output
            .persist(&absolute)
            .map_err(|e| AppError::new("io", e.error.to_string()))?;
        Ok(())
    }
}
