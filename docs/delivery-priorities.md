# Delivery priorities

User direction, 2026-10-27: work in the following order. This overrides treating performance work or general feature visibility as the next main milestone. Keep each feature independently reviewable and continue serialized verification to limit host load.

1. Complete remaining planned features, including broader Rebase/hunk coverage and submodule/worktree lifecycle operations. Clone, Amend, linear Rebase and tracked-text hunk staging/unstaging already have implemented slices, described below. Keep the graph central and implement actual native behavior before exposing actions. Each feature includes its own correctness and recovery regression checks; those essential checks are not deferred to the later general audit.
2. Measure speed and resident memory over long sessions with large real fixture repositories. Include native process, WebKit and Git subprocesses, repeated repo switches and large diff/search workloads. Separate cold/warm latency, resident memory trends and peak/transient allocations. Do not infer application speed or bounded memory from the language or a short core benchmark.
3. Triage defects from actual use and expand crash/partial-failure recovery validation across workflows. Keep genuine native Git operations separate from app bookkeeping; never auto-abort, discard or retry work as recovery.

Earlier fix: result-based submodule merge preview and already-included-history
classification (M43). Jenkins/GitLab build evidence remains available (M42), with
shared bounded provider reads and independent credentials.
Copy commit link remains available (M41), as does reviewed Amend (M40). Hosted
merge eligibility, stage/log drill-down and live-account CI acceptance remain open.
Clone into the workspace picker/empty-workspace form is implemented (ADR 0027).
Submodule branch context, linked worktree creation/removal, version tags,
provider create/open requests and local/remote branch deletion also have bounded
implemented slices. The full feature plan and production readiness are not
established by those slices. Reviewed native linear Rebase and tracked-text
Stage/Unstage hunk are implemented in M56. Worktree Lock/Unlock/Repair and selected
initialized-submodule URL synchronization are also available. See
[milestone 56](milestone-56.md) for supported scope and remaining limitations.

Recent delivered increments include active-repository configurable auto fetch
([M67](milestone-67.md)), reviewed local repository initialization
([M68](milestone-68.md)), saved resizable history columns
([M69](milestone-69.md)), and targeted platform-check/UI-state corrections
([M70](milestone-70.md)). These are implemented alpha slices, not a statement
that the entire roadmap or hosted/native cross-platform acceptance is complete.


Rebase acceptance: graph-selected source and current branch/range are explicit, published/rewritten history is identified, original tips remain recoverable, preview does not mutate the working checkout, and begin/resolve/continue/abort survive interruption. No implicit force push, stash or reset. Design the durable operation state before execution.

Amend acceptance: stage and message scope are explicit, preserve omitted unstaged content, show the original commit and replacement parents, retain recovery and distinguish published history. A fresh confirmation binds HEAD, index, message and author choices.

Hunk acceptance: use exact per-file patch identity and reviewed index/working snapshots, preserve unrelated staged/unstaged hunks, handle repeated context and new/deleted/renamed/binary cases honestly, and refuse stale or unsupported selections. Do not synthesize a successful partial stage from a truncated display patch.

Remaining related-repository work includes reviewed worktree prune/relocation and submodule add/remove, relative-URL synchronization and remote-version update, including nested relationships, dirty worktrees and unpushed commit protection. Keep recorded-version restoration distinct from a newer-version update.

Performance follow-up (M57): [measured read-path optimization](milestone-57.md)
adds a reproducible packaged native/background-WebKit soak. This does not close
foreground full-workday, enterprise network latency or incremental-index work.

## Version tags and provider requests — design request, 2026-10-27

The next proposed feature sequence is local version-tag UX → explicit single-tag
Push → provider connection/read identity → create/open GitHub PR and GitLab MR
→ hosted request checks/merge. See [the concrete design](design/version-tags-and-change-requests.md).
This is a design addition, not an implementation/completion claim; the existing
broader rebase/hunk, related-repository and performance/recovery work remains open.

Initial tag creation/publication and provider create/open slices now exist in
milestones 37/38. Hosted merge/checks/diffs and live-provider acceptance remain
open alongside the previously recorded delivery priorities.
