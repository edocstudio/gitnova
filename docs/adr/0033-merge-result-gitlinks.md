# ADR 0033 — Decide submodule blockers from the predicted merge result

Status: accepted, 2026-10-28.

The user wants feature changes integrated into release while retaining release's
already-correct submodule version. Comparing the two tips is not a merge result:
a submodule pointer changed only on the destination can differ from the incoming
branch yet remain unchanged in the actual merge. The former broad raw-tip guard
also overwrote an earlier already-included-history result.

Classify ancestor/up-to-date/FF-only/unrelated history first. If blocked, preserve
that reason without running prediction or replacing it with a gitlink comparison.
For an eligible merge, predict in the existing isolated object store, then inspect
current HEAD versus the result tree. Allow ordinary merge when that result keeps
all gitlink entries unchanged and has no submodule conflict. Existing source-tip,
index/config/working-state token checks, nonrecursive native execution, staged-tree
verification and reviewed Finish/Abort behavior remain mandatory.

A conflict can retain our gitlink in the synthetic result tree. Therefore also
check conflict paths against differing submodule entries; do not interpret an
unchanged tree entry as a resolved submodule conflict. Actual gitlink changes,
add/remove/type changes and unresolved submodule conflicts remain blocked until
an explicit child-aware review/recovery workflow is implemented. Cherry-pick and
revert retain conservative submodule-delta blockers.

Expose exact current/incoming/predicted pointer identities in the existing merge
preview. Label preserved pointers and uncomputed conflict results distinctly.
This is evidence for the existing decision, not a new panel or navigation item.
No child checkout, branch, index, URL sync or custom update command is performed.
A clean/detached child does not establish whether it is the latest remote version;
no fetch or switch is implied by inspection. Do not advise restoring an old
incoming pointer just to bypass this app limitation.
