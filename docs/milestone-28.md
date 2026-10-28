# Milestone 28 — Pop after an empty pending merge

The screenshot shows a real blocker: the working tree has no changed files, but the native merge and owned GitNova session remain active after Discard. Discard intentionally preserves the pending operation. Allowing Pop merely because the file count is zero would mix saved work into an unfinished merge and would break the reviewed-write contract.

The usability defects were vague recovery guidance and an open stash preview retaining an old blocker/destination after the live checkout changed. This change reuses the stash inspector's existing recovery button and the operation inspector's existing abort review; no screen, panel, navigation item or button is added.

Delivered behavior:

- An owned pending operation is named in the stash blocker, with its current branch and an explicit explanation that zero files does not end the operation. Its existing recovery button opens Working changes for that operation.
- A zero-file pending operation explains Discard versus Abort and emphasizes the existing abort-review button. Native preview, recovery and explicit confirmation remain required. Finishing a genuine empty-diff merge remains available as a separate explicit choice.
- The disabled graph Pop action explains how to reach operation review in its tooltip.
- Open stash previews revalidate read-only on working/operation token changes. This applies to Push/Pop/Apply/Drop and preserves selected file, saved index/working comparison and requested options. No write is replayed automatically; a fresh confirmation uses the new reviewed token and branch.
- EN/TH copy follows the same flow. External/unowned operations and other blockers keep their original inspection requirements.

Regression coverage uses temporary real Git repositories for the complete sequence: stash, begin merge, discard to zero files, reject Pop and stale tokens, preview/cancel abort without clearing anything, explicitly abort, freshly review Pop, restore saved work and remove the stash only after success. Branch tips and unrelated files remain checked. Browser coverage follows this sequence and separately changes branches externally while Pop review remains open, verifying the new destination and preserved comparison before explicit Pop.

No real user repository is aborted, popped, reset or otherwise modified as part of this development. The user's pending operation must be resolved through its reviewed workflow.

## Verification

Serial real-Git core integrations: five operation-discard cases and twelve stash cases passed, including the new empty-operation/blocked-token/abort/Pop sequence, conflicts retaining the stash, exact selected-entry removal, shared worktrees, packed refs, helpers/hooks, SHA-256 and stale positions/content. Frontend/catalog units: 23 passed; TypeScript check passed. Three one-worker browser flows passed in 2.7 minutes: main stash onto develop with graph lanes, full merge-discard/abort/Pop with EN/TH, and live external branch changes while Pop review stays open. The latter verifies that review alone leaves stash/files untouched and preserves Saved index selection until explicit confirmation.

Local build: **GitNova Updated.app**, 61.55 MiB. Production frontend type-check/build, ad-hoc signature verification and embedded asset self-check passed. The running GitNova.app session was left intact; the alternate bundle avoids overwriting its assets. Quit that session after its current command finishes before opening the updated bundle, since both bundles share the single-instance application identity.
