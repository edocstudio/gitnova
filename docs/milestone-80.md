# M80 — Advanced reviewed replay and linked-worktree movement

## Graph Rebase review

The optional **Customize linear replay…** action edits an existing bounded linear
range, oldest first. Pick keeps the commit; Drop omits its patch; Fixup combines
its patch with the preceding kept commit, retaining that commit's message and
author. Move controls reorder the exact original OIDs. Every original commit must
occur exactly once; at least one Pick must remain. Free-form todo/exec commands
are never accepted. Changing the plan resets rewrite acknowledgement and requires
an updated prediction. Invalid plans remain editable in the same review.

Execution uses a generated private sequence editor with a validated todo, not
user-provided executable text. Unix supports this flow; Windows receives an
explicit refusal. Clean predictions bind the final tree; completion checks commit
count/authorship and retained other refs. Conflicts reuse the owned journal,
Working changes and Continue/Abort. Empty-result Skip retains its existing bounds.
A same-base custom rewrite is allowed even when a normal rebase is unnecessary.

Topology-preserving replay now retains cousin branch points using native
`--rebase-merges=no-rebase-cousins`. Clean octopus replay supports up to eight
ordered parents; prediction folds merges in an isolated object store, and final
verification checks the one-to-one graph, parent order, authors and messages.
An octopus range with a predicted conflict is refused before writing. These
operations are separate from editing a linear sequence. Shallow history,
submodule-changing replay, custom merge drivers and renormalization retain their
existing refusals. This is not unrestricted interactive scripting/rewording or
arbitrary conflictful octopus support. Native semantics are documented in
[Git's rebase manual](https://git-scm.com/docs/git-rebase).

## Move an existing linked checkout

The existing Worktree metadata form accepts a new folder and offers **Move linked
worktree…**. Review source/destination, metadata recovery and retained state, then
confirm **Move reviewed worktree**. It operates from another checkout of the same
repository; the current/main checkout cannot be moved by this action.

The selected checkout must be clean, unlocked, registered and not prunable.
Destination must be absent, outside repositories/administrative directories,
with an existing parent. Unix rejects cross-filesystem movement. Tracked
submodules and ignored nested repositories require a separate workflow, rather
than leaving their metadata silently broken. Nested scanning and metadata copies
have explicit bounds.

Confirmation saves original metadata privately, retains the source tip, rechecks
state and invokes native `git worktree move` without force. Verification checks
repository/worktree identity, HEAD/index and normal refs. Catalog memberships keep
their identity and switch to the repaired path. Metadata/filesystem movement and
catalog persistence are separate effects; failures require inspection rather than
automatic rollback or retry. Dirty worktrees are never moved implicitly.

## Evidence

See [verification](verification/2026-10-30-advanced-lifecycle.md). All tested writes
use disposable repositories. No user project history, working files or remotes
are used as fixtures, and no commit/push/release is performed by this increment.
