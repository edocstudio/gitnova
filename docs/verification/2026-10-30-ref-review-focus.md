# Ref review keyboard focus — 2026-10-30

## Failure

The supplied desktop Actions log reports 99 passing tests and one failure in
`graph management renames, configures upstream and retains deleted branch and
annotated tag objects`. After focusing Back and pressing Shift+Tab, the test
expected the annotated-tag disclosure summary to retain focus. This is distinct
from the earlier native tracking-ref reconciliation failure.

RefManager scheduled autofocus through requestAnimationFrame on mode/loading
changes. A callback could run after the user had already moved focus to the
summary, returning focus to Back. GraphActions also queued parent autofocus for
the nested form. Simply increasing the assertion timeout cannot restore focus.

## Reproduction and change

A new real-core browser regression pauses the browser animation clock while the
review loads, moves focus using Shift+Tab, then releases pending frames. Before
the fix it failed precisely at the post-frame summary-focus assertion. The
original integration case passed locally once, confirming that it alone was not
a reliable reproducer of the timing race.

- RefManager now focuses the ready form in a layout effect before paint, rather
  than scheduling a callback that can steal a later keyboard interaction.
- GraphActions uses synchronous layout focus for its own modes and leaves
  manage/push/delete-local form focus to RefManager.
- The regression retains focus assertions, Enter disclosure activation, forward
  Tab order and Escape dismissal. No retries, sleeps, timeout increases or test
  skips were added. The clock advances model a pending animation frame.
- No Git writer, credentials, dependencies or CI configuration changed.

## Verification

All Git writes are confined to disposable test repositories; one runner at a
time on local macOS with Apple Git 2.39.2.

- Before fix: original integration case passed once; deterministic clock
  regression failed at the post-frame focus assertion, reproducing the defect.
- After fix: original case and clock regression repeated three times each:
  **6 passed** (1.2 minutes).
- Adjacent graph context/checkout, merge conflict editor, worktree creation and
  the five previously failing Push/delete/publication cases: **8 passed**
  (1.6 minutes).
- Frontend production build (including TypeScript): passed. The existing
  JavaScript chunk-size advisory remains; it is not suppressed.
- Frontend unit tests: **53 passed**, 13 files.
- `git diff --check`: passed.

The complete browser suite and hosted Actions have not been rerun by this task.
No Rust source changed, so Rust suites were not rerun. Native bundle replacement
is separate from the frontend build. No commit, push, bundle replacement or
release was performed.
