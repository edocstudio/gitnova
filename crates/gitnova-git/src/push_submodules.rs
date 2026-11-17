//! Bounded offline evidence for historical gitlinks, never a live publication claim.
use crate::{GitCli, RepositoryReader};
use gitnova_domain::*;
use std::{collections::BTreeSet, ffi::OsStr, path::Path};

impl GitCli {
    pub fn push_submodule_report(
        &self,
        root: &Path,
        preview: &PushPreview,
        children: &[Submodule],
    ) -> Result<PushSubmoduleReport> {
        let range = preview.remote_oid.as_ref().map_or_else(
            || preview.request.oid.clone(),
            |oid| format!("{oid}..{}", preview.request.oid),
        );
        let args = [
            "log",
            "--no-show-signature",
            "--no-decorate",
            "--format=%H",
            "--raw",
            "--no-abbrev",
            "--no-renames",
            "-m",
            "--root",
            "-z",
            "--max-count=500",
            &range,
            "--",
        ];
        let output = self
            .read(
                root,
                &args.iter().map(OsStr::new).collect::<Vec<_>>(),
                8 * 1024 * 1024,
            )?
            .bytes;
        let mut fields = output.split(|byte| *byte == 0);
        let mut parent = String::new();
        let mut seen = BTreeSet::new();
        let mut report = PushSubmoduleReport {
            issues: vec![],
            complete: preview.outgoing_count <= 500,
        };
        let root_identity = self.discover(root)?;
        while let Some(field) = fields.next() {
            let field = field.strip_prefix(b"\n").unwrap_or(field);
            if field.is_empty() {
                continue;
            }
            if !field.starts_with(b":") {
                let oid = std::str::from_utf8(field)
                    .map_err(|_| AppError::new("git_parse", "Invalid submodule history record."))?;
                crate::validate_oid(oid)?;
                parent = oid.into();
                continue;
            }
            let path = fields
                .next()
                .ok_or_else(|| AppError::new("git_parse", "Missing historical submodule path."))?;
            let header = field.split(|byte| *byte == b' ').collect::<Vec<_>>();
            if header.len() != 5 {
                return Err(AppError::new(
                    "git_parse",
                    "Invalid historical submodule change.",
                ));
            }
            if header[1] != b"160000" {
                continue;
            }
            let relative = crate::path_from_bytes(path.to_vec())?;
            let native = NativePath::new(&relative);
            crate::working::relative(&native)?;
            let oid = std::str::from_utf8(header[3])
                .map_err(|_| AppError::new("git_parse", "Invalid submodule commit."))?;
            crate::validate_oid(oid)?;
            if !seen.insert((native.hex.clone(), oid.to_owned())) {
                continue;
            }
            if seen.len() > 64 {
                report.complete = false;
                break;
            }
            let child = children
                .iter()
                .find(|child| child.path == native && child.depth == 0);
            let mut issue = PushSubmoduleIssue {
                path: native,
                oid: oid.into(),
                parent_oid: parent.clone(),
                current_oid: child.and_then(|c| c.checked_out_oid.clone()),
                can_open: false,
                object_available: false,
            };
            if let Some(child) =
                child.filter(|child| child.initialized && child.inspection_error.is_none())
            {
                let path = root.join(child.path.to_path()?);
                let identity = self.discover(&path)?;
                if identity.common_dir == root_identity.common_dir
                    || identity.root.as_deref() != Some(path.canonicalize()?.as_path())
                {
                    report.complete = false;
                    report.issues.push(issue);
                    continue;
                }
                issue.can_open = true;
                if self
                    .text(&path, &["cat-file", "-e", &format!("{oid}^{{commit}}")])
                    .is_ok()
                {
                    issue.object_available = true;
                    let missing = self.text(
                        &path,
                        &["rev-list", "--max-count=1", oid, "--not", "--remotes"],
                    )?;
                    if missing.trim().is_empty() {
                        continue;
                    }
                } else {
                    report.complete = false;
                }
            } else {
                report.complete = false;
            }
            report.issues.push(issue);
        }
        Ok(report)
    }
}
