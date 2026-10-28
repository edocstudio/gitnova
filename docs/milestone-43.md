# M43 — Merge result versus submodule tip differences

A clean, detached submodule checkout does not mean every parent branch records
its same revision. The old preview compared destination and source tips and
blocked any different gitlink before computing the merge. It also continued
that comparison after determining the source was already included, replacing the
correct reason with a misleading submodule warning.

Preview now preserves earlier classification blockers and computes an eligible
merge's result in the existing isolated object store. Only actual changed
result gitlinks or unresolved submodule conflicts block applying. Destination-only
submodule updates that the merge retains no longer prevent ordinary feature files
from being integrated. No child checkout, refs, index or files are moved.

The existing review shows each differing submodule's path and current, incoming
and predicted commit. Preserved pointers are labelled Keep current pointer;
unresolved results are labelled Submodule conflict. Actual pointer/layout changes
remain explicitly unsupported rather than suggesting that merely inspecting the
child will unlock the operation. An already-included commit gets that reason and
a disabled Already included action instead of a submodule warning.

This does not implement pointer-changing merge, automatic latest-remote updates,
child conflict resolution or a keep-ours override of a real incoming pointer
change. Cherry-pick/revert retain conservative submodule-delta refusal. Existing
staged-tree validation, nonrecursive native execution, parent recovery references,
explicit Finish/Abort and stale-preview checks remain mandatory. See ADR 0033.

Only read-only ref/tree metadata was inspected in the user's repository. No merge,
checkout, stage, reset, fetch, submodule update or other project write was executed
there. All native write verification uses disposable real repositories.

Final verification and the local bundle are recorded below.

## Verification — 2026-10-28

- Four new real-Git submodule merge cases passed: keep-current-pointer integration
  through reviewed Finish with preserved child bytes/HEAD/refs/index; already-included
  history; actual incoming gitlink refusal with exact revisions and no MERGE_HEAD;
  unresolved gitlink conflict including an unexecuted custom update sentinel.
- All 22 existing merge/recovery cases passed (470.13 s, one test thread), including
  merge-commit/FF behavior, conflict resolution, abort/recovery, ignored files,
  stale reviews, locks, custom-driver refusal and linked-worktree session identity.
- Focused cherry-pick and revert end-to-end regressions passed independently.
- Two real-core browser flows passed: reviewed merge preserving the existing child;
  actual pointer blocker followed by externally included history and correct Already
  included display. Child checkout/index/refs remain preserved.
- 29 frontend checks and TypeScript validation passed. Strict Clippy passed for
  git/core/desktop all targets; rustfmt passed.
- One-job local debug bundle completed in 65 seconds; ad-hoc signature and embedded
  UI verification passed for `target/debug/bundle/macos/GitNova.app` (77.45 MiB).
  No GUI or user workspace was opened by artifact verification. The running older
  `GitNova Updated.app` was preserved.

No production repository write was performed. Pointer-changing merge and child
conflict resolution remain outside this fix; this is not a general submodule-merge
support claim. No whole-app or long-session RAM certification is implied.
