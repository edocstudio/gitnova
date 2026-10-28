# Graph action coverage and sequencing

The user's reference establishes direct manipulation: act on the commit or exact ref already under the pointer. GitNova implements an original compact contextual menu, rather than copying the reference's entire flat list. Keep the graph visible; use the same scope → evidence → confirmation → result contract as branch and working-change flows.

Right click, Context Menu and Shift+F10 open commit actions. Double click on a ref uses its exact identity. Double click on a graph node prefers a unique local branch, then a unique remote branch when no local exists. Multiple candidate branches open the choice menu. A node without branch refs previews detached checkout. These gestures open the action for review; they never reset or move an existing branch, discard work or integrate changes without confirmation. A remote selection fetches that remote branch first, then reviews the actual local create/fast-forward effect. Fetch can update cached remote refs before checkout confirmation; local files and branches remain unchanged during that step.

## Available in this increment

| Action | Scope and behavior |
| --- | --- |
| Inspect commit | Selected graph commit; existing inspector and diff dock |
| Rebase current branch | Selected target commit; reviewed linear replay range, rewrite acknowledgment, retained tips and owned Continue/Skip/Abort |
| Stage / Unstage hunk | Selected tracked text file’s complete diff header; atomic partial index update, working files retained |
| Checkout local branch | Exact local ref; existing dirty/lock/worktree/submodule safeguards |
| Fetch and open remote branch | Exact configured remote ref; fetch then create tracking local or fast-forward pull preview |
| Checkout commit | Exact full local commit OID; detached HEAD is explicit |
| Create branch here | New name at selected OID and checkout; never reset an existing branch or infer upstream |
| Create worktree here | New folder at the exact commit; new/existing local branch or detached HEAD, reviewed destination/occupancy, source work retained |
| Create tag | Reviewed annotated or lightweight tag at selected commit; preserves checkout/index |
| Push selected tag | Exact tag object, one destination, live conflict check, no force or implicit branch/tag following |
| Create/open PR or MR | Exact local branch, native host/account connection, source/target review and duplicate reconciliation |
| Delete local/remote branch | Direct named graph actions; existing local review or live remote compare-and-delete, never combined |
| Manage local branch | Rename with native config/reflog migration; recoverable deletion with worktree occupancy checks |
| Manage local tag | Delete with recovery of the exact object, including annotations/signatures |
| Set/change/remove upstream | Explicit fetched remote branch; configuration-only review |
| Push branch | Button beside current branch or direct local-ref menu entry; live single-destination check, full outgoing list up to 500, pinned normal push, optional upstream and validated cache refresh |
| Fetch | Header or graph menu; configured upstream remote or explicit choice, refresh refs while preserving local branches/files |
| Pull current branch | Header or current local-ref menu; native upstream resolution, Fetch, affected-path preview and confirmed fast-forward into the current branch |
| Merge into current branch | Isolated prediction, explicit start, conflict resolution/staged review and native abort with retained recovery data |
| Cherry-pick onto current branch | One selected commit delta; explicit merge mainline, conflict/staged review, original author/date, single-parent commit and saved-edit abort |
| Revert on current branch | Inverse selected changes as a new commit; explicit mainline, conflict/staged review and retained history |
| Copy commit/branch/tag identity | Full commit SHA or scoped ref name; clipboard errors are shown |

Existing inspector actions own file staging/unstaging, staged review, commit and reviewed stash workflows. Stashes live in Working changes, preserving the graph rather than adding a global stash screen. Related repository items still own initialize/restore recorded submodules. No parallel navigation is added for these operations. Menu placement stays within the viewport, scrolls when needed and supports keyboard navigation, focus return and Escape.

Checkout refuses dirty work, hidden index flags, active operations/native locks and detached commits not retained by any local branch. A retained ancestor can be left safely. Creating a branch at the current detached commit preserves it and is allowed. Ignored files use Git's no-overwrite-ignore protection; submodule checkouts are not recursively changed. Preview runs no hooks, filters or network fetches. Confirmed writes may execute configured hooks/filters; failures and partial outcomes are reported in repository-scoped feedback and durable receipts containing the requested kind/name/OID. Offline graph writes do not trigger an implicit fetch.

## Remaining important actions

This is a coverage plan, not a claim that every planned feature is implemented. Add each item only after its real backend, preview, recovery and temporary-repository regression coverage exist. Do not add inactive placeholders to the menu.

| Priority | Capability | Required interaction and safeguards |
| --- | --- | --- |
| Later | Advanced push destinations | Multi-push-URL selection, nonstandard mappings and explicit signed/tag publication need separate reviews |
| Later | Ref recovery UI and tag rename | Current deletion receipts retain recovery refs; a full restoration/configuration workflow remains separate |
| Later | Topology-preserving rebase, squash, edit/drop/reorder commits | Show rewritten range, published-history consequences and recoverable ref before execution |
| Later | Reset soft/mixed/hard | Explicit mode, affected index/files and unpublished commits; protect work before destructive variants |
| Later | Signed local tags | Explicit signing identity and actual signing failure handling |
| Later | Apply/export patches | Exact selected diff, destination and validated file effects; path safety |
| Available | Remote branch deletion | Live single-destination preview, exact expected OID, local recovery and separate local/remote actions |
| Later | Hosted merge/checks and provider diffs | Current PR/MR create/open exists; add approvals/CI, guarded hosted merge and comparison pagination |
| Later | Solo/ref filtering and comparisons | Existing graph scope with explicit active filter and recovery to full history |

