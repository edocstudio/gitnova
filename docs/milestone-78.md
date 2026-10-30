# M78 — Lifecycle completion and cross-repository history evidence

This increment extends the existing graph, Working changes, related-worktree
metadata review and Search surfaces. It does not add a dashboard or make batch
writes atomic. Native writes are tested only in temporary repositories.

## Rebase: root and two-parent topology

The graph Rebase review now recognises a complete linear history with no common
ancestor and reviews replay from its root onto the selected target. A shallow
boundary is never treated as a root. The existing rewrite acknowledgement,
100-commit bound, retained tips, native session ownership, conflict resolution,
Continue, explicit empty Skip and Abort remain in force.

For ordinary two-parent merge ranges, isolated prediction recreates the parent
graph rather than flattening it. Native execution uses
`--rebase-merges=no-rebase-cousins`. Completion verifies a one-to-one mapping of
the reviewed graph, ordered parent counts, original messages/authorship and the
predicted final tree when prediction was conflict-free. Other branch refs remain
unchanged. Git's internal rewritten labels are bound with the native journal;
changed labels invalidate continuation. Skipping a recreated merge is refused
because that would change topology without a reviewed rewrite.

Original manual merge resolutions may need reapplication. Octopus merges,
cousin/external-parent ranges, shallow history, custom merge-driver or
renormalization configurations and submodule-changing ranges retain explicit
refusals. These are supported bounded operations, not arbitrary interactive
history editing. See native [Rebase semantics](https://git-scm.com/docs/git-rebase).

## Coupled submodule registration staging

Working changes routes added/deleted gitlinks to the existing registration
review. Selecting `.gitmodules` also offers Stage/Unstage registration with an
explicit relative child path. The action reviews the entire manifest together
with that pointer; it never silently applies a half-registration.

Preview validates HEAD, index, working manifest, configuration and child
identity/revision. Other semantic registration changes, conflicts, hidden index
flags and unsafe paths require separate inspection. Execution prepares a private
index, preserves unrelated staged entries, saves the original index privately
and publishes the prepared index through Git's index lock after revalidation.
Child files, branch, HEAD and working manifest are not changed. Configured clean
filters may run on confirmed manifest staging; review the resulting staged diff
before committing. This is independent of checkout/update operations.

## Selected missing-worktree prune

The missing worktree's existing metadata form offers a prune review only for an
unlocked, prunable linked checkout. Existing folders are never selected for
removal. Other missing, redirected,
malformed, symbolic-link or active/locked metadata blocks the selected action.
The complete reviewed metadata (bounded to 64 MiB / 1,024 files) is archived in a
private recovery folder and the tip retained. Confirmation atomically moves only
the selected registration directory into that archive; it does not run a global
prune that could remove unrelated registrations if another disk disappears.
Only matching repository/worktree memberships are removed from the catalog;
branches, existing checkout files and unrelated repositories remain unchanged.

Native metadata removal and catalog persistence are separate effects. A partial
failure requires inspection, not blind retry. Metadata backups cannot restore
working files from a disconnected disk or undiscovered moved folder: use Reattach
for moved checkouts. No object garbage collection is performed.

## Follow file renames

Search → Follow file renames takes one exact regular file at an exact local
branch/ref or HEAD. Native `git log --follow` supplies rename history; each hit
carries its historical filename for graph/diff navigation without checkout.
Directory pathspecs, non-UTF-8 historic names and ambiguous/invalid output fail
explicitly. The output is bounded to 100 results / 2 MiB and the native process
deadline; shallow or limited coverage remains partial. This does not claim full
line/blame archaeology or every merge path through Git's history simplification.

## Dependency-aware branch investigation

The existing cross-repository branch comparison also compares versioned gitlinks
at both immutable tips. For changed pins, evidence checks the versioned/current
manifest relationship and explicitly registered child checkout identity before
reading child commit objects or ancestry. It reports forward updates, rollback,
divergence, addition/removal and cached remote containment. It never matches
repositories by folder name or coincidentally identical object IDs alone.

Unregistered children, changed registration URLs, unavailable objects, shallow
child history and time/output bounds remain unknown or partial. At most 32
versioned gitlinks per parent are admitted. No network access, dependency
checkout, commit, publication or CI action is performed. Cached remote history
is not live publication proof. Package/runtime compatibility, hosted CI approval,
semantic release readiness and general package-manifest dependency inference are
not certified by gitlink evidence.

See [verification](verification/2026-10-30-lifecycle-dependency-evidence.md).
