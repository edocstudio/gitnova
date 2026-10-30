//! Committed root package.json facts and explicit registered-repository matches.
use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
/// Disposable per-investigation lookup, never used by Git writes.
pub struct PackageRegistry {
    rows: Vec<(String, PackageLocationEvidence)>,
    incomplete: bool,
}
struct Manifest {
    name: Option<String>,
    version: Option<String>,
    requirements: BTreeMap<(String, String), String>,
    workspace: bool,
}
fn invalid(s: &str) -> AppError {
    AppError::new("investigation", s)
}
fn value(v: Option<&serde_json::Value>) -> Result<Option<String>> {
    match v {
        None => Ok(None),
        Some(serde_json::Value::String(s))
            if !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control) =>
        {
            Ok(Some(s.clone()))
        }
        _ => Err(invalid("Invalid bounded package manifest field.")),
    }
}
fn manifest(git: &GitCli, root: &Path, oid: &str) -> Result<Option<Manifest>> {
    validate_oid(oid)?;
    let entry = git
        .read(
            root,
            &[
                "--literal-pathspecs".as_ref(),
                "ls-tree".as_ref(),
                "-z".as_ref(),
                oid.as_ref(),
                "--".as_ref(),
                "package.json".as_ref(),
            ],
            65536,
        )?
        .bytes;
    if entry.is_empty() {
        return Ok(None);
    }
    if !(entry.starts_with(b"100644 ") || entry.starts_with(b"100755 "))
        || entry.iter().filter(|b| **b == 0).count() != 1
    {
        return Err(invalid(
            "Package evidence requires a regular root package.json.",
        ));
    }
    let header = std::str::from_utf8(
        &entry[..entry
            .iter()
            .position(|b| *b == b'\t')
            .ok_or_else(|| invalid("Invalid package tree entry."))?],
    )
    .map_err(|_| invalid("Invalid package tree entry."))?;
    let blob = header
        .split_whitespace()
        .nth(2)
        .ok_or_else(|| invalid("Invalid package revision."))?;
    validate_oid(blob)?;
    let bytes = git
        .read(
            root,
            &["cat-file".as_ref(), "blob".as_ref(), blob.as_ref()],
            65536,
        )?
        .bytes;
    let json: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| invalid("Invalid committed package.json."))?;
    let object = json
        .as_object()
        .ok_or_else(|| invalid("Package manifest must be an object."))?;
    let mut requirements = BTreeMap::new();
    for kind in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(deps) = object.get(kind) {
            let deps = deps
                .as_object()
                .ok_or_else(|| invalid("Package dependencies must be string maps."))?;
            for (name, v) in deps {
                if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
                    return Err(invalid("Invalid package dependency name."));
                }
                let constraint =
                    value(Some(v))?.ok_or_else(|| invalid("Missing dependency constraint."))?;
                requirements.insert((kind.into(), name.clone()), constraint);
                if requirements.len() > 100 {
                    return Err(invalid(
                        "Package evidence is limited to 100 declared requirements per manifest.",
                    ));
                }
            }
        }
    }
    Ok(Some(Manifest {
        name: value(object.get("name"))?,
        version: value(object.get("version"))?,
        requirements,
        workspace: object.contains_key("workspaces"),
    }))
}
impl GitCli {
    pub fn release_packages(
        &self,
        root: &Path,
        source: &str,
        target: &str,
        registered: &[(SearchLocation, PathBuf)],
        registry: &mut Option<PackageRegistry>,
    ) -> Result<Option<PackageManifestEvidence>> {
        let before = self.fingerprint(root)?;
        let a = manifest(self, root, source)?;
        let b = manifest(self, root, target)?;
        if a.is_none() && b.is_none() {
            return Ok(None);
        }
        let empty = BTreeMap::new();
        let ar = a.as_ref().map(|m| &m.requirements).unwrap_or(&empty);
        let br = b.as_ref().map(|m| &m.requirements).unwrap_or(&empty);
        let keys = ar.keys().chain(br.keys()).cloned().collect::<BTreeSet<_>>();
        let mut requirements = keys
            .into_iter()
            .filter(|key| ar.get(key) != br.get(key))
            .map(|(kind, name)| PackageRequirementEvidence {
                source: ar.get(&(kind.clone(), name.clone())).cloned(),
                target: br.get(&(kind.clone(), name.clone())).cloned(),
                kind,
                name,
                locations: vec![],
                issue: None,
            })
            .collect::<Vec<_>>();
        for row in &mut requirements {
            if [&row.source, &row.target]
                .into_iter()
                .flatten()
                .any(|value| {
                    value.contains(':')
                        || value.starts_with(['.', '/'])
                        || value.starts_with("git@")
                })
            {
                row.issue = Some(
                    "Alias, path, workspace and Git dependency constraints are not resolved."
                        .into(),
                );
            }
        }
        if requirements.len() > 100 {
            return Err(invalid(
                "Package evidence is limited to 100 changed requirements.",
            ));
        }
        if requirements.iter().any(|row| row.issue.is_none()) {
            if registry.is_none() {
                let mut rows = Vec::new();
                let mut incomplete = registered.len() > 32;
                let started = std::time::Instant::now();
                for (location, common) in registered.iter().take(32) {
                    if started.elapsed().as_secs() >= 5 {
                        incomplete = true;
                        break;
                    }
                    let read = (|| -> Result<Option<(String, PackageLocationEvidence)>> {
                        let path = location.path.to_path()?;
                        let d = self.discover(&path)?;
                        if d.common_dir != *common
                            || d.root
                                .as_ref()
                                .is_some_and(|r| fs::canonicalize(&path).ok().as_ref() != Some(r))
                        {
                            return Err(invalid("Registered package repository identity changed."));
                        }
                        let stamp = self.fingerprint(&path)?;
                        let oid = self
                            .text(&path, &["rev-parse", "--verify", "HEAD^{commit}"])?
                            .trim()
                            .to_owned();
                        validate_oid(&oid)?;
                        let m = manifest(self, &path, &oid)?;
                        if stamp != self.fingerprint(&path)? {
                            return Err(invalid("Registered package refs changed."));
                        }
                        Ok(m.and_then(|m| {
                            m.name.map(|name| {
                                (
                                    name,
                                    PackageLocationEvidence {
                                        location: location.clone(),
                                        oid,
                                        version: m.version,
                                    },
                                )
                            })
                        }))
                    })();
                    match read {
                        Ok(Some(row)) => rows.push(row),
                        Ok(None) => {}
                        Err(_) => incomplete = true,
                    }
                }
                *registry = Some(PackageRegistry { rows, incomplete });
            }
            let lookup = registry
                .as_ref()
                .ok_or_else(|| invalid("Package lookup is unavailable."))?;
            for row in requirements.iter_mut().filter(|row| row.issue.is_none()) {
                row.locations = lookup
                    .rows
                    .iter()
                    .filter(|(name, _)| *name == row.name)
                    .map(|(_, evidence)| evidence.clone())
                    .collect();
                let repositories = row
                    .locations
                    .iter()
                    .map(|l| &l.location.repository_id)
                    .collect::<BTreeSet<_>>();
                row.issue = if lookup.incomplete {
                    Some("Registered package lookup is incomplete; no unique dependency identity is proven.".into())
                } else if repositories.len() != 1 {
                    Some(
                        "No unique registered repository matches this committed package name."
                            .into(),
                    )
                } else {
                    None
                };
            }
        }
        let result=PackageManifestEvidence{source_name:a.as_ref().and_then(|m|m.name.clone()),target_name:b.as_ref().and_then(|m|m.name.clone()),source_version:a.as_ref().and_then(|m|m.version.clone()),target_version:b.as_ref().and_then(|m|m.version.clone()),requirements,issue:(a.as_ref().is_some_and(|m|m.workspace)||b.as_ref().is_some_and(|m|m.workspace)).then(||"Only root package.json is inspected; workspace package manifests and lockfile resolution remain unverified.".into())};
        if before != self.fingerprint(root)? {
            return Err(invalid(
                "Repository refs changed. Run the investigation again.",
            ));
        }
        Ok(Some(result))
    }
}
