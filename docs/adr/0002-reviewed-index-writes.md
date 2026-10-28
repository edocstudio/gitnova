# ADR 0002: Reviewed index writes in the existing graph context

Accepted for the everyday Git milestone.

## Interaction

Uncommitted changes are represented by one row above the commit graph. Selecting it reuses the existing contextual details area. Stage/unstage acts on the selected path only. Commit requires a user-authored message, a complete staged patch review, and an explicit confirmation in that same area. No dashboard, permanent changes sidebar, modal stack, stage-all or discard action is introduced.

## Git correctness

Keep WorkingTreeReader, IndexWriter and CommitWriter independent of the desktop host. Paths are native encoded relative paths found in the current Git status, never shell fragments. Reject path traversal, directories/gitlinks, unresolved conflicts, bare repositories and in-progress merge/rebase/cherry-pick/revert operations. Recheck the registered repository/worktree identity before writing.

Use SHA-256 tokens bound to index bytes, HEAD and symbolic HEAD. File tokens additionally bind the selected path, mode and current file/symlink content. Stage/unstage prepares an alternate index while holding Git's native index.lock, revalidates the original state/content, then publishes the prepared index through the lock path. Unborn unstage removes the selected entry only from that temporary index. Working files are never restored or deleted by unstage.

Commit also holds the native index lock and invokes native Git with an alternate index copied from the reviewed index. This prevents unrelated external index changes from slipping into that commit. Hooks/signing retain normal Git behavior. Successful commit publishes the resulting temporary index, including legitimate hook changes. A configured hook may alter content or other files; the review is a Git content review, not a sandbox or a runtime-safety guarantee. Native ref locks/CAS still apply. Revalidation cannot make arbitrary external ref or filesystem edits atomic with the entire operation.

A separate common-directory GitNova lock coordinates app writes across linked worktrees and instances. Never adopt or automatically remove another process's lock. Write started/final receipts with repository, worktree, membership, timestamps and before/after HEAD under app data. On timeout/failure, inspect resulting HEAD and report failed/unknown with recovery guidance; never automatically retry, roll back refs or claim no mutation from an exit code alone.

## Read side effects

Working-tree diff/status can invoke clean/process filters even when external diff and textconv are disabled. Enumerate configured filter drivers using config-only reads, then override their commands and required flags for all inspection commands. Do not run hooks/filters during previews. Raw working-file status/diff can differ from filter-normalized results; the staged commit review uses actual index content. Stage execution may run configured clean filters, commit execution runs configured hooks/signing. Disable read-side network protocols and lazy object fetching.

## Bounds and deferred work

Initial limits: file hash 16 MiB, index 64 MiB, 2,000 changed paths, patch 512 KiB, write subprocess 60 seconds. A truncated commit preview cannot be approved through either UI or core API. Unix timeout cleanup targets the owned Git process group. Windows helper-tree cleanup is not yet certified. Polling and multiple Git subprocesses cost more than an event-backed/native-reader design; no performance target is claimed.

No hunk staging, amend, conflict resolution, destructive worktree edits, network writes, gitlink writes or batch writes. Interrupted-operation recovery UI and workspace-catalog multi-instance reconciliation remain future work.

## M50 update: complete manifests and reviewed Stage all

The initial limits above describe the original milestone. Normal commit approval now requires a complete staged manifest, distinct from a bounded code preview. If the combined patch exceeds 512 KiB, retain the complete staged file list and load one exact staged path at a time, validating the same index/HEAD token before and after each load. The confirmation explicitly includes all listed staged content. An incomplete manifest still fails closed. M59 extends the same complete-manifest/per-file delta policy to amendment, with existing exact-tree validation, referenced-history acknowledgment and recovery intact.

Stage all is a reviewed operation in the existing Working changes composer. Show the exact eligible unstaged paths and excluded paths before confirmation. Bind the selection, contents, current index/HEAD, configuration and attributes to a token. Apply the literal NUL-delimited reviewed path list to a prepared index while owning index.lock, then revalidate before publishing. No recursive `git add .`, ignored-file inclusion, automatic child staging or external-lock removal is permitted. Configured clean filters run only during confirmed execution; failures retain the original index. File hashing remains limited to 16 MiB, total working content to 256 MiB, and changed paths to 2,000. These bounds are not relaxed for a first commit.

Repository remote configuration is a separate reviewed Settings operation. Validate the name, source transport and effective insteadOf expansion, prepare the common config under its native lock, preserve a private original config copy, revalidate and publish. It shares configuration across linked worktrees. Saving a remote makes no network request, creates no hosted repository and performs no fetch/push. Existing Push reviews and confirms first publication separately.
