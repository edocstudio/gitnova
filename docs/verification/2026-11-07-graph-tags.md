# Remote version tags in the graph

The tag creation inventory reads the selected push destination directly using
`ls-remote`. History uses local refs. Branch fetch previously passed `--no-tags`,
so an annotated remote-only version could be listed in the creation inventory
without appearing beside its commit in history.

Fetch and auto fetch now request tags explicitly. Existing tags are never forced
or pruned. Atomic local ref updates reject a conflicting existing tag without
partially updating other refs; downloaded objects may remain. HEAD, local
branches, working files, index and FETCH_HEAD retain their previous behavior.
Tag inventory inspection itself remains read-only and offline mode is retained.
With distinct fetch/push URLs, each operation still uses its intended destination.

The two-slot graph badge layout prioritizes the current branch, then the newest
version tag, ahead of other branch labels at that commit. Additional refs remain
available in the row tooltip and commit context. No screen or control was added.

Validation on disposable repositories:

- Rust `gitnova-core` integration suites `tags` and `remotes`: 18 passed.
  Includes annotated remote-only tags, warm graph fingerprint invalidation,
  configured `--no-tags` override, local tag collision rejection, automatic
  refresh and preservation of dirty/staged files and receipt behavior.
- Frontend `graph-state.test.ts`: 11 passed, including current branch + newest
  version tag visibility when multiple branches share the commit.
- Targeted browser tests: 2 passed. The actual Rust core supplies Git results;
  browser IPC is emulated. Tests verify remote inventory pagination/duplicate
  checks and a remote-only annotated `0.0.35` appearing in the graph after Fetch.
- No company repository, credentials or remote service was used for these tests.
- `npm run build:local` passed TypeScript/frontend compilation, macOS packaging,
  embedded UI and bundled attribution checks. Installed the result over the
  existing Desktop app only after checking no GitNova process was running;
  strict/deep signature verification passed. Existing large-chunk/dynamic-import
  build warnings remain. No commit, push or release publication was performed.

Historical milestone and verification records describe their implementation at
the time; the current behavior is documented in [workflows](../ux/workflows.md).
