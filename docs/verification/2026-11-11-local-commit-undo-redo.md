# Local commit Undo and Redo

Version: 1.0.8. Baseline: `cba0d79` plus this working-tree change.
All Git write checks used disposable repositories, not project repositories.

Undo acts on the checked-out branch's latest unpublished commit, keeps the
index and working files, and retains the original tip in a recovery ref. Redo
restores the identical commit object only while the original staged tree,
checkout and first-parent position still match. It is one-level commit Redo,
not an undo stack for arbitrary Git operations.

## Verification

- `cargo test -p gitnova-core --test commit_history -- --test-threads=2`:
  8 passed. Includes root and merge commits, exact index/file preservation,
  restart persistence, worktree isolation, stale tokens, published/tagged tips,
  pending operations, native locks, changed staging and disabled ref hooks.
- Related `amend` integration suite: 13 passed.
- Native `real_git_flows_through_tauri_commands_and_remote_origins_are_rejected`:
  passed, including Undo/Redo through Tauri IPC and external-origin rejection.
- Frontend Vitest: 21 files, 82 tests passed.
- TypeScript check passed.
- Real-core Playwright Undo/Redo regression passed: staged changes after Undo,
  exact original commit after Redo, and refusal after further file edits.
- Clippy on `gitnova-git`, `gitnova-core` and `gitnova-ipc`, all targets,
  with warnings denied: passed.
- `build:local`: passed. The existing Desktop/GitNova.app was updated to
  1.0.8 after Quit, with installed version, strict code signature and embedded
  assets verified. This is a local debug/ad-hoc bundle, not a notarized release.

The initial native regression had a duplicated temporary fixture declaration;
it was removed and the test passed on rerun. No timeout was increased.

## Boundaries

Validation ran on macOS. Windows/Linux native execution and the full CI matrix
were not run in this session. Cached remote refs cannot prove a commit was
never published; Undo therefore also requires explicit user acknowledgment.
Shallow, detached, grafted/replaced history and active Git operations are
rejected. Concurrent arbitrary external ref writers remain outside the UI's
serialization; compare-and-swap and post-write verification detect changed or
uncertain outcomes without silently rolling back.

No fetch, push, implicit stash, checkout filters or repository hooks are used.
No commit, tag, pull request or release was created for this change.
