# ADR 0032 — Amend inside the existing commit composer

Status: implemented as M40 after the delivery-priority audit.
Clone, tags/provider creation and related-repository operations have landed;
rebase's durable multi-commit lifecycle and hunk staging are still outstanding.
Complete Amend as an independently reviewable history-write flow before exposing
another operation. This is not a claim that the whole feature plan is complete.

Add Amend last commit to the existing composer, not a new screen or global menu.
It uses the same staged-file groups, message fields and diff dock. An independent
amend message draft preserves the user's ordinary new-commit draft. Review pins
HEAD, branch, exact index/tree, message, preserved author and complete parents.
The dock shows Original HEAD → Staged replacement (the additions to the original
commit); message-only amendments explicitly keep its tree. Preserve every parent
of a merge commit, and original authorship/date. No reset-author choice is added.

Keep the old commit under a recovery ref before invoking native `commit --amend`.
Use the normal isolated-index commit writer with no implicit add/all/checkout.
Preview executes no hooks or signing. Confirmation uses configured native hooks
and signing, verifies resulting tree/parents/author/message, and reports unknown
outcomes without rollback if a hook or external writer changes the result.
Cached remote/tag references are displayed and require an explicit history-rewrite
acknowledgment. This offline inspection is not proof that a commit was never
published. Do not push or force-push automatically. Stale reviews are invalidated.

Support message-only, staged plus unstaged, root and merge commits, and linked
worktree identity. Reject detached/unborn/active-operation contexts, split indexes
in this first slice, oversized/unreviewable deltas and changed reviews. Keep
recovery discoverable in operation receipts and recent activity after restart.

Native execution retains intentional empty replacement commits using --allow-empty;
the preview labels that condition and all parent/tree/message checks still apply.
