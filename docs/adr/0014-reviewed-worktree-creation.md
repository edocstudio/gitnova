# ADR 0014 — Create a linked worktree from graph context

Status: accepted for incremental implementation.

The next multi-repository slice creates a linked worktree at an exact selected commit, with a new branch, an available existing local branch at that commit, or explicit detached HEAD. This does not complete worktree removal/repair or the broader MVP.

## Surface budget

The commit already supplies the revision and local refs. Add one contextual “Create worktree here…” item to its existing graph menu; reuse that menu's form/review/result and the existing workspace picker for opening the result. A separate worktree screen, permanent sidebar, wizard and automatic checkout of the source are unnecessary. Creation leaves source selection and unsent drafts in place. Opening the created worktree is a separate explicit use of the existing related-repository item.

## Native contract

The independent worktree crate coordinates typed requests with the Git adapter's bounded offline reader/writer. Previews are read-only, execute no hooks/filters/network and bind full commit OID, branch availability/occupancy, common/per-worktree identity, relevant configuration and canonical destination-parent identity. Destination must be an absent child of an existing directory, outside Git repositories and their administrative directories; existing files, directories and symlinks are refused. Native paths returned by the preview carry canonical destination bytes; typed paths are explicit user input.

Apply re-previews under the common-repository operation guard and persists the plan before execution. Parent discovery fails closed on errors other than Git’s explicit not-a-repository result. An exclusive directory creation claims the absent destination immediately before native checkout; an externally created empty folder is not adopted. Later arbitrary filesystem races remain outside a transaction. Native Git uses explicit commit/ref, --no-guess-remote, no force, no recursive submodule initialization, and offline protocols. A new branch uses -b with --no-track; detached mode is explicit. Confirmed checkout may run configured hooks/filters; preview cannot. Existing branch movement during execution is detected after the operation, not represented as an atomic guarantee. External filesystem/Git writers remain a race boundary.

Success requires rediscovery of the expected common directory, exact checked-out revision and requested branch state. A failure may leave a branch, destination or administrative metadata; receipts report unknown partial state and direct inspection, never delete directories, force-reset, retry or roll back automatically. Source dirty work is allowed because creation operates on a distinct absent destination; it is neither stashed nor discarded. Submodule gitlinks are disclosed and not initialized.

## Limits

Only a complete bounded target tree is supported (10,000 entries / 4 MiB metadata). Parent directory creation, clone, remote tracking inference, force checkout, removal, pruning, locks/repair writes and automatic registration/open are separate. Bare repositories remain readable; this initial UI creates from registered non-bare worktrees because the existing durable write service validates worktree identity. Verify writes only in temporary actual Git repositories, including stale names/destinations, occupancy, native locks, Unicode, source dirty/index preservation and hook-induced partial failures.

Reference: [Git worktree](https://git-scm.com/docs/git-worktree).
