# ADR 0013 — Reviewed Stash, Apply, Pop and recoverable Drop

Status: accepted for the first stash increment, 2026-10-26.

## Interaction and scope

Working changes owns uncommitted content. Its Stashes control opens a mode within the existing inspector; the graph and bottom diff dock remain in place. No dashboard, global Stashes sidebar or automatic stash-before-checkout is introduced. See [surface-budget.md](../ux/surface-budget.md).

The create form names the stash and explicitly chooses whether to include untracked files. It captures staged and unstaged tracked content together; ignored files are excluded. Preview lists all selected paths and offers Saved working tree / Saved index comparisons in the existing dock. Apply retains the saved entry; Pop removes it only after successful application; Drop removes the saved entry without applying it. A confirmation identifies the current branch/HEAD context, exact saved OID/index, scope and recovery reference. Restoring the saved staging state is explicit and enabled initially. All product text has English and Thai catalog entries; saved messages, paths and code remain original data. The ordinary commit composer draft remains mounted while this mode is open.

Stashes belong to the underlying repository and are shared by linked worktrees. Listing uses real Git reflog entries, capped at 200 visible entries with an explicit more indicator. Requests bind the selected index to a full immutable stash OID. They do not accept revision expressions or arbitrary paths. Stash commits and their index/untracked parents are excluded from ordinary graph/search traversal roots, alongside existing internal recovery refs.

## Preview and validation

Typed StashRequest/Preview/List DTOs cross core/IPC; the Git adapter remains independent of Tauri. A versioned preview prevents the UI from confirming against an older native implementation. Reads disable configured filters/fsmonitor, external diff/textconv, signature verification and implicit network fetch through the existing bounded adapter. No hooks run during preview, and no Git write is used to predict an application result. The preview shows the captured delta; it does not claim that applying to another base will be conflict-free.

The token includes request/operation identity, working/index/HEAD state, captured working/index patches, selected content hashes, configuration, complete visible list and the original reflog/ref digest for removal. Revalidation occurs inside the existing common-repository operation guard. Native index/HEAD locks and active operations block unsafe writes. Hidden index flags, existing conflicts, changed gitlinks/nested repositories, malformed stash parent structure and incomplete previews stop this increment. Applying requires a clean working tree and checks ignored/untracked/file-directory/symlink collisions at affected paths. Push blocks deleted index paths that still contain local files, preserving data that Git reset cleanup could otherwise replace.

**Current extension:** [M85](../milestone-85.md) supersedes this increment's
aggregate-code completeness requirement. Complete bounded path/content identity
is still required, but code text may be partial and selected patches load lazily.
Working/index truncation is explicit, saved index-only paths are included and
all three stash deltas are checked for gitlink changes. Other write/removal and
recovery guards in this decision remain applicable.

A stash shape has two or three parents: the recorded base, an index commit based on that base and an optional root commit for untracked contents. Complete working/index patch reviews are limited to 512 KiB each. Existing 16 MiB per-file hashing, 2,000 changed-path and bounded metadata limits apply. Larger cases require Git inspection; truncation never enables confirmation.

## Execution and recovery

Core persists a planned receipt before execution and uses the existing common-repository guard. All writes use fixed argument arrays. Configured native filters/hooks can run after confirmation. No push, fetch, auto-stash, force reset or fallback cleanup is inferred.

Push uses native `stash push`, with an operation UUID in the saved message to identify its own new entry even if another client changes the stack. It pins that stash to the reviewed internal recovery ref and verifies HEAD and cleanup state before reporting success. Apply uses the immutable OID, optionally with `--index`, after pinning a recovery ref. Pop follows the same apply step and returns immediately on failure/conflicts, retaining saved data. Recovery ref creation uses no-dereference semantics. Existing refs are not replaced silently.

The stash-specific native writer omits the global literal-pathspec option: this API accepts no UI pathspecs, and Apple Git 2.39's internal stash cleanup leaves untracked files behind when run with that option. A real temporary-repository reproduction and Unicode/untracked roundtrip tests establish this behavior. Other native write callers retain their existing literal-pathspec protection. This exception is restricted to fixed stash/ref commands; user text remains a single message argument.

Native application failures can leave working/index changes. They are reported as conflicted or unknown with no automatic retry/rollback. A conflict retains the original stash and recovery object. **This increment requires Git for stash conflict resolution**; it does not pretend the existing owned merge editor owns a native stash conflict or offer an unsafe automatic abort. The working inspector preserves the recovery guidance when returning from the stash mode.

## Exact entry removal

A numeric stash selector can move when another Git client changes the reflog. Pop/Drop therefore do not run a positional native drop against the live repository after a separate OID check.

For the supported files backend, capture the exact original loose stash ref/reflog before applying. Use native Git reflog deletion/rewrite in an isolated scratch object store with read-only alternates to generate the proposed metadata. Save original/proposed metadata in a private per-operation directory, flushing files before live publication. Pin the selected stash object separately for recovery. Acquire native packed-ref, stash-ref and reflog lock files; under those locks revalidate the exact original metadata. Preserve the original ref/reflog file permissions and publish the proposed metadata with individual atomic renames, or delete the loose ref for the last entry. Keep every other reflog entry. A published lock guard never deletes a subsequent writer's lock.

Git's files backend protects reflog changes with the reference lock and publishes the reflog before its updated ref; see the primary [Git files-backend implementation](https://github.com/git/git/blob/v2.39.2/refs/files-backend.c). The scratch rewrite delegates record formatting/relinking to Git. Packed stash refs, reftable/other ref storage, symlinked or inconsistent metadata, reference-transaction hooks and stacks above 200 entries stop Pop/Drop with explicit guidance rather than bypassing their semantics. Apply and Push retain native Git behavior within their own validation boundary.

This is **not an atomic transaction across the worktree, reflog and ref**. A process/storage failure between publications may leave partial metadata. Receipts disclose the recovery reference and original/proposed metadata directory. Normal Git writers are coordinated through native locks; arbitrary direct filesystem edits remain outside that guarantee. Recovery snapshots/refs are not expired automatically. After inspecting a partial outcome, a retained stash can be reintroduced under Git's own semantics, replacing the example ID:

```sh
git stash store -m 'Recovered GitNova stash' refs/gitnova/recovery/stash-PASTE_ID_HERE
```

Do not blindly copy an old reflog over newer activity or repeat apply after a partial Pop. Inspect local changes and recovery data first. The source behavior and native conflict/index semantics are documented in the primary [git-stash manual](https://git-scm.com/docs/git-stash).
