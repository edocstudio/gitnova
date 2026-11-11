use super::{denied, invalid};
use gitnova_domain::{AppError, Result};
use gitnova_git::{GitCli, RepositoryReader};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    pub repositories: Vec<Grant>,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Grant {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
    stamps: Vec<String>,
}
fn stamp(path: &Path) -> Result<String> {
    let m = fs::metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("{}:{}", m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        Ok(format!("{:?}", m.created()?))
    }
}
impl Grant {
    fn discover(root: &Path, id: String) -> Result<Self> {
        let root = root.canonicalize()?;
        let d = GitCli.discover(&root)?;
        // An explicit checkout root is required; no implicit ancestor grants.
        if d.root
            .as_ref()
            .is_some_and(|p| p.canonicalize().ok().as_ref() != Some(&root))
        {
            return Err(denied());
        }
        let git_dir = d.git_dir.canonicalize()?;
        let common_dir = d.common_dir.canonicalize()?;
        if d.root.is_none() && root != git_dir {
            return Err(denied());
        }
        let stamps = [&root, &git_dir, &common_dir]
            .iter()
            .map(|p| stamp(p))
            .collect::<Result<Vec<_>>>()?;
        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repository")
            .to_string();
        Ok(Self {
            id,
            name,
            root,
            git_dir,
            common_dir,
            stamps,
        })
    }
    pub fn verify(&self) -> Result<()> {
        match Self::discover(&self.root, self.id.clone()) {
            Ok(now) if now == *self => Ok(()),
            _ => Err(denied()),
        }
    }
    pub fn path(&self, path: &str) -> Result<PathBuf> {
        if path.is_empty()
            || path.len() > 4096
            || path.contains(['\\', ':'])
            || path.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        let rel = Path::new(path);
        if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
            return Err(invalid());
        }
        let mut p = self.root.clone();
        for c in rel.components() {
            let name = c.as_os_str().to_string_lossy().to_ascii_lowercase();
            if name == ".git"
                || name.starts_with(".env")
                || matches!(
                    name.as_str(),
                    ".ssh" | ".aws" | ".gnupg" | "credentials" | "id_rsa" | "id_ed25519"
                )
                || name.ends_with(".pem")
                || name.ends_with(".key")
                || name.ends_with(".p12")
            {
                return Err(denied());
            }
            p.push(c);
            match fs::symlink_metadata(&p) {
                Ok(m) if m.file_type().is_symlink() => return Err(denied()),
                Ok(m)
                    if m.is_dir()
                        && (p.join(".git").exists()
                            || (p.join("HEAD").is_file()
                                && p.join("objects").is_dir()
                                && p.join("refs").is_dir())) =>
                {
                    return Err(denied())
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            if p.exists() && !p.canonicalize()?.starts_with(&self.root) {
                return Err(denied());
            }
        }
        // Index gitlinks protect absent/uninitialized submodules too. Symlink blobs
        // are not rendered as files, even if the checkout removed the symlink.
        let raw = super::read(
            &self.root,
            &["ls-files", "--stage", "-z", "--", path],
            256 * 1024,
        )?;
        for entry in raw.split(|b| *b == 0).filter(|b| !b.is_empty()) {
            if entry.starts_with(b"160000 ") || entry.starts_with(b"120000 ") {
                return Err(denied());
            }
        }
        let mut prefix = String::new();
        for part in path
            .split('/')
            .take(path.split('/').count().saturating_sub(1))
        {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let raw = super::read(
                &self.root,
                &["ls-files", "--stage", "-z", "--", &prefix],
                256 * 1024,
            )?;
            if raw.split(|b| *b == 0).any(|e| e.starts_with(b"160000 ")) {
                return Err(denied());
            }
        }
        Ok(p)
    }
}
impl Policy {
    pub fn load(path: &Path) -> Result<Self> {
        let m = match fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(_) => return Err(denied()),
        };
        if !m.is_file() || m.file_type().is_symlink() || m.len() > 65536 {
            return Err(denied());
        }
        let mut bytes = Vec::new();
        File::open(path)?.take(65537).read_to_end(&mut bytes)?;
        let p: Self = serde_json::from_slice(&bytes).map_err(|_| denied())?;
        if p.repositories.len() > 32
            || p.repositories.iter().any(|g| {
                g.id.len() > 64
                    || g.name.len() > 256
                    || !g.root.is_absolute()
                    || !g.git_dir.is_absolute()
                    || !g.common_dir.is_absolute()
            })
        {
            return Err(denied());
        }
        Ok(p)
    }
    fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().ok_or_else(invalid)?;
        fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(&serde_json::to_vec_pretty(self).map_err(|_| invalid())?)?;
        file.as_file().sync_all()?;
        file.persist(path)
            .map_err(|e| AppError::new("policy_write", e.error.to_string()))?;
        Ok(())
    }
}
/// Operator CLI only. Never expose as an MCP tool. Does not write Git state.
pub fn authorize(config: &Path, root: &Path) -> Result<String> {
    if !config.is_absolute() || !root.is_absolute() {
        return Err(invalid());
    }
    let mut p = Policy::load(config)?;
    let g = Grant::discover(root, uuid::Uuid::new_v4().to_string())?;
    if let Some(existing) = p.repositories.iter().find(|x| x.root == g.root) {
        existing.verify()?;
        return Ok(existing.id.clone());
    }
    if p.repositories.len() >= 32 {
        return Err(AppError::new(
            "policy_limit",
            "At most 32 explicit grants are supported.",
        ));
    }
    let id = g.id.clone();
    p.repositories.push(g);
    p.save(config)?;
    Ok(id)
}
pub fn revoke(config: &Path, id: &str) -> Result<()> {
    let mut p = Policy::load(config)?;
    p.repositories.retain(|g| g.id != id);
    p.save(config)
}
