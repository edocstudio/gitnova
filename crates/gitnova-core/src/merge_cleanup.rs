use super::*;
impl Application {
    pub(crate) fn complete_merge_source_cleanup(
        &self,
        id: &str,
        mut receipt: OperationReceipt,
        selected: Option<(MergeRequest, String)>,
    ) -> Result<OperationReceipt> {
        let Some((request, destination)) = selected.filter(|_| receipt.status == "succeeded")
        else {
            return Ok(receipt);
        };
        let merged = receipt.head_after.clone().unwrap_or_default();
        // A second, separately journaled write: never conflate deletion with the
        // already verified merge, retry it automatically, or roll the merge back.
        let mut attempted = false;
        let mut recovery_ref = None;
        let cleanup = (|| {
            let p = self.git.preview_merged_source_cleanup(
                &self.path(id)?,
                &request,
                &destination,
                &merged,
            )?;
            if let Some(reason) = p.blocked_reason {
                return Err(AppError::new("merge_cleanup", reason));
            }
            recovery_ref = p.recovery_ref.clone();
            attempted = true;
            self.apply_ref_operation(id, &p.action, &p.token)
        })();
        let mut outcome = MergeBranchCleanup {
            source_ref: request.source_ref.unwrap_or_default(),
            source_oid: request.source_oid,
            status: "kept".into(),
            message: String::new(),
            receipt_id: None,
            recovery_ref,
        };
        match cleanup {
            Ok(result) => {
                outcome.status = if result.status == "succeeded" {
                    "deleted"
                } else {
                    "unknown"
                }
                .into();
                outcome.message = result.summary;
                outcome.receipt_id = Some(result.id);
                if let Some(PlannedOperation::Ref(p)) = result.planned {
                    outcome.recovery_ref = p.recovery_ref;
                }
                if let Some(recovery) = result.recovery {
                    outcome.message.push(' ');
                    outcome.message.push_str(&recovery);
                }
            }
            Err(error) => {
                if attempted {
                    outcome.status = "unknown".into();
                }
                outcome.message = error.message;
            }
        }
        receipt.summary = if outcome.status == "deleted" {
            "Merge completed; local source branch deleted."
        } else {
            "Merge completed; inspect source branch cleanup."
        }
        .into();
        receipt.branch_cleanup = Some(outcome);
        self.save_receipt(&receipt)?;
        Ok(receipt)
    }
}
