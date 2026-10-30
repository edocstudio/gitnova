# M76 — Explicit newer submodule revision review

The initialized submodule's existing Manage context adds Review newer version…,
separate from Restore recorded. It uses the configured child remote and branch
selection, but **only refs already fetched locally**. Open/fetch the child first
when freshness matters; neither preview nor confirmation fetches or runs custom
submodule update commands. This is not an automatic remote update or a claim to
know the live server's latest commit.

Selection follows native Git's child `branch.<current>.remote` with origin as its
fallback, parent-local `submodule.<name>.branch` before `.gitmodules`, `.` parent
branch inheritance, and cached remote HEAD when no explicit branch is set.
Nonstandard local-dot remotes and unavailable/ambiguous targets are refused.
See [native Git documentation](https://git-scm.com/docs/git-submodule).

The existing preview identifies parent branch, parent index revision, checked-out
commit, exact remote ref, exact target commit and a complete bounded changed-path
list. Confirm switches only the selected child checkout, detached, without moving
local branch refs or staging the parent's gitlink. Parent HEAD/index, unrelated
staged/unstaged work and nested physical checkouts remain in place. The updated
parent pointer appears as an unstaged change; Stage pointer remains a separate
reviewed action.

Only a proven forward ancestry from the current child checkout is supported.
Dirty/untracked work, active operations, native locks, hidden index flags, missing
objects, shallow/unknown ancestry and stale preview/config/ref changes refuse the
update. A target changing nested gitlinks or `.gitmodules` requires investigation
inside the child instead. Complete changed-path evidence is limited to 1,000
paths and 256 KiB; no partial manifest authorizes a checkout.

This uses the existing selected-submodule operation/index guards, revalidation,
nonrecursive native checkout, parent preservation checks and operation receipts.
No new screen, global setting, credential scope, branch deletion or batch write is
introduced. Confirmed checkout may run configured native checkout hooks/filters;
preview does not. Native Git's non-force checkout protects ignored collisions;
postcondition failures require inspection and do not promise rollback.

Submodule add/remove, automatic fetch+update, nested layout transitions and
arbitrary divergent child history remain separate work. M56/M71's lack of newer
revision support is historical and superseded only for the scope above.
