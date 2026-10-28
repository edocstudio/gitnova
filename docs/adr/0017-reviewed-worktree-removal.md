# ADR 0017 — Reviewed removal of one linked worktree

Status: accepted for incremental implementation.

## Existing context

Use the existing Related repositories worktree item in the workspace picker for one Remove… action and its inline review. The graph selects commits; deleting a checkout directory needs the exact worktree identity and path supplied by that existing relation. A second worktree screen/dashboard or a graph commit action that guesses a folder would obscure the scope. Keep Open as the normal item action. Preview shows the exact folder/branch/HEAD, recovery pointer and all workspace memberships affected. Source graph, selection and drafts remain in place.

## Native and catalog scope

Only an available linked checkout sharing the registered source common directory is eligible. Never remove the active source or the main/bare checkout. Refuse locked/prunable/missing/replaced/symlinked paths, native or owned operations, hidden index flags, submodule gitlinks, staged/unstaged/untracked/ignored content and unsupported ref storage. Preserve the branch and pin the exact HEAD under an internal recovery ref before confirmed removal, including detached and unpushed history. Nothing fetches or infers publication state.

Read-only preview rebuilds a zero-stat index in the existing isolated scratch repository from the target HEAD, using source objects as read-only alternates. A really-refresh of that disposable index and explicit worktree diff recheck tracked bytes without trusting the target's stat cache or changing its index. Configured helpers do not execute during preview. Custom normalization may conservatively block removal. Inspection/output/time budgets refuse incomplete evidence.

Apply revalidates under the common GitNova operation guard and uses exclusive target HEAD/index and named-branch lock files. These prevent ordinary native Git checkout/commit/ref writes during the final checks; guards remove only their own unique content and never remove a later owner's lock. Pin recovery, recheck directory identities, cleanliness and HEAD/branch, then invoke native `git worktree remove` without force. Never recursively delete the folder from application code. Native failures may leave the recovery ref, folder or metadata, and are reported without automatic retry/rollback. Arbitrary external filesystem writers remain outside an atomic transaction.

Workspace membership impact is included in the reviewed token. After verified native success, remove only the reviewed target's memberships and retired WorktreeId, preserving the RepositoryId and all other checkouts. Validate catalog impact again before publishing cleanup. Native removal, receipt and catalog publication are separate effects; report partial outcomes and retain the exact plan/recovery pointer if catalog persistence fails. Recreating a checkout receives a fresh WorktreeId.

## Limits

This slice requires an available source checkout and supports files ref storage and clean linked worktrees, not automatic pruning, unlocking, repair, submodule-containing worktrees, or deletion of dirty work. Native configured hooks can run at confirmed ref creation. Test writes only in temporary actual repositories, including stat-cache-hidden edits, ignored/nested data, locks, detached/unpushed recovery, stale snapshots, workspace impact and main/active protections. Windows/Linux and native webview automation need separate verification.

Reference: [Git worktree](https://git-scm.com/docs/git-worktree).
