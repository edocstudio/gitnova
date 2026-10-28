//! Bounded disposable display metadata; never authorizes a Git write.
use super::*;
use moka::sync::Cache;
use std::{sync::Arc, time::Duration};
const BUDGET: u64 = 4 * 1024 * 1024;
pub(super) struct TrackingCache(Cache<String, Arc<Vec<BranchTracking>>>);
fn weight(key: &str, rows: &Vec<BranchTracking>) -> u32 {
    let strings: usize = rows
        .iter()
        .map(|r| {
            r.branch.capacity()
                + r.oid.capacity()
                + r.state.capacity()
                + [
                    &r.upstream,
                    &r.remote,
                    &r.remote_branch,
                    &r.push_remote,
                    &r.push_branch,
                ]
                .into_iter()
                .filter_map(|s| s.as_ref())
                .map(String::capacity)
                .sum::<usize>()
        })
        .sum();
    (key.len() + rows.capacity() * std::mem::size_of::<BranchTracking>() + strings + 256)
        .min(u32::MAX as usize) as u32
}
impl Default for TrackingCache {
    fn default() -> Self {
        Self(
            Cache::builder()
                .max_capacity(BUDGET)
                .weigher(|key: &String, value: &Arc<Vec<BranchTracking>>| weight(key, value))
                .time_to_live(Duration::from_secs(60))
                .time_to_idle(Duration::from_secs(30))
                .build(),
        )
    }
}
impl Application {
    pub(super) fn display_tracking(
        &self,
        path: &Path,
        fingerprint: &str,
    ) -> Result<Vec<BranchTracking>> {
        let key = self.git.tracking_display_key(path, fingerprint)?;
        let rows = self
            .tracking_cache
            .0
            .try_get_with(key, || self.git.branch_tracking(path).map(Arc::new))
            .map_err(|e| (*e).clone())?;
        self.tracking_cache.0.run_pending_tasks();
        Ok((*rows).clone())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_tracking_rows_are_not_retained() {
        let cache = TrackingCache::default();
        let value = BranchTracking {
            branch: "x".repeat(BUDGET as usize),
            oid: "a".repeat(40),
            state: "noUpstream".into(),
            upstream: None,
            remote: None,
            remote_branch: None,
            push_remote: None,
            push_branch: None,
            ahead: None,
            behind: None,
        };
        cache.0.insert("fixture".into(), Arc::new(vec![value]));
        cache.0.run_pending_tasks();
        assert!(cache.0.get("fixture").is_none());
    }
}
