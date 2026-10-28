# Continue a diverged Pull with an explicit merge review

The supplied 2026-10-27 defect shows a valid fast-forward refusal but a dead-end
workflow and an uncomputed zero-path prediction. A disabled Pull alone cannot
complete the user's integration task.

Reuse the current Pull review and existing MergeStartReview, merge inspector and
diff dock. Offer Review merge only when both local and incoming commits exist.
This button is read-only: starting and finishing a merge remain separate explicit
confirmations. No automatic rebase, stash, reset or history rewrite is introduced.

Carry the actual fetched remote commit independently from the fast-forward target
(which remains the local tip when ahead). Bind the merge handoff to that source
ref/OID and the reviewed destination branch/local OID. Reject context changes
before enabling Start; native merge tokens revalidate again on confirmation.

Blocked previews do not display empty file lists as a computed zero-path change.
Apply that presentation rule to the related remote picker and graph checkout
previews too. Preserve native fast-forward-only Pull behavior and recovery.
