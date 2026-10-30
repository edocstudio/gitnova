use super::*;
use std::io::{Read, Write};
fn normalized(profile: &HostProfile) -> Result<HostProfile> {
    let c = gitnova_hosting::host_context(profile)?;
    Ok(HostProfile {
        provider: c.provider,
        origin: c.origin,
    })
}
fn same(a: &HostProfile, b: &HostProfile) -> bool {
    a.provider == b.provider && a.origin == b.origin
}
pub(super) fn load(path: &Path) -> Result<Vec<ConnectionCommitIdentity>> {
    let file = match std::fs::File::open(path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err(AppError::new(
            "commit_identity",
            "Commit identity settings exceed their size limit.",
        ));
    }
    let mut rows: Vec<ConnectionCommitIdentity> = serde_json::from_slice(&bytes).map_err(|_| {
        AppError::new(
            "commit_identity",
            "Commit identity settings could not be read.",
        )
    })?;
    if rows.len() > 20 {
        return Err(AppError::new(
            "commit_identity",
            "At most 20 commit identities are supported.",
        ));
    }
    for row in &mut rows {
        row.profile = normalized(&row.profile)?;
        gitnova_git::commit_identity::validate_identity(&row.identity)?;
    }
    for (i, row) in rows.iter().enumerate() {
        if rows[..i]
            .iter()
            .any(|other| same(&row.profile, &other.profile))
        {
            return Err(AppError::new(
                "commit_identity",
                "Duplicate host commit identities require inspection.",
            ));
        }
    }
    Ok(rows)
}
impl Application {
    fn identities_path(&self) -> PathBuf {
        self.receipt_dir
            .parent()
            .unwrap()
            .join("commit-identities.json")
    }
    pub fn connection_commit_identity(
        &self,
        profile: &HostProfile,
    ) -> Result<Option<GitCommitIdentity>> {
        let profile = normalized(profile)?;
        let rows = self
            .commit_identities
            .lock()
            .map_err(|_| AppError::new("state", "Commit identities are unavailable."))?;
        Ok(rows
            .iter()
            .find(|v| same(&v.profile, &profile))
            .map(|v| v.identity.clone()))
    }
    pub fn set_connection_commit_identity(
        &self,
        profile: &HostProfile,
        identity: Option<GitCommitIdentity>,
    ) -> Result<()> {
        let profile = normalized(profile)?;
        if let Some(i) = &identity {
            gitnova_git::commit_identity::validate_identity(i)?;
        }
        let mut rows = self
            .commit_identities
            .lock()
            .map_err(|_| AppError::new("state", "Commit identities are unavailable."))?;
        let mut next = rows.clone();
        next.retain(|v| !same(&v.profile, &profile));
        if let Some(identity) = identity {
            next.push(ConnectionCommitIdentity { profile, identity });
        }
        if next.len() > 20 {
            return Err(AppError::new(
                "commit_identity",
                "At most 20 commit identities are supported.",
            ));
        }
        let bytes = serde_json::to_vec_pretty(&next)
            .map_err(|e| AppError::new("storage", e.to_string()))?;
        if bytes.len() > 32768 {
            return Err(AppError::new(
                "commit_identity",
                "Commit identity settings exceed their size limit.",
            ));
        }
        let path = self.identities_path();
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(&bytes)?;
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist(path)
            .map_err(|e| AppError::new("storage", e.to_string()))?;
        *rows = next;
        Ok(())
    }
    fn selected_commit_identity(&self, root: &Path) -> Result<Option<ConnectionCommitIdentity>> {
        let rows = self
            .commit_identities
            .lock()
            .map_err(|_| AppError::new("state", "Commit identities are unavailable."))?
            .clone();
        if rows.is_empty() {
            return Ok(None);
        }
        let Some(url) = self.git.identity_remote_url(root)? else {
            return Ok(None);
        };
        let mut matches = rows
            .into_iter()
            .filter(|row| gitnova_hosting::matches_host(&row.profile, &url).unwrap_or(false))
            .collect::<Vec<_>>();
        matches.sort_by_key(|r| std::cmp::Reverse(r.profile.origin.len()));
        if matches.len() > 1
            && (!url.starts_with("https://")
                || matches[0].profile.origin == matches[1].profile.origin)
        {
            return Err(AppError::new(
                "commit_identity",
                "More than one connection matches this commit identity.",
            ));
        }
        Ok(matches.into_iter().next())
    }
    pub(super) fn with_repo_identity<T>(
        &self,
        membership: &str,
        work: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let root = membership_path(&self.catalog()?, membership)?;
        if gitnova_git::commit_identity::is_scoped(&root) {
            return work();
        }
        let identity = self
            .selected_commit_identity(&root)?
            .map(|row| row.identity);
        gitnova_git::commit_identity::with_commit_identity(&root, identity, work)
    }
    pub fn commit_identity(&self, membership: &str) -> Result<CommitIdentityContext> {
        let root = membership_path(&self.catalog()?, membership)?;
        let row = self.selected_commit_identity(&root)?;
        let connection = row.as_ref().map(|r| r.profile.clone());
        gitnova_git::commit_identity::with_commit_identity(&root, row.map(|r| r.identity), || {
            Ok(CommitIdentityContext {
                identity: self.git.effective_commit_identity(&root)?,
                connection,
            })
        })
    }
}
