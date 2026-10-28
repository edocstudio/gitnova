# ADR 0015 — Saved work in the graph and bounded large merge review

Status: accepted for defect correction.

Saved stashes belong in the existing graph context, with a distinct saved-work marker, their exact stash identity and a link to the recorded base commit. Keep Git's internal index/untracked stash parents out of ordinary commit traversal. Selection opens the existing stash inspector/diff and its reviewed actions; it must never offer commit checkout/merge actions on a stash node. No permanent stash sidebar or screen is required. Stash discovery failure and bounded coverage must be explicit. Graph windowing, keyboard navigation and cached selection must remain bounded.

A merge review has three different budgets: streaming content identity, bounded visible patch text and recoverable replacement/abort data. A large tracked binary must not require loading its contents as editable text merely to inspect or finish an already staged result. Stream fingerprints with bounded byte/time limits; retain conservative complete-data limits for destructive replacement/abort recovery. Finishing binds the exact full index tree and complete path manifest, while a partial patch is labelled and files can be inspected individually in the existing dock. Do not turn truncated code into a complete-review claim or silently raise destructive recovery limits.

An existing owned operation is a navigation destination. Reuse the blocked start review's existing Back action as Review current operation; open Working changes. Show pending/interrupted operation state even with zero changed files. Do not clear markers or finish/abort a user's repository automatically. External or inconsistent sessions retain their Git recovery guidance.

## Interrupted session closure

The reported repository was inspected read-only: original HEAD/ORIG_HEAD still match the owned start, while MERGE_HEAD is absent. Native abort/completion can leave the application record behind. Reuse the existing abort-review control as Review session cleanup only for incomplete owned sessions. A separate reviewed `release` action requires original branch/HEAD, no native merge/pick/revert/rebase/sequencer/autostash or native lock. It archives the exact marker with create-new semantics in its validated private session directory, revalidates live metadata and removes only that marker. Files, index, HEAD and both recovery tips remain untouched. It does not run Git abort/reset or clear an active/inconsistent foreign operation. No automatic cleanup of the developer's repository is authorized by this defect report.
