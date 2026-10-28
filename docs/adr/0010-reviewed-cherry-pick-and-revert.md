# ADR 0010 — Reviewed single-commit cherry-pick and revert

Accepted, 2026-10-26.

## Context and interaction

The selected graph commit identifies the source; the current named local branch identifies the destination. Reuse graph context review, the owned-operation inspector, conflict editor and staged review from ADR 0009. Expose Cherry-pick onto and Revert on with explicit operation labels. No new screen, dashboard, sidebar or permanent toolbar is added. A merge commit requires an explicitly selected mainline parent; show its number/OID and explain that reverting a merge affects later merge behavior. Do not guess the first parent.

## Prediction and native application

Extend the existing typed request/state/receipt boundary with operation and optional mainline, defaulting older data to merge. Source OID/ref, destination, configuration, index, attributes and mainline participate in preview identity. Restrict this increment to one selected commit, a clean committed local branch and the existing hidden-entry/native-lock/worktree/configuration safeguards. Refuse changed gitlinks, ignored affected-path collisions, empty results and incomplete previews. Shallow repositories stop until their history is completed: Git can hide a boundary commit’s parents and make a selected delta look like a root addition. Missing parent objects in other partial histories also require explicit fetching outside preview. Source-message drafts must fit a 16 KiB JSON-encoded budget without NUL; owned metadata is bounded to 64 KiB, and mainline selection to 256 parents. User-edited final messages retain the existing 64 KiB commit limit.

Prediction uses deterministic synthetic commits in the isolated bare object store. For cherry-pick, the explicit selected parent tree (or empty root baseline) is the common base; the destination tree is ours and selected commit tree is incoming. For revert, the selected commit tree is the base and the selected parent/empty tree is incoming. Native merge-tree predicts the resulting three-way merge with exactly this base. Synthetic objects have fixed preview identity/date and remain in scratch storage; the source object database is a read-only alternate. Ancestor changes outside the selected commit delta are not imported.

Apply uses native cherry-pick/revert with --no-commit and ort. It never advances HEAD or commits implicitly. Compare a clean staged result with prediction. Real conflicts use the existing stage-aware file inspection, side choice, bounded text editing and recovery snapshot workflow. For revert, the incoming-side action is labelled Use pre-commit version because that side represents the selected parent. Display mainline context in the inspector as well as start review.

## Ownership and finish

Git no-commit operations do not consistently leave native sequencer/head markers. The existing per-worktree private ownership marker therefore records a preparation phase and the exact native head/original metadata after application. Publish readiness with an atomic metadata rename only after a successful native apply or observed conflicts. A crash or unexpected state before readiness remains inspect-only. Validate original HEAD/branch, operation, native marker bytes and absence of an external sequencer. An external operation is never adopted. Ordinary write APIs also refuse the ownership marker, including clean no-commit states with no native Git operation marker.

Finish reviews and binds the actual complete staged tree. Use the isolated locked-index commit mechanism, preserving unstaged work. Cherry-pick explicitly supplies the selected commit's original author and author date; the current identity remains committer. Revert uses the current identity. Postconditions require exactly one original-HEAD parent, the reviewed tree, the same branch and cleared native operation markers. Merge keeps its two-parent postcondition. Empty resolved results stop for explicit abort instead of creating an inferred empty commit. Hooks/signing can fail or change state; report uncertainty without automatic rollback.

Receipts retain operation, selected source, mainline, original tip, expected tree and recovery locations. Existing internal merge-prefixed IPC/type names and recovery paths are retained for compatibility; user-facing text identifies the actual action.

## Abort and recovery

Review abort identifies paths, current saved edits and the retained original commit before confirmation. Reuse ADR 0009's private bounded working/index/staged-blob archive, now also preserving CHERRY_PICK_HEAD and REVERT_HEAD metadata. Both source and destination tips remain under recovery refs.

Because a clean --no-commit operation may have no native sequencer to abort, the reviewed single-commit cherry-pick/revert abort explicitly uses `git reset --merge` at the pinned original OID, with submodule recursion disabled. This is the disclosed restoration action, not a fallback after a failed abort. HEAD must still equal the original tip before execution; no branch movement to an unreviewed target is permitted. There is no --hard, automatic stash or force fallback. Git can refuse staged-plus-unstaged edits; retain the archive and current contents, report unknown/partial state and require inspection. Successful abort verifies original branch/HEAD and cleared native markers before removing ownership metadata. Merge continues using native merge --abort.

Working/index recovery remains manual as documented in ADR 0009. Archives are not silently restored, replayed or expired. Multi-commit sequences, skip/continue of external sequencers, rebase, unusual hook/filter/LFS configurations and native cross-platform automation remain outside this increment.

## References

- [Git cherry-pick](https://git-scm.com/docs/git-cherry-pick): selected changes, mainline and no-commit behavior.
- [Git revert](https://git-scm.com/docs/git-revert): inverse commits, mainline and effects on later merges.
- [Git commit](https://git-scm.com/docs/git-commit): author/date overrides and commit identity.
- [ADR 0009](0009-owned-reviewed-merges.md): staged review, index publication and private recovery limits.
