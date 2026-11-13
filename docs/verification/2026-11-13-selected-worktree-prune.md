# Selected missing worktree recovery — 1.0.14

The old preview required every other registered checkout to be discoverable.
With multiple missing temporary checkouts, each selected prune was therefore
blocked by the others. This guard predated the selected-only archival mutation.

Remove that unrelated checkout discovery requirement. Keep locator validation,
selected missing/prunable/unlocked checks, metadata limits, preview tokens,
recovery archival, and post-write verification. No global worktree prune runs.
An all-zero HEAD represents no recoverable commit; archive its metadata without
trying to create an invalid recovery ref.

Regression fixtures cover sequential pruning with multiple missing checkouts,
a missing registration whose branch no longer exists, a locked missing checkout,
a live dirty checkout, unchanged branches/index, stale preview rejection, and
picker refresh after successful confirmation. No developer checkout registrations
or branches are removed by this verification.

## Validation

- macOS arm64, Apple Git 2.39.2, Rust 1.99.1, Node 26.13.0.
- `cargo test -p gitnova-core --test related_maintenance -- --test-threads=2`: 17 passed before the zero-HEAD refinement.
- Final `related_maintenance prune_` selection: 3 passed, including zero HEAD.
- Playwright selected missing-worktree confirmation/automatic picker refresh: 1 passed using real disposable Git repositories.
- `node --test apps/desktop/scripts/set-version.test.mjs`: 1 passed.
- License inventory regenerated; this is inventory evidence, not a complete license audit.
- Windows/Linux execution and the full application test suite were not run for this patch.
- Native optimized macOS release build and embedded bundle verification passed.
- Installed over Desktop/GitNova.app, verified version 1.0.14 and ad-hoc signature (not notarized).
- Installed executable SHA-256: `7037aaae4dca08a51767a7981abe9206b4649e5a530fa02f94ead84c4ada0891`.
