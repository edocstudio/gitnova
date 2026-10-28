# M50 — First-project publication in the existing Git workspace

A repository with no commits can now use Working changes → Stage all → Review commit → Commit staged changes, configure its destination in Settings → Repository remotes, then use existing Push with an explicit upstream choice. No dashboard, new navigation item or publication wizard is introduced. The graph remains the current repository context.

## Staging and commit scope

Stage all first reviews the exact eligible unstaged file list, with exclusions for submodules, directories and other paths needing separate workflows. Ignored files remain excluded. The token covers original index/HEAD, status, working contents, configuration and attributes. Confirmed execution stages literal NUL-delimited reviewed paths into a temporary index under the native lock, revalidates, then publishes. It never stages an entire folder through an unreviewed recursive pathspec. Filters execute only after confirmation; a failed filter leaves the original index in place.

Normal commit review separates the complete staged manifest from bounded code. Large combined patches retain all staged paths and load code one selected file at a time against the same staged token. Confirmation states that all listed staged content is included. A stale index/HEAD refuses both per-file preview and commit. Existing hooks/signing behavior, write receipts and native locking remain. Amendment retained a stricter complete-code-preview bound at this milestone; M59 replaces it with the same complete-manifest/per-file delta policy.

Bounds remain explicit: 2,000 changed paths, 64 MiB index, 16 MiB per-file hash, 256 MiB total Stage all working content, 512 KiB selection/patch reads. This is not unrestricted staging for arbitrarily large repositories. Unrelated unstaged files remain outside a commit.

## Remote configuration and Push

Repository remotes are scoped to the repository/common Git directory, including linked worktrees. They are separate from global hosting/CI accounts. Empty Push offers a shortcut to this existing Settings section. Review shows the exact name and clone URL, with original-config recovery location. Saving validates transport, credential-free URL and configured insteadOf expansion; prepares config under its native lock; saves a private original copy; revalidates; and publishes atomically. Existing config locks, duplicate names, stale previews and unsafe transports are refused.

Saving makes no server contact, creates no hosted repository and performs no fetch/push. Create an empty hosted repository first, configure the exact clone URL, close Settings and use Push. Existing destination review, first-publication indication, native Git credentials/SSH, pre-push hooks and optional upstream configuration apply. Hosting/CI account connection does not grant native Git authentication automatically.

## Verification

All Git mutations ran in disposable repositories; no developer repository was staged, committed or pushed.

- Nine new core integration checks: 383-file root commit and first push to local bare remote; ignored-file exclusion; stale content and external index lock; partial staging/deletion/literal special paths; nested independent repository exclusion; unsafe/rewritten URL and duplicate remote refusal; stale config/external config lock; stale commit-file review; linked-worktree shared config; and clean-filter preview/execution failure isolation. The first test combines several assertions; the suite contains nine test functions.
- Fifteen existing write integration checks passed, including the revised large normal-commit complete-manifest acceptance case.
- TypeScript, all 32 frontend unit/catalog tests, strict Git/core/desktop Clippy across all targets passed.
- Single-worker browser tests exercise the real Rust core through the test transport: normal stage/unstage/commit regression, stale Stage all with untouched unborn index, and the 383-file initial-publication flow through Settings and Push. Browser selectors were corrected to use the existing Working changes entry and accessible diff/review labels; the workflow uses no mocked Git data.

Native WebView interaction and live GitHub authentication/publication are separate acceptance steps. A local bare remote proves Git behavior without publishing user source. Local macOS packaging uses the existing builder's signing/embedded-asset checks; public notarized distribution remains outside this milestone.

Local macOS debug bundle built and verified: `target/debug/bundle/macos/GitNova Updated.app` (80.18 MiB). Signing and embedded UI checks passed. The builder chose the Updated bundle because the main bundle was in use; it launched no GUI and left the running app in place.
