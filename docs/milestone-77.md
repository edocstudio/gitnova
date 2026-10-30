# M77 — Registration lifecycle, moved checkouts and text hunk boundaries

This increment closes specific ordinary-workflow gaps found during the lifecycle
audit. It does not imply support for every Git topology or erase safety bounds.

| Workflow | Supported path | Preservation / boundary |
| --- | --- | --- |
| Add submodule | Branch Submodules → Add → source and relative folder → preview → confirm | New folder downloads the source default branch; its exact commit is known after completion. An existing independent checkout keeps HEAD/branch/files. No custom update strategy or nested initialization. |
| Remove submodule | Existing Manage → Remove registration → preview → confirm | Stages gitlink/.gitmodules removal, retains child directory, object store, local commits and configuration. No recursive file deletion or deinit. |
| Commit registration | Working changes → staged manifest → Commit or Amend | Staged gitlink additions/modifications/deletions may be included; committing does not check out a child. Generic file staging/discard still yields to pointer/registration reviews. |
| Nested registration | Open the immediate parent repository, then use its Submodules group | Parent index scope stays explicit. No automatic recursive checkout or nested-layout migration. |
| Existing child lifecycle | Initialize / Restore recorded / reviewed newer cached revision / URL sync / pointer review | Existing guards and nonrecursive operations remain distinct. Newer revision requires proven forward ancestry; fetch the child separately for freshness. |
| Linked checkout lifecycle | Create / Open / Lock / Unlock / Repair / clean Remove | Dirty, ignored, hidden or submodule-containing worktrees retain their removal guards. Main/active checkout is never removed. |
| Moved linked checkout | Missing entry → Worktree metadata → existing moved folder → Reattach review | Original .git locator must identify the selected admin directory in the same repository. No files or branches move. Matching catalog paths update across workspaces while worktree IDs remain stable. Missing/locked states are distinct. |
| Modified text hunks | Stage / Unstage the selected native-generated block | Working bytes and other staged content are preserved by a private-index transaction. |
| Added/deleted text hunks | Same controls, including a repository with no initial index/commit | Native text additions/deletions are single hunks. Non-UTF-8, binary, symlink/gitlink, mode-changing and incomplete patches remain whole-file actions. New-file text/EOL/encoding/ident conversion also requires native whole-file staging. |
| Linear Rebase | Graph target → replay/publication/rewrite review → Start → conflict resolution / Continue / explicit empty Skip / Abort | Existing owned journals, original author/date verification and recovery refs remain. Regressions cover restart, multiple conflict stops, changed state and linked-checkout isolation. |

## Write and recovery contracts

Registration previews bind parent HEAD/index/configuration, the existing manifest,
child identity/state and selected destination. Existing `.gitmodules` edits,
tracked parent destinations, overlapping registrations, unsafe paths and retained
metadata collisions refuse Add/Remove. A one-shot private recovery directory
saves the original parent index, manifest and configuration before native writes.
Unrelated index records and parent HEAD are checked afterward. Native Add/Remove
stages metadata; the user reviews that staged result before committing.

Removal intentionally leaves the child on disk; this includes dirty child work
and cached/absorbed metadata. The retained folder can appear as untracked local
content after registration removal. This is not a claim that it is safe to delete
that content or overwrite retained metadata by immediately re-adding it.

Moved-worktree repair backs up locators, HEAD and index, then uses native repair.
The parent and child index/HEAD/branch/lock postconditions are checked. A foreign,
replaced, symbolic-link or ambiguously registered destination cannot be adopted.
Native metadata repair and catalog persistence are separate effects: a catalog
failure requires inspection, not automatic retry or rollback.

Previews do not clone, fetch, invoke custom update commands or run hooks/filters.
Confirmed native writes can run configured hooks/filters. Uncertain partial
results retain receipts/recovery locations; no transaction across arbitrary
external Git/filesystem activity is promised. Large indexes are hashed rather
than expanded into JSON numeric arrays for snapshot tokens.

## Explicit advanced limits

Topology-preserving/root/unrelated-history Rebase and interactive reordering are
not implemented by the linear Rebase engine; its 100-commit complete replay
budget remains. No silent flattening, implicit autostash or force push is added.
Worktree metadata prune, arbitrary folder moves/discovery and automatic nested
checkout transitions are not part of the new actions. These limits stay explicit
in previews/documentation rather than becoming unchecked writes.

See [verification](verification/2026-10-30-registration-worktree-hunks.md).
