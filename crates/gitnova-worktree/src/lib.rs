use gitnova_domain::*;
use gitnova_git::GitCli;
use std::path::Path;
pub fn inspect(git: &GitCli, path: &Path) -> Result<Vec<LinkedWorktree>> {
    git.linked_worktrees(path)
}

pub fn preview_create(
    git: &GitCli,
    path: &Path,
    request: &WorktreeCreateRequest,
) -> Result<WorktreeCreatePreview> {
    git.preview_create_worktree(path, request)
}
pub fn create(
    git: &GitCli,
    path: &Path,
    request: &WorktreeCreateRequest,
    token: &str,
) -> Result<()> {
    git.create_worktree(path, request, token)
}

pub fn preview_remove(
    git: &GitCli,
    path: &Path,
    request: &WorktreeRemoveRequest,
) -> Result<WorktreeRemovePreview> {
    git.preview_remove_worktree(path, request)
}
pub fn remove(
    git: &GitCli,
    path: &Path,
    request: &WorktreeRemoveRequest,
    token: &str,
) -> Result<()> {
    git.remove_worktree(path, request, token)
}
pub fn membership_token(
    native: &str,
    worktree_id: &Option<String>,
    memberships: &[WorktreeMembershipImpact],
) -> Result<String> {
    gitnova_git::worktree_remove::membership_token(native, worktree_id, memberships)
}
