# ADR 0009 — Owned merges, staged review and retained recovery data

Accepted, 2026-10-26.

## Context

Merging should begin at the selected commit/ref in the graph. The user must see the current destination, predicted paths/conflicts, then the actual working/index result before a merge commit is created. Conflict resolution and abort must not silently lose manual edits. The application's existing graph menu, Working changes inspector and diff dock already provide these contexts.

## Decision

Predict with native `merge-tree --write-tree` in a temporary bare repository. The original object store is a read-only alternate, using Git's C-quoted alternate-path representation; generated trees/blobs and scratch indexes remain temporary. Copy data-only merge settings and attributes, disable hooks, network protocols and custom drivers for prediction. Complete previews are bounded; unsupported configurations stop rather than display a partial plan as complete.

Begin only from a clean named local branch. Retain both tips under `refs/gitnova/recovery/merge-<session>` and its `-incoming` ref. Fast-forward uses native `merge --ff-only`. Divergent histories use native ort with `--no-ff --no-commit`, no automatic stash, no ignored-file overwrite, no recursive submodule change and no rerere application. No merge commit is implicit. Gitlink changes, custom merge drivers, merge options, renormalization, hidden index flags, duplicate branch checkouts, unrelated histories and other active operations stop this first slice.

A private marker and session directory belong to the worktree Git directory, independent of common repository identity. Pending state validates the marker, current branch/HEAD, MERGE_HEAD and ORIG_HEAD. Externally started merges remain inspectable but are not adopted; changed ownership metadata becomes incomplete. Ordinary stage/commit/branch-changing APIs remain blocked by the active Git operation.

The inspector separates conflicts from resulting paths and keeps a merge-message draft per membership/session. A selected conflict can explicitly choose current/incoming content (including deletion), review a marker-free working result, or edit supported UTF-8 regular text in the existing diff dock. The editor shows the current branch read-only beside the proposed result. Vertical synchronization clamps the shorter pane without pulling the active longer pane backward. No file is written until the proposed resolution is reviewed and confirmed. Binary and symlink cases use explicit side selection; no binary text editor. Resolved paths with subsequent unstaged edits can explicitly review and stage them. Final review always uses the actual index after configured Git normalization.

Resolution additionally pins the proposed single-path tree, including binary bytes and mode, in its review token. Resolution/abort revalidate a content-bound token, then save a private filesystem/index archive before changing anything. The manifest records native path identities, file modes, hashes, missing/directory states and symlink target bytes; payloads are regular backup files, not live backup symlinks. A raw index, relevant staged blobs and merge metadata are included. Files use 0600 and directories 0700 on Unix. Budgets are 2,000 paths, 16 MiB per working file, 64 MiB aggregate working bytes, 128 MiB combined working/staged payload and a separate raw index up to 64 MiB; over-budget or incomplete archives stop before mutation. Retrying never overwrites a different archive. Recovery refs and archives are not expired automatically.

Finish previews the entire staged result and binds its tree, both parent OIDs, ownership state and message. Native commit runs with the existing isolated-index/lock publication mechanism; the committed tree, exact two parents and branch are checked afterward. Hooks/signing may run and can fail or change content; postcondition mismatch is reported as uncertain, never automatically rolled back.

Abort uses only native `merge --abort`, after the reviewed snapshot. It may refuse staged/unstaged combinations or collisions. There is no forced reset fallback. Partial failures retain marker/archive and show the recovery location. Repository result feedback exposes the existing recovery-copy control.

## Recovery and limits

Copy the recovery location from result feedback or the durable operation receipt. Verify `manifest.json` exists before using an archive; an absent manifest means capture did not complete. Inspect manifest `entries`: each original native path has a kind/mode/hash and optional payload filename. Regular file payloads contain the prior working bytes. Symlink payloads contain the target bytes, not target contents. `stage_blobs` identifies copied staged object payloads; raw index/metadata are evidence, not files to blindly copy into an active repository. Preserve the archive, inspect current Git state and recover individual files into a separate directory first. Index formats, split-index references, concurrent tools and hooks make automatic raw-index restoration unsafe. A recovery UI is deferred.

Original/incoming commits remain named by the disclosed recovery refs. For example, after replacing the placeholder with the actual ref, a fresh branch can retain the original tip:

```sh
git branch recovered-before-merge 'refs/gitnova/recovery/merge-PASTE_SESSION_HERE'
```

No atomicity is claimed against arbitrary external filesystem/Git mutations. Windows/Linux, native webview automation, executable filters/hooks, LFS and unusual recursive/rename/directory merges require further compatibility testing. The implementation deliberately refuses unsupported submodule-pointer merges. Rebase, cherry-pick, revert and history rewriting remain separate workflows.

## References

- [Git merge-tree](https://git-scm.com/docs/git-merge-tree): write-tree and structured conflict output.
- [Git merge](https://git-scm.com/docs/git-merge): no-commit, ignored-file protection and abort behavior.
- [Git environment](https://git-scm.com/docs/git): alternate object directories and quoted paths.