Workspace commit search is available through Cmd/Ctrl+K and opens exact graph context without checkout. File/code search, worktree prune/relocation and the broader multi-repository roadmap remain required independently of context-menu coverage. Reviewed worktree removal/metadata maintenance and selected initialized-submodule URL synchronization use existing Related repositories/Manage rows. Completing these graph actions does not by itself complete the multi-repository MVP.

## Working count

Status and the working inspector explicitly request expanded untracked files with rename detection disabled. Counts describe the same changed-path entries; an untracked folder is not a single file. Staged and unstaged changes to the same path count once in the total. The current per-path rename presentation lists source removal and destination addition separately and therefore counts those two paths. Oversized reads fail explicitly rather than returning a partial count as a complete result.

## Ref management and publication

Manage a branch or tag in the existing contextual menu. Deletion shows a deterministic recovery ref before confirmation. The native ref transaction creates/verifies that recovery pointer and deletes only the expected old object. Recovery refs retain unpublished commits and original annotated tag objects, are hidden from the normal graph and are not implicitly pushed. Branch deletion also removes local branch configuration; restoring a tip later does not automatically recreate every old setting. Receipts persist the reviewed object, names, upstream and recovery location. Native rename uses no-force branch move with before/after validation, retaining config/reflog; it is not a compare-and-swap transaction with arbitrary external Git clients.

Push checks the live push URL using standard Git transports and configured credentials/SSH, without running pre-push hooks or writing refs. Apply revalidates that review and sends a single explicit immutable commit-to-branch refspec with no force, mirror or tag following. Native submodule checking does not publish child repositories. Remote changes before apply invalidate the token; changes during the transfer remain protected by native push's fast-forward/ref checks. This is not an atomic transaction with a remote server and local configuration. Successful publication may remain even if subsequent cache or upstream steps fail.

Cached tracking updates require exactly one standard fetch mapping and matching fetch/push URLs. Optional upstream configuration is reviewed explicitly. Embedded credentials/query parameters in push URLs (use credential helpers/SSH), multiple push URLs, chained rewrites, custom transports, mirror remotes, configured push options, same-repository local destinations, incomplete outgoing histories and inherited tracking configuration, forced duplicate branch checkouts and incompatible worktree-specific settings stop the supported workflow. Up to 500 outgoing commits must fit the complete review budget. Fetch destination history first if its advertised OID is missing locally or is not an ancestor of the selected source.

## Recovering a deleted local ref

Use Copy recovery ref in the repository's result feedback, or find the named recovery location in the operation receipt. For a partial result, inspect whether the recovery ref or original name exists first. To restore a branch tip under a fresh name, pass the copied recovery ref as the start point to `git branch`. To restore an annotated tag, recreate the tag ref directly at the recovery object rather than using `git tag -a`, which would create a different tag object. For example, replacing `PASTE_ID_HERE` with the copied identifier:

```sh
git branch recovered-branch 'refs/gitnova/recovery/PASTE_ID_HERE'
git update-ref refs/tags/recovered-tag 'refs/gitnova/recovery/PASTE_ID_HERE' ''
```

Both examples require a fresh destination name. Branch settings removed during deletion are not automatically recreated; the receipt's prior upstream can be configured again through Manage branch. Recovery refs are never automatically expired in this increment.

## Reviewed merge

See [ADR 0009](../adr/0009-owned-reviewed-merges.md). Right click the source commit/ref, choose Merge into the current named branch and review the predicted paths/conflicts. After Start merge, Working changes shows actual conflicts and staged results. Select a conflict, choose a side or edit supported text in the existing diff dock, then review/apply that exact path. Review merge commit shows the actual full staged result before explicit Finish merge. Review abort lists affected paths and the private recovery location before native abort. Clean non-fast-forward merges also wait for staged review; fast-forward has its own explicit confirmation. No auto-stash, force-reset or submodule movement is inferred.

## Cherry-pick and revert

See [ADR 0010](../adr/0010-reviewed-cherry-pick-and-revert.md). Use the selected graph commit and existing start review, then resolve/review/finish in Working changes. Mainline selection is explicit for merge commits. Abort discloses native reset --merge restoration and the saved-edit location; refusal does not trigger a forced reset. Single-commit operations create one parent and preserve source history. Cherry-pick retains original authorship/date. The bounded linear Rebase sequence is implemented separately; see [M56](../milestone-56.md). Arbitrary multi-commit cherry-pick sequencing remains open.
