# GitNova multi-repository addendum

Mandatory product requirements supplied with the master engineering prompt. This addendum supersedes its single-repository assumptions and roadmap numbering.

“Developers work on projects, not isolated repositories.”

## Workspace model

Support independent local repositories, submodules and nested submodules, linked worktrees, nested independent repositories, and bare repositories where applicable. Distinguish repository identity, worktree identity, workspace membership and submodule relationships. One repository may belong to several workspaces. Workspace groups are not Git submodules.

## Workspace experience

Provide a workspace switcher, add-existing and clone-into-workspace actions, recent and pinned repositories, a workspace repository list, fast switching, per-repository branch/dirty status and preserved graph/selection context. Support search scopes for current repository, selected repositories and the whole workspace. The graph remains central; do not introduce a permanent dashboard for multiple repositories.

Before adding any screen, sidebar, panel, dialog, button or navigation item, prove that the same capability cannot be delivered naturally from the existing Git graph context. Optimize for fewer interactions and lower cognitive load.

## Submodules

Discover recursively. Detect missing/uninitialized, dirty and detached states. Support initialize, restore recorded commit, update from configured remote branch, add/remove, URL sync, pointer differences, graph navigation and nested submodules. Clearly distinguish “Restore recorded version” from “Update to a newer version.” Never silently move a commit pointer. Detect staged and unstaged parent gitlink changes when a checkout changes.

Before destructive operations, preview affected repositories, identify dirty worktrees, protect uncommitted work and unpushed commits, and require explicit confirmation. Preview must not execute custom update commands or untrusted hooks.

## Worktrees

List linked worktrees and checked-out branches. Open/switch existing worktrees; safely create/remove; detect locked/prunable entries and repair metadata where appropriate. Never automatically remove a dirty worktree.

## Multi-repo operations

View statuses, fetch explicitly selected repositories concurrently, search and investigate changes across a workspace, and preview selected-repository operations. Later support coordinated branch comparisons, multi-repository release readiness and dependency-aware release investigation.

No blind batch push/pull/reset/rebase/commit. Every batch write needs per-repository preview, explicit selection, validation, partial-failure reporting and recovery guidance. Never describe batch operations as atomic.

## Search and architecture

Search results identify their repository and navigate directly to commits, files, diffs or historical revisions. Add gitnova-workspace, gitnova-submodule and gitnova-worktree boundaries independent of Tauri. Use stable domain IDs and explicit parent-child relationships. Repository folder names are not identities. Handle paths, symlinks, linked worktrees and submodule gitdir indirection correctly. Avoid duplicate indexing when worktrees share an object database.

## Multi-repo MVP acceptance

A developer can open several repositories in one workspace, switch without a new app, see branch/dirty state, open a cached graph promptly, recognize submodules, inspect recorded/checked-out commits, initialize/restore safely, navigate into submodules, open linked worktrees and search commits across the workspace. Use real Git repositories; do not substitute mock data after integration.

## Revised priority

1. Multi-repo workspace foundation.
2. Real Git graph and everyday operations.
3. Submodule and worktree support.
4. Workspace search and code archaeology.
5. Smart change preview.
6. Cross-repository release intelligence.

Revise architecture and milestones before implementation. Continue through small reviewable changes and tests without sacrificing graph simplicity. The full MVP acceptance above spans several implementation milestones; completing the first graph slice does not complete that acceptance.
