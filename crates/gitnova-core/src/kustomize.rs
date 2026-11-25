//! Read the Git blobs needed by a local overlay, then run the standard renderer
//! in a private directory. Only the selected container images leave this module.
use crate::Application;
use gitnova_domain::{AppError, Result};
use gitnova_git::GitCli;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    ffi::OsStr,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mapping {
    reference: String,
    overlays: BTreeMap<String, String>,
    container: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overlay {
    config_updated_at: Option<String>,
    image: Option<String>,
    error: Option<String>,
}
#[derive(Serialize)]
pub struct Images {
    revision: String,
    overlays: BTreeMap<String, Overlay>,
}
fn error(msg: &str) -> AppError {
    AppError::new("kustomize", msg)
}

// Resolve paths in Git's portable namespace before creating native paths.
fn relative(dir: &str, path: &str) -> Result<String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(['\\', ':', '?', '#', '%'])
        || path.chars().any(char::is_control)
    {
        return Err(error("Use a repository-relative local Kustomize path."));
    }
    let mut parts: Vec<_> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in path.split('/') {
        match part {
            "." | "" => (),
            ".." => {
                parts
                    .pop()
                    .ok_or_else(|| error("Overlay path leaves the deploy repository."))?;
            }
            p if p.eq_ignore_ascii_case(".git") => return Err(error("Invalid overlay path.")),
            p if p.ends_with(['.', ' '])
                || matches!(
                    p.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "CONIN$"
                        | "CONOUT$"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                ) =>
            {
                return Err(error("Invalid overlay path."))
            }
            p => parts.push(p),
        }
    }
    Ok(parts.join("/"))
}

struct Snapshot<'a> {
    git: &'a GitCli,
    repo: &'a Path,
    dest: &'a Path,
    blobs: &'a HashMap<String, String>,
    written: HashSet<String>,
    directories: HashSet<String>,
    bytes: usize,
}
impl Snapshot<'_> {
    fn file(&mut self, name: &str) -> Result<PathBuf> {
        let oid = self
            .blobs
            .get(name)
            .ok_or_else(|| error("A referenced file is missing or is not a regular Git file."))?;
        let dest = self.dest.join(name);
        if self.written.insert(name.to_owned()) {
            let blob = self.git.read(
                self.repo,
                &[OsStr::new("cat-file"), OsStr::new("blob"), OsStr::new(oid)],
                4 * 1024 * 1024,
            )?;
            if blob.truncated {
                return Err(error("Overlay exceeds the local render limit."));
            }
            let bytes = blob.bytes;
            self.bytes += bytes.len();
            if self.bytes > 32 * 1024 * 1024 || self.written.len() > 2048 {
                return Err(error("Overlay exceeds the local render limit."));
            }
            fs::create_dir_all(
                dest.parent()
                    .ok_or_else(|| error("Invalid overlay path."))?,
            )
            .map_err(|_| error("Could not prepare overlay files."))?;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&dest)
                .and_then(|mut file| file.write_all(&bytes))
                .map_err(|_| error("Could not prepare overlay files."))?;
        }
        Ok(dest)
    }
    fn directory(&mut self, dir: &str, depth: usize) -> Result<()> {
        if depth > 48 {
            return Err(error("Overlay nesting is too deep."));
        }
        if !self.directories.insert(dir.to_owned()) {
            return Ok(());
        }
        let name = ["kustomization.yaml", "kustomization.yml", "Kustomization"]
            .into_iter()
            .map(|n| relative(dir, n))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|n| self.blobs.contains_key(n))
            .collect::<Vec<_>>();
        if name.len() != 1 {
            return Err(error(
                "Expected one kustomization file in the selected overlay or base.",
            ));
        }
        let input = fs::read(self.file(&name[0])?).map_err(|_| error("Could not read overlay."))?;
        let k: Value =
            serde_yaml_ng::from_slice(&input).map_err(|_| error("Invalid kustomization YAML."))?;
        for key in [
            "resources",
            "bases",
            "components",
            "generators",
            "transformers",
            "validators",
        ] {
            for item in k[key].as_sequence().into_iter().flatten() {
                let path = relative(
                    dir,
                    item.as_str()
                        .ok_or_else(|| error("Invalid resource path."))?,
                )?;
                if self.blobs.contains_key(&path) {
                    self.file(&path)?;
                } else {
                    self.directory(&path, depth + 1)?;
                }
            }
        }
        let mut files = Vec::new();
        for key in ["crds", "configurations", "patchesStrategicMerge"] {
            for value in k[key]
                .as_sequence()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if key != "patchesStrategicMerge" || !value.contains('\n') {
                    files.push(value);
                }
            }
        }
        for key in ["patches", "patchesJson6902", "replacements"] {
            for item in k[key].as_sequence().into_iter().flatten() {
                if let Some(path) = item["path"].as_str() {
                    files.push(path);
                }
            }
        }
        if let Some(path) = k["openapi"]["path"].as_str() {
            files.push(path);
        }
        for key in ["configMapGenerator", "secretGenerator"] {
            for item in k[key].as_sequence().into_iter().flatten() {
                for file in item["files"]
                    .as_sequence()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    files.push(file.split_once('=').map_or(file, |(_, p)| p));
                }
                files.extend(
                    item["envs"]
                        .as_sequence()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str),
                );
                files.extend(item["env"].as_str());
            }
        }
        for path in files {
            self.file(&relative(dir, path)?)?;
        }
        Ok(())
    }
}
fn renderer() -> Result<(PathBuf, bool)> {
    let mut dirs = std::env::var_os("PATH")
        .map(|v| std::env::split_paths(&v).collect::<Vec<_>>())
        .unwrap_or_default();
    if cfg!(target_os = "macos") {
        dirs.extend([
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ]);
    }
    for tool in ["kustomize", "kubectl"] {
        for dir in dirs.iter().filter(|d| d.is_absolute()) {
            let binary = dir.join(format!("{tool}{}", std::env::consts::EXE_SUFFIX));
            if binary.is_file() {
                return Ok((binary, tool == "kubectl"));
            }
        }
    }
    Err(error(
        "Install kubectl or kustomize to read overlay images.",
    ))
}
fn render(dir: &Path, container: &str) -> Result<String> {
    let (binary, kubectl) = renderer()?;
    let mut cmd = Command::new(binary);
    cmd.env_clear().current_dir(dir).env("PATH", "").env(
        "KUBECONFIG",
        if cfg!(windows) { "NUL" } else { "/dev/null" },
    );
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    let mut child = cmd
        .arg(if kubectl { "kustomize" } else { "build" })
        .arg(".")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| error("Could not start the Kustomize renderer."))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| error("Renderer output unavailable."))?;
    let reader = thread::spawn(move || {
        let mut data = Vec::new();
        stdout
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut data)
            .map(|_| data)
    });
    let start = Instant::now();
    let status = loop {
        if let Ok(Some(status)) = child.try_wait() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error("Kustomize render timed out."));
        }
        thread::sleep(Duration::from_millis(25));
    };
    let data = reader
        .join()
        .ok()
        .and_then(|r| r.ok())
        .ok_or_else(|| error("Could not read renderer output."))?;
    if data.len() > 8 * 1024 * 1024 {
        return Err(error("Overlay exceeds the local render limit."));
    }
    if !status.success() {
        return Err(error("Kustomize could not render this local overlay. Check its bases, patches and generator inputs."));
    }
    let mut images = BTreeSet::new();
    for doc in serde_yaml_ng::Deserializer::from_slice(&data) {
        let v = Value::deserialize(doc).map_err(|_| error("Invalid rendered YAML."))?;
        let spec = match v["kind"].as_str() {
            Some("CronJob") => &v["spec"]["jobTemplate"]["spec"]["template"]["spec"],
            Some("Pod") => &v["spec"],
            Some(
                "Deployment"
                | "StatefulSet"
                | "DaemonSet"
                | "ReplicaSet"
                | "ReplicationController"
                | "Job"
                | "Rollout",
            ) => &v["spec"]["template"]["spec"],
            _ => continue,
        };
        for c in spec["containers"].as_sequence().into_iter().flatten() {
            if c["name"].as_str() == Some(container) {
                if let Some(img) = c["image"].as_str() {
                    images.insert(img.to_owned());
                }
            }
        }
    }
    if images.len() > 1 {
        return Err(error(
            "The selected container resolves to multiple images in this overlay.",
        ));
    }
    images
        .pop_first()
        .ok_or_else(|| error("The selected container was not found in the rendered workloads."))
}

