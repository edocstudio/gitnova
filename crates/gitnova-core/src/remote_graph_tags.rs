//! Bounded, disposable remote tag evidence for display only. Never authorizes writes.
use crate::Application;
use gitnova_domain::{GitRef, RemoteGraphTags, Result};
use gitnova_git::RepositoryReader;
use moka::sync::Cache;
use std::{sync::Arc, time::Duration};

pub(super) struct RemoteTagsCache(Cache<String, Arc<Vec<GitRef>>>);
impl Default for RemoteTagsCache {
    fn default() -> Self {
        Self(
            Cache::builder()
                .max_capacity(4 * 1024 * 1024)
                .weigher(|key: &String, refs: &Arc<Vec<GitRef>>| {
                    (key.len()
                        + refs.capacity() * std::mem::size_of::<GitRef>()
                        + refs
                            .iter()
                            .map(|r| r.name.capacity() + r.oid.capacity())
                            .sum::<usize>()
                        + 256)
                        .min(u32::MAX as usize) as u32
                })
                .time_to_live(Duration::from_secs(60))
                .build(),
        )
    }
}
impl Application {
    pub fn remote_graph_tags(&self, id: &str) -> Result<RemoteGraphTags> {
        self.with_repo_identity(id, || {
            let path = self.path(id)?;
            let names = self.git.text(&path, &["remote"])?;
            let remotes: Vec<_> = names.lines().collect();
            let branch = self
                .git
                .optional_text(&path, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
            let upstream = match branch {
                Some(branch) => self.git.optional_text(
                    &path,
                    &["config", "--get", &format!("branch.{branch}.remote")],
                )?,
                None => None,
            };
            let selected = upstream
                .as_deref()
                .filter(|name| remotes.contains(name))
                .or_else(|| remotes.iter().copied().find(|name| *name == "origin"))
                .or_else(|| (remotes.len() == 1).then(|| remotes[0]));
            let Some(remote) = selected else {
                return Ok(RemoteGraphTags {
                    remote: None,
                    tags: vec![],
                });
            };
            // Match the fetch destination, not a possibly different push URL.
            let url = self.git.remote_graph_tag_destination(&path, remote)?;
            let discovered = self.git.discover(&path)?;
            let key = serde_json::to_string(&(
                gitnova_domain::NativePath::new(&discovered.common_dir),
                remote,
                &url,
                self.git.fingerprint(&path)?,
            ))
            .map_err(|error| gitnova_domain::AppError::new("tag_inspection", error.to_string()))?;
            let tags = self
                .remote_tags_cache
                .0
                .try_get_with(key, || {
                    self.git.remote_graph_tag_refs(&path, &url).map(Arc::new)
                })
                .map_err(|error| (*error).clone())?;
            self.remote_tags_cache.0.run_pending_tasks();
            // A response for an old URL must never decorate a newly configured remote.
            if self.git.remote_graph_tag_destination(&path, remote)? != url {
                return Err(gitnova_domain::AppError::new(
                    "stale_preview",
                    "Remote changed while reading tags. Refresh to try again.",
                ));
            }
            Ok(RemoteGraphTags {
                remote: Some(remote.into()),
                tags: (*tags).clone(),
            })
        })
    }
}
