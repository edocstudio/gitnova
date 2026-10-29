# Stale owned-operation session — 2026-10-29

## Diagnosis

Read-only inspection found a valid older GitNova cherry-pick journal, while native merge/cherry-pick/revert/rebase/sequencer indicators were absent and the current checkout had advanced. Git status showed no tracked changes. The app correctly refused unrelated writes but its release preview required the old original HEAD/branch, stranding this state in the UI. No user project journal, source file, index, submodule, ref or configuration was changed during diagnosis or testing.

## Change

The existing reviewed session-cleanup operation archives only the validated ownership marker and removes that marker after revalidation. It accepts an externally advanced/switched checkout while preserving its current state. A shared implementation handles merge/cherry-pick/revert/rebase; their execution/abort logic is not used for cleanup. Native operation markers, locks, unresolved index conflicts, malformed ownership, existing archives and stale reviews still refuse closure. The review token now includes live HEAD/branch, including same-tree external commits. Closing a record no longer reads a historical diff or claims any working-file scope.

The existing operation-status text opens Working changes directly. It does not add a screen or automatically clean up user state. The user must review and confirm Close session & keep work.

## Checks actually run

- Native `merges` integration suite: **24 passed**. New regressions cover externally completed merge/cherry-pick/revert, a different current branch, preserved exact index bytes/staged/unstaged/untracked content and every ref, archived marker equality, identical-tree HEAD changes invalidating review, and newly active native operations refusing closure.
- Native `rebase` integration suite: **12 passed**. New coverage confirms explicit metadata-only closure after external abort and a subsequent commit, preserving the current checkout/index/files/recovery refs. Existing conflict/restart/Continue/Skip/Abort and topology guards pass.
- Frontend TypeScript check and **45 unit tests** passed across 11 files.
- Two real-core browser workflows passed: interrupted-session review/confirmation after checkout advancement while remote is two commits ahead; graph-owned rebase conflict resolution and Continue. Cleanup restores Pull availability without losing staged/unstaged content. Browser/native IPC is emulated; Git operations use actual Rust and disposable repositories.
- Workspace all-target Clippy with warnings denied, formatting and `git diff --check` passed.
- Canonical local debug GitNova.app rebuilt (70.99 MiB), with signature and embedded-UI verification passed. No alternate app was created or launched.
- Inspected the synthetic cleanup-review screenshot: advanced local HEAD, remote-ahead graph, explicit metadata-only scope, private archive path and confirmation remain visible in the existing inspector/dock.

Verification ran one runner at a time with one Cargo job, one native test thread and one browser worker. All Git-writing fixtures were temporary. This does not certify arbitrary external races or corrupted sessions; uncertain results retain recovery evidence and are not retried automatically.

Logs: `/tmp/gitnova-m61-native.log`, `/tmp/gitnova-m61-ui.log`, `/tmp/gitnova-m61-clippy.log` (local verification artifacts).
