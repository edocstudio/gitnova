# ADR 0007: Scoped graph actions and consistent working counts

Accepted, 2026-10-25.

A graph row and its exact refs provide enough scope for everyday history actions. Use one transient contextual menu at that position, with inline review, rather than adding permanent chrome. Support right click and keyboard activation. Double click initiates an unambiguous checkout preview; it never moves an existing branch to an arbitrary historical commit. A badge supplies its exact ref identity. Ambiguous nodes present choices, and unlabeled nodes explicitly preview detached HEAD. Fetching a selected remote precedes local integration review, matching the existing branch picker.

Core `GraphAction` requests specify a closed enum, full commit OID and exact name. Graph action preview/application remain independent of Tauri. Only local commit objects are accepted; no revision expressions or object downloads. Checkout and create-and-open branch reuse branch impact inspection and dirty/hidden-index/active-operation/lock/submodule safeguards. A new branch uses native nonforce, nontracking switch creation at the reviewed OID. Lightweight tags use compare-and-swap creation with a zero old OID. Ref names are checked through Git, and existing refs are never replaced. Tag postconditions check checkout/index identity as well as the created ref.

Creating a branch at the current detached OID preserves otherwise unretained work. Leaving a detached ancestor retained by a local branch is safe and is allowed; checking only equality to branch tips was unnecessarily restrictive. Unretained detached work still requires preservation before leaving.

Durable receipts include kind/name/OID, membership/repository/worktree identities and before/after HEAD. Confirmed hooks can alter results; postconditions and honest partial/unknown recovery remain required. Preview does not execute hooks/filters or fetch. Tests write only temporary repositories. Existing concurrent-filesystem limitations remain: snapshot validation and Git native locking are not an atomic transaction with arbitrary external programs.

Working counts explicitly enumerate untracked files, and status and inspector use the same rename-disabled per-path representation. Files with both staged and unstaged changes count once overall. Renames remain two path entries until the per-file operation model supports paired rename identity. Oversized output fails explicitly. Empty history reports zero loaded commits instead of a 1–0 range.

Alternate index copies preserve the original modification timestamp during preparation, and the native output index timestamp during publication. Copying bytes with a later timestamp can bypass Git’s racy-clean detection for same-size edits. This applies to both per-file index changes and commit publication; do not work around the defect by sleeping or changing test content sizes.

See [interaction/coverage plan](../ux/graph-actions.md) for available actions and remaining push, integration, rewriting and provider work. This ADR does not claim those future backends exist or that the multi-repository MVP is complete.
