use super::*;
impl Application {
    pub fn preview_init(&self, request: &InitRequest) -> Result<InitPreview> {
        if !self
            .catalog()?
            .workspaces
            .iter()
            .any(|w| w.id == request.workspace_id)
        {
            return Err(AppError::new(
                "workspace",
                "The selected workspace no longer exists.",
            ));
        }
        self.git.preview_init(request)
    }
    pub fn init_repository(&self, request: &InitRequest, token: &str) -> Result<Catalog> {
        let preview = self.preview_init(request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Repository destination changed. Review initialization again.",
            ));
        }
        self.git.init_repository(request, token)?;
        self.register_repository_in(&preview.destination.to_path()?, &request.workspace_id).map_err(|e| AppError::new("init_registration", format!("Repository initialized, but workspace registration failed: {}. Use Add an existing repository; files and Git metadata are retained.", e.message)))
    }
}
