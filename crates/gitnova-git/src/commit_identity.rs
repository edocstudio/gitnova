//! Scoped command configuration; never changes process environment or Git config files.
use gitnova_domain::*;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    process::Command,
};
thread_local! { static CURRENT: RefCell<Option<(PathBuf, Option<GitCommitIdentity>)>> = const { RefCell::new(None) }; }
pub fn with_commit_identity<T>(
    root: &Path,
    identity: Option<GitCommitIdentity>,
    work: impl FnOnce() -> T,
) -> T {
    struct Restore(Option<(PathBuf, Option<GitCommitIdentity>)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|v| *v.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(CURRENT.with(|v| v.replace(Some((root.to_owned(), identity)))));
    work()
}
pub(crate) fn configure(command: &mut Command, root: &Path) {
    CURRENT.with(|v| {
        if let Some((_, Some(identity))) = v.borrow().as_ref().filter(|(path, _)| path == root) {
            for prefix in ["user", "author", "committer"] {
                command.args([
                    "-c",
                    &format!("{prefix}.name={}", identity.name),
                    "-c",
                    &format!("{prefix}.email={}", identity.email),
                ]);
            }
        }
    });
}
pub fn validate_identity(identity: &GitCommitIdentity) -> Result<()> {
    if identity.name.trim().is_empty()
        || identity.name.len() > 256
        || identity
            .name
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>'))
        || identity.email.len() > 254
        || identity.email.is_empty()
        || !identity.email.contains('@')
        || identity
            .email
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>'))
    {
        return Err(AppError::new(
            "commit_identity",
            "Enter a commit name and a valid email without control characters.",
        ));
    }
    Ok(())
}

pub fn is_scoped(root: &Path) -> bool {
    CURRENT.with(|v| v.borrow().as_ref().is_some_and(|(path, _)| path == root))
}
impl crate::GitCli {
    pub fn identity_remote_url(&self, root: &Path) -> Result<Option<String>> {
        let names = self.text(root, &["remote"])?;
        let remotes = names.lines().collect::<Vec<_>>();
        let remote = if remotes.contains(&"origin") {
            "origin"
        } else if remotes.len() == 1 {
            remotes[0]
        } else if remotes.is_empty() {
            return Ok(None);
        } else {
            return Err(AppError::new(
                "commit_identity",
                "More than one remote may select a commit identity. Configure origin explicitly.",
            ));
        };
        let urls = self.text(root, &["remote", "get-url", "--all", "--", remote])?;
        if urls.lines().count() != 1 {
            return Err(AppError::new(
                "commit_identity",
                "More than one fetch URL may select a commit identity.",
            ));
        }
        Ok(Some(urls.trim_end().to_owned()))
    }
    pub fn effective_commit_identity(&self, root: &Path) -> Result<GitCommitIdentity> {
        let ident = self.text(root, &["var", "GIT_COMMITTER_IDENT"])?;
        let (name, tail) = ident.rsplit_once(" <").ok_or_else(|| {
            AppError::new("commit_identity", "Git commit identity is unavailable.")
        })?;
        let (email, _) = tail.split_once('>').ok_or_else(|| {
            AppError::new("commit_identity", "Git commit identity is unavailable.")
        })?;
        Ok(GitCommitIdentity {
            name: name.into(),
            email: email.into(),
        })
    }
}

impl crate::GitCli {
    pub fn repository_commit_identity(&self, root: &Path) -> Result<Option<GitCommitIdentity>> {
        if self
            .optional_text(
                root,
                &[
                    "config",
                    "--local",
                    "--bool",
                    "--get",
                    "gitnova.useRepositoryIdentity",
                ],
            )?
            .as_deref()
            != Some("true")
        {
            return Ok(None);
        }
        let name = self.text(root, &["config", "--local", "--get", "user.name"])?;
        let email = self.text(root, &["config", "--local", "--get", "user.email"])?;
        let identity = GitCommitIdentity {
            name: name.trim_end_matches('\n').into(),
            email: email.trim_end_matches('\n').into(),
        };
        validate_identity(&identity)?;
        Ok(Some(identity))
    }
    pub fn repository_identity_token(&self, root: &Path) -> Result<String> {
        use crate::RepositoryReader;
        use sha2::{Digest, Sha256};
        let config = self.discover(root)?.common_dir.join("config");
        Ok(format!(
            "{:x}",
            Sha256::digest(crate::remote_config::config_bytes(&config)?)
        ))
    }
    pub fn set_repository_commit_identity(
        &self,
        root: &Path,
        identity: &GitCommitIdentity,
        token: &str,
    ) -> Result<()> {
        use crate::{
            working::{run_write, OperationLock},
            RepositoryReader,
        };
        use std::io::Write;
        validate_identity(identity)?;
        let dir = self.discover(root)?.common_dir;
        let mut guard = OperationLock::acquire(dir.join("config.lock"))?;
        if self.repository_identity_token(root)? != token {
            return Err(AppError::new(
                "stale_preview",
                "Repository configuration changed. Reopen identity settings and review again.",
            ));
        }
        let config = dir.join("config");
        let original = crate::remote_config::config_bytes(&config)?;
        let mut temp = tempfile::NamedTempFile::new_in(&dir)?;
        temp.write_all(&original)?;
        temp.flush()?;
        let file = crate::process::git_path(temp.path())?;
        for (key, value) in [
            ("user.name", identity.name.as_str()),
            ("user.email", identity.email.as_str()),
            ("gitnova.useRepositoryIdentity", "true"),
        ] {
            run_write(
                root,
                &[
                    "config".into(),
                    "--file".into(),
                    file.clone(),
                    "--replace-all".into(),
                    "--".into(),
                    key.into(),
                    value.into(),
                ],
                None,
            )?;
        }
        let updated = crate::remote_config::config_bytes(temp.path())?;
        if self.repository_identity_token(root)? != token {
            return Err(AppError::new(
                "stale_preview",
                "Repository configuration changed. Original configuration was preserved.",
            ));
        }
        guard.publish(&config, &updated, None)?;
        Ok(())
    }
}
