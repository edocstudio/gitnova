# Working-change preparation journeys

Date: 2026-11-06. Local macOS verification; temporary repositories and local
remotes only. No developer project was stashed, switched or published.

## Behavior

Parent working changes that block checkout, branch creation at another tip,
Pull, Merge, Cherry-pick, Revert, Rebase or Apply/Pop now open the existing
Stash review inline with the requested action. Saving requires confirmation.
After a successful save and refresh, the original action receives a fresh
preview and still requires its own confirmation. Cancellation leaves it blocked.

Creating a branch at the current tip offers an explicit choice to carry staged
and working changes instead. Fetch, Push and tag creation do not require an
unnecessary stash. Dirty child repositories, active operations and locks retain
their own scoped review/recovery paths; parent Stash cannot resolve those states.

Pop retains the selected stash object ID across preparation: creating a new
stash shifts indexes, so the original object is located again before previewing.
There is no automatic original operation, automatic Apply/Pop or discard.

The component mounts the existing Stash UI lazily and loads code per file through
the existing bounded preview. StashControls remains in the main bundle because
WorkingDetails also imports it statically; this is not a bundle-size reduction.
UI copy is available in English and Thai.

## Checks executed

- `cargo fmt --all`: passed.
- `cargo test --locked -j 1 -p gitnova-core --test graph_actions --test branches -- --test-threads=1`:
  17 passed. The new native case verifies current-tip branch creation preserves
  staged, modified and untracked content while another-tip creation stays blocked.
- `npm run check`: passed.
- `npm test`: 68 passed across 17 files.
- `cargo clippy --locked -j 1 -p gitnova-git -p gitnova-core --all-targets -- -D warnings`:
  passed.
- `git diff --check`: passed.
- Targeted Playwright tests, one worker and real Rust core: 11 passed across
  the final targeted runs. Coverage includes stash/carry/cancel branch creation,
  Switch, Pull, Merge, Cherry-pick, Revert, Rebase and Pop after index shifting;
  existing graph checkout, cross-branch Pop and diverged Pull guards also passed.
  An initial Create-branch case exposed stale UI dirty state; reading fresh
  working changes when preparing the review fixed it, and the rerun passed.

These are targeted checks, not a full cross-platform acceptance run. Existing
uncommitted graph-label refinements are retained separately in the working tree.

## Local artifact

`npm run build:local` passed. Installed over the single Desktop `GitNova.app`
after checking it was closed. Strict/deep signature verification and installed
`--verify-bundle` passed; executable SHA-256 matches the built artifact:
`64a311d61e77428d5d16fafa4d36409a87d79b0f18fc35efdcda13b446815c06`.
This is a debug, ad-hoc signed macOS alpha, without notarization. Build warnings
include the existing large main chunk and the shared StashControls import noted
above. Attribution inventory reports 50 unresolved entries; it is not a completed
license-compliance audit. No commit, push or release was performed.
