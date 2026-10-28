use super::*;
use std::{
    fs,
    io::{Read, Write},
};

impl Application {
    pub fn preview_clone(&self, request: &CloneRequest) -> Result<ClonePreview> {
        if !self
            .catalog()?
            .workspaces
            .iter()
            .any(|workspace| workspace.id == request.workspace_id)
        {
            return Err(AppError::new(
                "workspace",
                "The selected workspace no longer exists.",
            ));
        }
        self.git.preview_clone(request)
    }
    fn save_clone_outcome(&self, outcome: &CloneOutcome) -> Result<()> {
        let directory = self.receipt_dir.join("clones");
        fs::create_dir_all(&directory)?;
        let mut journal = tempfile::NamedTempFile::new_in(&directory)?;
        let mut record = outcome.clone();
        record.catalog = None;
        serde_json::to_writer_pretty(&mut journal, &record)
            .map_err(|error| AppError::new("storage", error.to_string()))?;
        journal.flush()?;
        journal.as_file().sync_all()?;
        journal
            .persist(directory.join(format!("{}.json", outcome.id)))
            .map_err(|error| AppError::new("storage", error.to_string()))?;
        Ok(())
    }
    pub fn clone_repository(&self, request: &CloneRequest, token: &str) -> Result<CloneOutcome> {
        let preview = self.preview_clone(request)?;
        if preview.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Clone source, destination or parent changed. Review again.",
            ));
        }
        // The initial empty catalog may not have been saved yet. Persist its workspace
        // identity before starting so failed/interrupted jobs still belong to it after restart.
        self.update(|catalog| {
            if !catalog
                .workspaces
                .iter()
                .any(|workspace| workspace.id == request.workspace_id)
            {
                return Err(AppError::new(
                    "workspace",
                    "The selected workspace no longer exists.",
                ));
            }
            Ok(())
        })?;
        let id = id();
        let mut outcome = CloneOutcome {
            journal: NativePath::new(&self.receipt_dir.join("clones").join(format!("{id}.json"))), id,
            workspace_id: request.workspace_id.clone(), source: preview.request.source.clone(), destination: preview.destination.clone(),
            status: "pending".into(), summary: "Clone started; completion has not been recorded.".into(),
            recovery: Some("Inspect the clone destination before retrying. Partial data is retained; nothing is automatically removed.".into()), catalog: None,
        };
        self.save_clone_outcome(&outcome)?;
        match self.git.clone_repository(&preview.request, token) {
            Err(error) => {
                outcome.status = "failed".into();
                outcome.summary = format!("Clone stopped ({}): {}", error.code, error.message)
                    .chars()
                    .take(4096)
                    .collect();
            }
            Ok(()) => {
                // Record transfer/checkout completion before separately committing catalog registration.
                outcome.status = "cloned".into();
                outcome.summary = "Repository cloned; workspace registration is pending.".into();
                if let Err(error) = self.save_clone_outcome(&outcome) {
                    outcome.recovery = Some(format!("The cloned repository is retained at {}. Its journal could not be updated ({}). Inspect it and use Add existing repository; do not clone over it.", outcome.destination.display, error.code));
                    return Ok(outcome);
                }
                match self
                    .register_repository_in(&outcome.destination.to_path()?, &request.workspace_id)
                {
                    Ok(catalog) => {
                        outcome.catalog = Some(catalog);
                        outcome.status = "succeeded".into();
                        outcome.summary =
                            "Repository cloned and added to the selected workspace.".into();
                        outcome.recovery = None;
                    }
                    Err(error) => {
                        outcome.summary = format!(
                            "Repository cloned, but workspace registration failed ({}).",
                            error.code
                        );
                        outcome.recovery = Some("The cloned repository is retained. Inspect it and use Add existing repository; do not clone over it.".into());
                    }
                }
            }
        }
        if let Err(error) = self.save_clone_outcome(&outcome) {
            outcome.recovery = Some(format!("{} The latest journal update failed ({}). Inspect the destination and workspace before retrying.", outcome.recovery.as_deref().unwrap_or("Clone and workspace registration completed."), error.code));
        }
        Ok(outcome)
    }
    pub fn recent_clones(&self, workspace_id: &str) -> Result<Vec<CloneOutcome>> {
        if !self
            .catalog()?
            .workspaces
            .iter()
            .any(|workspace| workspace.id == workspace_id)
        {
            return Err(AppError::new("workspace", "Unknown workspace"));
        }
        let directory = self.receipt_dir.join("clones");
        if !directory.exists() {
            return Ok(vec![]);
        }
        let mut candidates = vec![];
        for entry in fs::read_dir(&directory)?.take(2048) {
            let entry = entry?;
            let metadata = entry.file_type()?;
            if !metadata.is_file() || entry.path().extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let metadata = entry.metadata()?;
            candidates.push((metadata.modified().ok(), entry.path()));
        }
        candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
        let mut outcomes = vec![];
        for (_, path) in candidates.into_iter().take(128) {
            let mut bytes = vec![];
            fs::File::open(path)?
                .take(128 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 128 * 1024 {
                continue;
            }
            let Ok(mut record) = serde_json::from_slice::<CloneOutcome>(&bytes) else {
                continue;
            };
            record.catalog = None;
            if record.workspace_id == workspace_id {
                outcomes.push(record);
            }
            if outcomes.len() >= 32 {
                break;
            }
        }
        Ok(outcomes)
    }
}
