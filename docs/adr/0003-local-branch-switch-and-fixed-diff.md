# ADR 0003: Previewed local branch switching and fixed comparison panes

The current-branch display becomes an actual local-branch selector. Reuse its popover for filtering, affected paths, blocked reasons and explicit confirmation. There is no new application page, force/discard action, automatic stash, remote guessing or implicit branch creation.

BranchReader/BranchWriter remain independent of Tauri. The preview binds source HEAD, symbolic HEAD, index bytes, target name/OID and linked-worktree occupancy. Revalidate under the common-directory operation guard, use native Git's index/ref locking, and execute `git switch --no-guess --no-recurse-submodules --no-overwrite-ignore -- <local-branch>`. Never opt into overwriting ignored files or another worktree's checkout. Native checkout hooks and filters run only during explicit execution. Preview reads use the existing neutralized read adapter.

Initially require a clean working tree. Refuse index/HEAD locks, merge/rebase/cherry-pick/revert states, occupied branches, submodule pointer changes and assume-unchanged/skip-worktree entries. Detached commits must be directly preserved on a local branch before switching away. Bare/unborn cases cannot use this write path. Do not claim atomicity against arbitrary external filesystem/ref edits. Confirm the resulting branch and HEAD, persist receipts, and report unknown/partial outcomes without retry or rollback.

For split diffs, replace the shared intrinsic-width grid with two `minmax(0, 1fr)` panes. Before/After labels and the center divider stay outside horizontal scrollers. Each code pane has its own horizontal offset; vertical offsets are synchronized over the same aligned row sequence. Preserve full-file content widths while windowing rows, so scrolling beyond the longest line cannot shrink the scroll range. Sticky line gutters keep line references visible. Headers identify the actual first-parent/empty-tree and selected commit OIDs.

Verify real branch writes against temporary repositories, including dirty/stale/occupied/locked/ignored/hidden-file cases and hook side effects. Verify geometry and scrolling in the browser using a real Git patch with long lines and 100 changed lines.

References: installed Git 2.39.2 help and the official [switch](https://git-scm.com/docs/git-switch) and [worktree](https://git-scm.com/docs/git-worktree) documentation.
