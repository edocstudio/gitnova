//! Post-merge cleanup reuses the exact-ref transaction and recovery journal.
use super::*;
pub(super) fn action(request: &MergeRequest) -> Result<RefOperation> {
    crate::validate_oid(&request.source_oid)?;
    let name = request
        .source_ref
        .as_deref()
        .and_then(|r| r.strip_prefix("refs/heads/"));
    if request.operation != "merge" || name.is_none_or(str::is_empty) {
        return Err(AppError::new(
            "merge_cleanup",
            "Post-merge deletion requires an explicitly selected local source branch.",
        ));
    }
    Ok(RefOperation {
        kind: RefOperationKind::DeleteBranch,
        name: name.unwrap().into(),
        oid: request.source_oid.clone(),
        new_name: String::new(),
        remote: String::new(),
        remote_branch: String::new(),
    })
}
impl GitCli {
    /// Read-only review bound to the verified merge result, worktree state,
    /// exact source tip and existing ref/config/recovery deletion protections.
    pub fn preview_merged_source_cleanup(
        &self,
        root: &Path,
        request: &MergeRequest,
        destination: &str,
        merged_oid: &str,
    ) -> Result<RefOperationPreview> {
        let action = action(request)?;
        let status = self.status(root)?;
        crate::validate_oid(merged_oid)?;
        if status.branch.as_deref() != Some(destination)
            || status.head.as_deref() != Some(merged_oid)
            || action.name == destination
            || self
                .text(
                    root,
                    &[
                        "merge-base",
                        "--is-ancestor",
                        &request.source_oid,
                        merged_oid,
                    ],
                )
                .is_err()
        {
            return Err(AppError::new("merge_cleanup", "The verified merge destination changed. The source branch was kept; review deletion separately."));
        }
        self.preview_ref_operation(root, &action)
    }
}
