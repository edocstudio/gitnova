# M40 — Reviewed Amend in the existing commit composer

Amend last commit is an explicit checkbox in Working changes. The ordinary
new-commit draft stays separate; choosing Amend preloads only the original
commit metadata/message, not its potentially large historical patch. Review
shows the old HEAD, preserved author and all parents, exact staged delta, new
message and recovery ref in the existing composer/diff dock. Message-only and
intentional empty-commit amendments retain their tree/commit instead of dropping
history. An empty replacement is labelled explicitly. The composer scrolls and
keeps confirmation visible at the 780×560 minimum window size.

The native preview binds branch/HEAD, original object/authorship, exact index/tree,
message, effective configuration, cached remote/tag references and recovery
identity. It computes the candidate tree from a copied index in the isolated
scratch repository; it runs no checkout filter, hook or signing program. A split
index is refused in this first slice. Active operations, detached/unborn contexts,
ambiguous worktree occupancy, symbolic/colliding recovery refs and oversized or
unsupported staged deltas cannot confirm. A changed HEAD must be selected again.

Before native `commit --amend`, retain the original under a recovery ref and save
an operation plan. Use the established isolated-index writer: never implicitly
add, stage all, reset or copy working files into the commit. Preserve native author
and author date, every original parent (including root/merge cases), the reviewed
tree and verbatim message bytes, including final-newline presence. Native hooks
and configured signing run only on confirmation. A changed resulting message,
tree, parents, author or extra staging produces an unknown outcome with explicit
inspection guidance; no rollback or automatic retry is performed.

Tags/cached remote refs referencing the original appear in review and require an
explicit rewrite acknowledgment in both UI and native execution. This offline
inspection cannot prove that history was never pushed. It never moves a tag,
pushes or force-pushes; an already-published amendment may be refused by the
normal non-force Push flow. Original signatures remain on the retained original;
a replacement uses configured signing. The receipt and Recent activity retain
its recovery ref after restart, and the current recovery-copy action understands
Amend and remote-branch deletion plans.

Verification is serialized with one Cargo job / one browser worker. Real
repositories cover message-only root commits, staged plus unstaged/untracked
work, authored identity/date, merge parents, intentional empty root/linear
commits, cached-remote acknowledgment with no publication, request/index/config/ref
staleness, detached/native-operation/split-index refusal, linked worktrees,
intent-to-add preservation, symbolic recovery refusal, and hook-modified
message/staging outcomes. Commit regression checks exercise the shared writer.
Browser tests cover ordinary Commit, separate drafts, message-only amendment,
referenced-history acknowledgment, exact staged scope and minimum-window action
visibility. Native Tauri coverage exercises all three new commands and rejects
external origins.

Boundaries: no older-commit amendment, author-reset/editor choice, implicit
force-push, split-index support or full public-platform acceptance is claimed.
Rebase and per-hunk staging remain open; the versioned rebase lifecycle design is
in [durable-rebase.md](design/durable-rebase.md). This milestone does not establish
whole-app readiness or large-repository/long-session memory performance.

Final verification: 11 real-Git Amend cases and 6 shared Commit writer
regressions passed. Both Amend browser workflows passed with final source,
including exact minimum-window confirmation visibility; the ordinary Commit
browser workflow passed separately. All 26 frontend checks passed. The final
native Tauri real-Git/origin suite passed, including the three new Amend commands.

[Message-only review](screenshots/amend-message-review.png),
[referenced-history review](screenshots/amend-referenced-review.png),
[minimum-window confirmation](screenshots/amend-referenced-compact.png).

CI follow-up: strict Clippy passed across all core workspace targets and the
desktop. Boxed the large hosted-request plan payload without changing its
serialized shape; the provider regression round-trips that persisted shape.
GitLab duplicate lookup now normalizes numeric/string project identities through
the same validated ID parser instead of comparing JSON formatting. All 7 local
provider contract tests passed, including existing-MR reuse without POST in both
ID representations. No live provider write or credential access was performed.

Final local artifact: `target/debug/bundle/macos/GitNova.app` passed TypeScript,
Vite/native build, ad-hoc signature and embedded-asset verification with the final
source and EN/TH catalogs. It was not launched against a user workspace during
verification. Large-repository and long-session memory measurements are deferred
to the recorded delivery phase, not inferred from this build or the boxed plan.
