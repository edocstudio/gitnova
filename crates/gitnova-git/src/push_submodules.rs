//! Bounded offline evidence for historical gitlinks, never a live publication claim.
use crate::{GitCli, RepositoryReader};
use gitnova_domain::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    path::Path,
};
const REACHABILITY_LIMIT: usize = 10_000;
struct ChildPushEvidence {
    available: Vec<bool>,
    missing: BTreeSet<String>,
    complete: bool,
}

impl GitCli {
    // One object batch and one graph traversal per child, independent of how many
    // historical gitlinks in that child the parent is about to publish.
    fn child_push_evidence(&self, path: &Path, oids: &[String]) -> Result<ChildPushEvidence> {
        let snapshot = || {
            self.text(
                path,
                &[
                    "for-each-ref",
                    "--format=%(refname)%00%(objectname)",
                    "refs/remotes/",
                ],
            )
        };
        let before = snapshot()?;
        let input = oids
            .iter()
            .map(|oid| format!("{oid}\n"))
            .collect::<String>();
        let args = [
            OsStr::new("cat-file"),
            OsStr::new("--batch-check=%(objectname) %(objecttype)"),
        ];
        let raw = self
            .read_input(path, &args, input.as_bytes(), 32 * 1024)?
            .bytes;
        let text = String::from_utf8(raw)
            .map_err(|_| AppError::new("git_parse", "Invalid child object batch."))?;
        let records = text.lines().collect::<Vec<_>>();
        if records.len() != oids.len() {
            return Err(AppError::new("git_parse", "Incomplete child object batch."));
        }
        let mut available = vec![];
        for (oid, record) in oids.iter().zip(records) {
            let fields = record.split_whitespace().collect::<Vec<_>>();
            if fields.len() != 2 || fields[0] != oid {
                return Err(AppError::new(
                    "git_parse",
                    "Child object batch identity changed.",
                ));
            }
            available.push(fields[1] == "commit");
        }
        let mut missing = BTreeSet::new();
        let mut complete = true;
        if available.iter().any(|value| *value) {
            let mut args = vec![
                OsString::from("rev-list"),
                format!("--max-count={}", REACHABILITY_LIMIT + 1).into(),
            ];
            args.extend(
                oids.iter()
                    .zip(&available)
                    .filter(|(_, exists)| **exists)
                    .map(|(oid, _)| OsString::from(oid)),
            );
            args.extend(["--not".into(), "--remotes".into()]);
            let raw = self
                .read(
                    path,
                    &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                    1024 * 1024,
                )?
                .bytes;
            let text = String::from_utf8(raw)
                .map_err(|_| AppError::new("git_parse", "Invalid child reachability result."))?;
            for oid in text.lines() {
                crate::validate_oid(oid)?;
                missing.insert(oid.to_owned());
            }
            complete = missing.len() <= REACHABILITY_LIMIT;
        }
        if before != snapshot()? {
            return Err(AppError::new(
                "stale_preview",
                "Child remote refs changed during inspection. Recheck.",
            ));
        }
        Ok(ChildPushEvidence {
            available,
            missing,
            complete,
        })
    }

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
        let mut candidates = vec![];
        let mut groups = BTreeMap::<String, Vec<usize>>::new();
        let mut report = PushSubmoduleReport {
            issues: vec![],
            complete: preview.outgoing_count <= 500,
        };
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
            groups
                .entry(native.hex.clone())
                .or_default()
                .push(candidates.len());
            candidates.push(PushSubmoduleIssue {
                evidence: PushSubmoduleEvidence::Unknown,
                path: native,
                oid: oid.into(),
                parent_oid: parent.clone(),
                current_oid: child.and_then(|c| c.checked_out_oid.clone()),
                can_open: false,
                object_available: false,
            });
        }
        if candidates.is_empty() {
            return Ok(report);
        }
        let root_identity = self.discover(root)?;
        let mut keep = vec![true; candidates.len()];
        for indices in groups.values() {
            let native = &candidates[indices[0]].path;
            let Some(child) = children.iter().find(|child| {
                child.path == *native
                    && child.depth == 0
                    && child.initialized
                    && child.inspection_error.is_none()
            }) else {
                report.complete = false;
                continue;
            };
            let path = root.join(child.path.to_path()?);
            let identity = self.discover(&path);
            if !identity.is_ok_and(|identity| {
                identity.common_dir != root_identity.common_dir
                    && path.canonicalize().ok().as_ref() == identity.root.as_ref()
            }) {
                report.complete = false;
                continue;
            }
            for index in indices {
                candidates[*index].can_open = true;
            }
            let oids = indices
                .iter()
                .map(|index| candidates[*index].oid.clone())
                .collect::<Vec<_>>();
            match self.child_push_evidence(&path, &oids) {
                Ok(ChildPushEvidence {
                    available,
                    missing,
                    complete,
                }) => {
                    report.complete &= complete;
                    for (index, exists) in indices.iter().zip(available) {
                        let issue = &mut candidates[*index];
                        issue.object_available = exists;
                        if !exists {
                            report.complete = false;
                        } else if missing.contains(&issue.oid) {
                            issue.evidence = PushSubmoduleEvidence::Missing;
                        } else if complete {
                            keep[*index] = false;
                        }
                    }
                }
                Err(_) => report.complete = false,
            }
        }
        report.issues = candidates
            .into_iter()
            .zip(keep)
            .filter_map(|(issue, keep)| keep.then_some(issue))
            .collect();
        Ok(report)
    }
}
