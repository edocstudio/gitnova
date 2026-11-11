use crate::Application;
use gitnova_domain::{CommitHistoryPreview, OperationReceipt, Result};

impl Application {
    pub fn preview_commit_history(&self, id: &str, redo: bool) -> Result<CommitHistoryPreview> {
        self.with_repo_identity(id, || {
            self.git.preview_commit_history(&self.path(id)?, redo)
        })
    }

    pub fn apply_commit_history(
        &self,
        id: &str,
        redo: bool,
        token: &str,
    ) -> Result<OperationReceipt> {
        self.execute_write(
            id,
            if redo { "redo_commit" } else { "undo_commit" },
            |git, path| git.apply_commit_history(path, redo, token),
        )
    }
}
