# Remote-only tags in the graph — 2026-11-12

Version: 1.0.9. Synthetic repositories only; no company repositories,
credentials, commit history or remote settings were modified for verification.

## Cause and behavior

The tag creation inventory inspects the publication remote, while the graph
previously decorated rows using only local refs. A remote-only annotated tag
also advertises a tag-object ID distinct from its peeled commit ID.

The graph now reads remote tag advertisements independently of local graph
loading. It matches peeled IDs to loaded commit rows, labels remote-only tags
with their remote, and combines a matching local/remote tag into one badge.
Selection uses the configured upstream fetch remote, otherwise origin or the
sole remote. Distinct push/fetch URLs remain deliberately distinct.

This inspection does not fetch objects, create tags, alter index/worktree,
change HEAD, or authorize Git writes. Existing tag create/push validation stays
in place. Background transport follows the non-interactive fetch policy. A
failed inspection is visibly unavailable/cached evidence, never an assertion
that the remote has no tags.

Responses are limited to 4 MiB. The native cache has a 4 MiB weighted capacity
and 60-second TTL, scoped by repository common directory, fetch URL, remote and
ref fingerprint. The frontend indexes labels by commit, polls every 60 seconds,
cleans up timers/listeners and ignores late results after scope changes. No new
panel or navigation is introduced.

## Checks run

- `cargo test -p gitnova-core --test tags -- --test-threads=2`: 7 passed.
  Covers annotations, packed refs, fetch/push endpoint separation, URL/cache
  isolation, unavailable remotes, inventory duplicates, fetch integration and
  preservation of working files/staging/history.
- `cargo clippy -p gitnova-core -p gitnova-ipc --all-targets -- -D warnings`:
  passed.
- `npm run check`: passed.
- `npm test`: 21 files / 83 tests passed.
- `npx playwright test -g 'remote-only annotated tags|late remote tag evidence|tag inventory discovers remote-only'`:
  3 passed, one worker. UI uses the real Rust core and disposable Git repos;
  browser IPC is emulated. Verified visible `0.0.35 origin` before Fetch,
  no accidental checkout on double-click, one badge after Fetch, duplicate-name
  checks, and no late tag evidence leaking to another repo.
- `cargo test -p gitnova-desktop real_git_flows_through_tauri_commands_and_remote_origins_are_rejected -- --test-threads=1`:
  1 passed. Native Tauri command registration and rejection of external origins
  were exercised with the mock webview runtime and real temporary Git repos.
- Visually inspected the generated before-Fetch screenshot.
- `cargo fmt --all` and `git diff --check`: passed.

## Scope and limitations

Only loaded commit rows can be decorated; absent remote-only objects are not
silently downloaded. Existing compact rows keep two badges and overflow labels
in the tooltip. Remote changes can take up to the polling/cache interval to
appear; offline use still displays local tags. This is not a fresh performance
benchmark or a certification of hosted Windows/Linux CI.

`npm run build:local` passed. The local macOS debug/ad-hoc-signed app was
installed over `~/Desktop/GitNova.app`; installed CFBundleShortVersionString is
1.0.9. Strict/deep code-signature verification and installed executable
`--verify-bundle` passed without opening a workspace. Five bundled attribution
resources were verified; the regenerated inventory still explicitly reports an
incomplete license audit (67 unresolved entries), with no dependency changes.
The build retained existing Vite chunk-size/dynamic-import warnings and skipped
notarization. This is a local build, not a newly signed/notarized release.

No commit, push, tag, release or deployment has been performed.
