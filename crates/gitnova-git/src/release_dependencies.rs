//! Read committed gitlink relationships; inspect only explicitly registered children.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
fn err(s: &str) -> AppError {
    AppError::new("investigation", s)
}
fn pins(git: &GitCli, root: &Path, oid: &str) -> Result<BTreeMap<PathBuf, String>> {
    validate_oid(oid)?;
    let raw = git
        .read(
            root,
            &[
                "ls-tree".as_ref(),
                "-r".as_ref(),
                "-z".as_ref(),
                oid.as_ref(),
            ],
            4 * 1024 * 1024,
        )?
        .bytes;
    let mut map = BTreeMap::new();
    for field in raw.split(|b| *b == 0).filter(|f| f.starts_with(b"160000 ")) {
        let tab = field
            .iter()
            .position(|b| *b == b'\t')
            .ok_or_else(|| err("Invalid dependency tree entry."))?;
        let header =
            std::str::from_utf8(&field[..tab]).map_err(|_| err("Invalid dependency revision."))?;
        let revision = header
            .split_whitespace()
            .nth(2)
            .ok_or_else(|| err("Invalid dependency revision."))?;
        validate_oid(revision)?;
        map.insert(
            crate::path_from_bytes(field[tab + 1..].to_vec())?,
            revision.into(),
        );
    }
    Ok(map)
}
fn urls(git: &GitCli, root: &Path, revision: Option<&str>) -> Result<BTreeMap<PathBuf, String>> {
    let mut args: Vec<OsString> = ["config", "--no-includes", "--null"]
        .into_iter()
        .map(Into::into)
        .collect();
    if let Some(oid) = revision {
        args.extend(["--blob".into(), format!("{oid}:.gitmodules").into()]);
    } else {
        args.extend(["--file".into(), ".gitmodules".into()]);
    }
    args.extend(["--get-regexp".into(), r"^submodule\..*\.(path|url)$".into()]);
    let bytes = git
        .read(
            root,
            &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            131072,
        )?
        .bytes;
    let mut fields = BTreeMap::new();
    for record in bytes.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let i = record
            .iter()
            .position(|b| *b == b'\n')
            .ok_or_else(|| err("Invalid dependency manifest."))?;
        let key =
            std::str::from_utf8(&record[..i]).map_err(|_| err("Non-UTF-8 dependency manifest."))?;
        let value = std::str::from_utf8(&record[i + 1..])
            .map_err(|_| err("Non-UTF-8 dependency manifest."))?;
        if fields.insert(key.to_string(), value.to_string()).is_some() {
            return Err(err("Ambiguous dependency manifest."));
        }
    }
    let mut map = BTreeMap::new();
    for (key, path) in &fields {
        if let Some(name) = key.strip_suffix(".path") {
            let url = fields
                .get(&format!("{name}.url"))
                .ok_or_else(|| err("Dependency URL is missing."))?;
            if map.insert(PathBuf::from(path), url.clone()).is_some() {
                return Err(err("Ambiguous dependency paths."));
            }
        }
    }
    Ok(map)
}
impl GitCli {
    pub fn release_dependencies(
        &self,
        root: &Path,
        source: &str,
        target: &str,
        registered: &[(SearchLocation, PathBuf)],
    ) -> Result<Vec<DependencyEvidence>> {
        let before = self.fingerprint(root)?;
        let a = pins(self, root, source)?;
        let b = pins(self, root, target)?;
        let paths = a.keys().chain(b.keys()).cloned().collect::<BTreeSet<_>>();
        if paths.len() > 32 {
            return Err(err(
                "Dependency evidence exceeds 32 gitlinks per repository. Narrow the release scope.",
            ));
        }
        if paths.is_empty() {
            return Ok(vec![]);
        }
        let au = urls(self, root, Some(source));
        let bu = urls(self, root, Some(target));
        let current = urls(self, root, None);
        let mut rows = vec![];
        let started = std::time::Instant::now();
        for path in paths {
            let mut row = DependencyEvidence {
                path: NativePath::new(&path),
                source_oid: a.get(&path).cloned(),
                target_oid: b.get(&path).cloned(),
                location: None,
                relation: "unknown".into(),
                source_only: None,
                target_only: None,
                cached_remote_contains: None,
                issue: None,
            };
            if row.source_oid == row.target_oid {
                row.relation = "unchanged".into();
                rows.push(row);
                continue;
            }
            let result = (|| -> Result<()> {
                if started.elapsed().as_secs() >= 20 {
                    return Err(err(
                        "Dependency investigation time budget reached. Narrow the scope.",
                    ));
                }
                crate::working::relative(&row.path)?;
                let needed = if row.source_oid.is_some() {
                    au.as_ref()
                } else {
                    bu.as_ref()
                }
                .map_err(|_| {
                    err("Read the committed dependency manifest before assessing this pointer.")
                })?;
                let expected = needed
                    .get(&path)
                    .ok_or_else(|| err("No unambiguous versioned URL for this dependency."))?;
                if let (Some(left), Some(right)) = (
                    au.as_ref().ok().and_then(|m| m.get(&path)),
                    bu.as_ref().ok().and_then(|m| m.get(&path)),
                ) {
                    if left != right {
                        return Err(err("Dependency URL changed between branches. Review repository identity separately."));
                    }
                }
                if current.as_ref().ok().and_then(|m| m.get(&path)) != Some(expected) {
                    return Err(err("The checked-out dependency registration differs from these tips. Inspect that branch separately."));
                }
                let mut child = root.to_path_buf();
                for part in path.components() {
                    child.push(part);
                    if std::fs::symlink_metadata(&child)?.file_type().is_symlink() {
                        return Err(err(
                            "Symbolic-link dependency paths need manual inspection.",
                        ));
                    }
                }
                let canonical = child.canonicalize()?;
                let d = self.discover(&canonical)?;
                if d.root.as_ref() != Some(&canonical) {
                    return Err(err("Dependency checkout identity is unavailable."));
                }
                let location=registered.iter().find(|(l,common)|l.path.to_path().ok().and_then(|p|p.canonicalize().ok()).as_ref()==Some(&canonical)&&*common==d.common_dir)
                    .ok_or_else(||err("Open this dependency in the workspace to establish its repository identity."))?;
                row.location = Some(location.0.clone());
                let snapshot = self.fingerprint(&canonical)?;
                if self
                    .text(&canonical, &["rev-parse", "--is-shallow-repository"])?
                    .trim()
                    == "true"
                {
                    return Err(err(
                        "Shallow dependency history cannot establish complete ancestry.",
                    ));
                }
                for oid in [&row.source_oid, &row.target_oid].into_iter().flatten() {
                    if self.text(&canonical, &["cat-file", "-t", oid])?.trim() != "commit" {
                        return Err(err("Dependency commit objects are unavailable locally."));
                    }
                }
                if let (Some(source), Some(target)) = (&row.source_oid, &row.target_oid) {
                    let counts = self.text(
                        &canonical,
                        &[
                            "rev-list",
                            "--left-right",
                            "--count",
                            &format!("{source}...{target}"),
                            "--",
                        ],
                    )?;
                    let counts = counts
                        .split_whitespace()
                        .map(str::parse::<usize>)
                        .collect::<std::result::Result<Vec<_>, _>>()
                        .map_err(|_| err("Invalid dependency ancestry counts."))?;
                    if counts.len() != 2 {
                        return Err(err("Invalid dependency ancestry counts."));
                    }
                    row.source_only = Some(counts[0]);
                    row.target_only = Some(counts[1]);
                    row.relation = if counts[1] == 0 {
                        "forward"
                    } else if counts[0] == 0 {
                        "rollback"
                    } else {
                        "diverged"
                    }
                    .into();
                } else {
                    row.relation = if row.source_oid.is_some() {
                        "added"
                    } else {
                        "removed"
                    }
                    .into();
                }
                if let Some(oid) = &row.source_oid {
                    row.cached_remote_contains = Some(
                        !self
                            .text(
                                &canonical,
                                &[
                                    "for-each-ref",
                                    &format!("--contains={oid}"),
                                    "--format=%(refname)",
                                    "refs/remotes/",
                                ],
                            )?
                            .trim()
                            .is_empty(),
                    );
                }
                if self.fingerprint(&canonical)? != snapshot {
                    return Err(err(
                        "Dependency refs changed during investigation. Search again.",
                    ));
                }
                Ok(())
            })();
            if let Err(e) = result {
                row.relation = "unknown".into();
                row.issue = Some(e.message);
            }
            rows.push(row);
        }
        if self.fingerprint(root)? != before {
            return Err(err("Repository refs changed. Run the investigation again."));
        }
        Ok(rows)
    }
}
