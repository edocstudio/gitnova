//! Inspect filter configuration without invoking filters, including child-only drivers.
use crate::{path_from_bytes, GitCli};
use gitnova_domain::*;
use std::{
    collections::{BTreeSet, HashSet, VecDeque},
    ffi::OsStr,
    path::{Component, Path},
};
impl GitCli {
    pub(crate) fn filter_drivers(&self, path: &Path) -> Result<BTreeSet<String>> {
        let args = [
            OsStr::new("config"),
            OsStr::new("--null"),
            OsStr::new("--name-only"),
            OsStr::new("--get-regexp"),
            OsStr::new(r"^filter\..*\.(clean|process|required)$"),
        ];
        let configured = match self.execute(path, &args, 65536, false) {
            Ok(out) => out.bytes,
            Err(error) if error.code == "git_exit_1" => vec![],
            Err(error) => return Err(error),
        };
        let mut drivers = BTreeSet::new();
        for key in configured
            .split(|byte| *byte == 0)
            .filter(|key| !key.is_empty())
        {
            let key = std::str::from_utf8(key).map_err(|_| {
                AppError::new("config", "Cannot safely decode filter configuration")
            })?;
            if let Some((driver, _)) = key.rsplit_once('.') {
                drivers.insert(driver.to_owned());
            }
        }
        Ok(drivers)
    }
    pub(crate) fn nested_filter_drivers(
        &self,
        path: &Path,
        drivers: &mut BTreeSet<String>,
    ) -> Result<()> {
        let mut queue = VecDeque::from([(path.to_path_buf(), 0usize)]);
        let mut seen = HashSet::new();
        while let Some((candidate, depth)) = queue.pop_front() {
            // Config and index inspection do not read working-file contents or execute drivers.
            let root = self.path(&candidate, "--show-toplevel")?;
            if !seen.insert(root.clone()) {
                continue;
            }
            if seen.len() > 257 || depth > 16 {
                return Err(AppError::new(
                    "safety_budget",
                    "Nested filter inspection exceeds its safe traversal budget.",
                ));
            }
            if depth > 0 {
                drivers.extend(self.filter_drivers(&root)?);
            }
            let index = self
                .read(
                    &root,
                    &[
                        OsStr::new("ls-files"),
                        OsStr::new("--stage"),
                        OsStr::new("-z"),
                    ],
                    8 * 1024 * 1024,
                )?
                .bytes;
            for entry in index
                .split(|byte| *byte == 0)
                .filter(|entry| entry.starts_with(b"160000 "))
            {
                let tab = entry
                    .iter()
                    .position(|byte| *byte == b'\t')
                    .ok_or_else(|| AppError::new("git_parse", "Invalid submodule index entry"))?;
                let relative = path_from_bytes(entry[tab + 1..].to_vec())?;
                if relative.as_os_str().is_empty()
                    || relative.components().any(|part| {
                        !matches!(part, Component::Normal(_))
                            || part.as_os_str().eq_ignore_ascii_case(OsStr::new(".git"))
                    })
                {
                    return Err(AppError::new(
                        "path",
                        "Unsafe submodule path during read inspection.",
                    ));
                }
                let mut child = root.clone();
                for part in relative.components() {
                    child.push(part);
                    if std::fs::symlink_metadata(&child)
                        .is_ok_and(|meta| meta.file_type().is_symlink())
                    {
                        return Err(AppError::new(
                            "path",
                            "Inspect symlinked submodule paths with Git before reading them here.",
                        ));
                    }
                }
                if std::fs::symlink_metadata(child.join(".git")).is_ok() {
                    queue.push_back((child, depth + 1));
                }
            }
        }
        Ok(())
    }
}