impl Application {
    pub fn kustomize_images(&self, membership: &str, mapping: &Mapping) -> Result<Images> {
        if mapping.overlays.len() > 32 {
            return Err(error("Too many release environments."));
        }
        let repo = self.path(membership)?;
        let reference = &mapping.reference;
        if !(reference.starts_with("refs/heads/") || reference.starts_with("refs/remotes/"))
            || mapping.container.trim().is_empty()
        {
            return Err(error("Select a deploy branch and container."));
        }
        self.git.text(&repo, &["check-ref-format", reference])?;
        let revision = self
            .git
            .text(
                &repo,
                &[
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("{reference}^{{commit}}"),
                ],
            )?
            .trim()
            .to_owned();
        let list = self.git.read(
            &repo,
            &[
                OsStr::new("ls-tree"),
                OsStr::new("-rz"),
                OsStr::new(&revision),
            ],
            16 * 1024 * 1024,
        )?;
        if list.truncated {
            return Err(error(
                "Deploy repository tree exceeds the local render limit.",
            ));
        }
        let blobs = list
            .bytes
            .split(|b| *b == 0)
            .filter_map(|row| {
                let (meta, path) = std::str::from_utf8(row).ok()?.split_once('\t')?;
                let mut fields = meta.split_whitespace();
                if !matches!(fields.next(), Some("100644" | "100755")) {
                    return None;
                }
                fields.next()?;
                Some((path.to_owned(), fields.next()?.to_owned()))
            })
            .collect();
        let extract = |path: &str| -> Result<Overlay> {
            let temp =
                tempfile::tempdir().map_err(|_| error("Could not prepare overlay files."))?;
            let dir = relative("", path)?;
            let mut snapshot = Snapshot {
                git: &self.git,
                repo: &repo,
                dest: temp.path(),
                blobs: &blobs,
                written: HashSet::new(),
                directories: HashSet::new(),
                bytes: 0,
            };
            snapshot.directory(&dir, 0)?;
            let image = render(&temp.path().join(dir), &mapping.container)?;
            // Include the exact inputs used from this revision, including shared bases and patches.
            let mut history = vec![
                "log",
                "-1",
                "--no-show-signature",
                "--format=%cI",
                revision.as_str(),
                "--",
            ];
            history.extend(snapshot.written.iter().map(String::as_str));
            let config_updated_at = self
                .git
                .text(&repo, &history)
                .ok()
                .map(|date| date.trim().to_owned())
                .filter(|date| !date.is_empty());
            Ok(Overlay {
                image: Some(image),
                config_updated_at,
                error: None,
            })
        };
        let overlay = |path: &str| {
            extract(path).unwrap_or_else(|e| Overlay {
                image: None,
                config_updated_at: None,
                error: Some(e.message),
            })
        };
        Ok(Images {
            overlays: mapping
                .overlays
                .iter()
                .map(|(name, path)| (name.clone(), overlay(path)))
                .collect(),
            revision,
        })
    }
}
