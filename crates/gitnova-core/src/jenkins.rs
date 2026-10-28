use super::*;
impl Application {
    pub fn jenkins_account(&self, controller: &str, username: &str) -> Result<Option<String>> {
        gitnova_hosting::jenkins::account_scope(controller, username)?;
        Ok(
            gitnova_hosting::jenkins::account_credential(controller, username)
                .ok()
                .map(|c| c.login),
        )
    }
    pub fn connect_jenkins_account(
        &self,
        controller: &str,
        username: &str,
        prompt: &CredentialPrompt,
    ) -> Result<String> {
        gitnova_hosting::jenkins::connect_account(controller, username, prompt)
    }
    pub fn disconnect_jenkins_account(&self, controller: &str, username: &str) -> Result<()> {
        gitnova_hosting::jenkins::disconnect_account(controller, username)
    }
    fn jenkins_scope(&self, id: &str, s: &JenkinsSelection) -> Result<String> {
        gitnova_hosting::jenkins::validate(s)?;
        self.git.pipeline_remote(&self.path(id)?, &s.remote, &s.oid)
    }
    pub fn connect_jenkins(
        &self,
        id: &str,
        s: &JenkinsSelection,
        prompt: &CredentialPrompt,
    ) -> Result<String> {
        self.jenkins_scope(id, s)?;
        gitnova_hosting::jenkins::connect(s, prompt)
    }
    pub fn jenkins_builds(
        &self,
        id: &str,
        s: &JenkinsSelection,
        refresh: bool,
    ) -> Result<JenkinsSnapshot> {
        let remote = self.jenkins_scope(id, s)?;
        let credential = gitnova_hosting::jenkins::credential(s)?;
        let key = gitnova_hosting::jenkins::cache_key(s, &remote, &credential)?;
        let mut cache = self
            .jenkins_cache
            .try_lock()
            .map_err(|_| AppError::new("hosting", "Pipeline cache unavailable."))?;
        if let Some((_, when, value)) = cache.iter().find(|(k, _, _)| k == &key) {
            let active =
                value.active_unlinked || value.pipeline.runs.iter().any(|r| r.status == "running");
            if when.elapsed().as_secs() < 2
                || !refresh && when.elapsed().as_secs() < if active { 15 } else { 60 }
            {
                return Ok(value.clone());
            }
        }
        let snapshot =
            gitnova_hosting::jenkins::JenkinsClient::new(s, &credential)?.builds(&remote)?;
        cache.retain(|(k, _, _)| k != &key);
        while cache.len() >= 8 {
            cache.pop_front();
        }
        cache.push_back((key, std::time::Instant::now(), snapshot.clone()));
        Ok(snapshot)
    }
}
