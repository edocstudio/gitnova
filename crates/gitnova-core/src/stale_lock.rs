use super::*;
impl Application {
    pub fn preview_stale_lock(&self, membership_id: &str) -> Result<StaleLockPreview> {
        self.git.preview_stale_lock(&self.path(membership_id)?)
    }
    pub fn recover_stale_lock(&self, membership_id: &str, token: &str) -> Result<NativePath> {
        // Deliberately bypass ordinary operation acquisition: the existing lock
        // is what is being inspected. The Git layer serializes recovery itself.
        self.git
            .recover_stale_lock(&self.path(membership_id)?, token)
    }
}
