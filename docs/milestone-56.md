# M56 — reviewed Rebase, hunk staging and repository maintenance

## Behavior

- Right-click the target graph commit → Rebase the current branch onto it. Review current branch → target, the complete replay range and cached publication evidence; explicitly acknowledge rewritten commit IDs. Original and target tips remain reachable through recovery refs.
- Native linear Rebase supports at most 100 commits. Its owned journal binds repository/worktree, source, target, original authorship and native metadata. Valid paused sessions reopen after restart. Resolve conflicts in the existing inspector/dock, review Continue, explicitly Skip an empty replay, or review Abort. Each Continue can encounter the next conflict. Foreign or inconsistent/interrupted native state requires inspection rather than automatic retry/adoption. Abort saves reviewed conflict edits/staging before native Git restores the source checkout.
- Select a supported tracked text file in Working changes. Stage hunk / Unstage hunk on its diff header updates only that native-generated block in the index. It preserves working bytes and unrelated staged content. A private index is atomically published after stale-state revalidation under the native index lock; the UI never constructs patch bytes.
- Related repositories → existing linked checkout → Worktree metadata offers reviewed Lock, Unlock and Repair. Repair shows registered checkout locations affected by native metadata repair; it never deletes files or moves branches.
- Submodule Manage → Sync URLs reviews the selected initialized child’s old parent/default-child-remote URLs and desired .gitmodules URL. Private configuration backups precede the selected nonrecursive native sync. Dirty child files and commit pointers remain intact; no fetch or checkout occurs.
- Labels, reviews, activity and FAQ are available in English and Thai, using existing graph/inspector/dock/related-row surfaces. No permanent panel or dashboard was added.

## Bounds

Rebase does not flatten merge topology, replay root/unrelated ranges, move submodule pointers or force-push. Initial dirty/untracked work, another native operation and custom merge drivers are refused. A clean scratch prediction does not guarantee later replay conflict resolution. Crash-boundary fault injection beyond tested interrupted-journal refusal remains additional hardening work.

Partial staging requires a complete UTF-8 regular-file modification with unchanged mode. New/deleted files, binary data, symlinks/gitlinks, active clean filters and oversized/truncated patches use whole-file actions. The canonical patch budget is 512 KiB and 1,000 hunks. Unused global filter configuration does not disable ordinary text hunks.

Metadata repair requires an existing registered linked directory; it does not discover moved folders or prune missing worktrees. URL sync currently refuses relative URLs and uninitialized children; it does not perform submodule add/remove or update to a newer remote version. Existing initialize/restore-recorded workflows remain separate.

## Verification

All Git-write tests use disposable repositories. Coverage includes exact partial index changes, quoted/tab paths and missing final newline, stale snapshots/native locks, active-filter refusal, original author/date preservation, multiple Rebase conflict stops, explicit empty-skip, restart, edited-result recovery on Abort, topology refusal and linked-worktree isolation. Maintenance fixtures cover dirty checkout preservation, stale lock/config reviews and repair of a broken worktree gitfile. Browser tests exercise the real native core bridge and existing UI flows.

See the [recorded verification results](verification/2026-10-28-rebase-hunks-maintenance.md) for completed checks. This milestone does not establish complete lifecycle coverage or production readiness for every repository shape.
