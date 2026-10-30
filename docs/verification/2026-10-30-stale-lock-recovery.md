# Stale operation lock recovery — 2026-10-30

Scope: [M79](../milestone-79.md). Inspection and explicit archival share the existing
activity/recovery surface in both ordinary and pending Working changes. Tests use
only disposable repositories, no real account credentials or network remotes.

## Local checks

- `cargo test -p gitnova-core --test stale_lock --locked -- --test-threads=1`:
  **5 passed** on macOS. Covers dead/live owners, malformed/symlink files, native
  markers/locks, changed inode/index, linked worktree scope and an orphan Git helper.
  A successful recovery preserves dirty working content, exact index bytes, refs
  and retained journal bytes. A second attempt is refused.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
  Existing cached build-script macOS deployment-target linker advisory remains;
  this is not certification for older macOS versions.
- TypeScript check and UI unit tests: passed, **53 tests / 13 files**.

- Browser tests with the real Rust core and temporary Git: **2 passed** in 48.2
  seconds, one worker. Covers reviewed archival/live-owner refusal plus the
  existing graph-owned conflict Rebase Continue flow. Native WebKit interaction
  is not implied by the browser transport.

`npm run build:local` passed, including TypeScript/Vite build, strict deep
codesign and embedded UI verification. The same bundle replaced the existing
`~/Desktop/GitNova.app` after checking again that GitNova was closed. Installed
codesign, executable SHA-256 equality and `--verify-bundle` from `/tmp` with a
minimal system PATH passed. No native workspace/window was opened. The artifact
remains a local ad-hoc-signed debug build, not a notarized public release.

Relative Markdown links, EN/TH key parity and whitespace checks passed. No
commit or push was performed; earlier uncommitted M78 work was preserved.

## Limits

Only macOS process verification is implemented. Native Git lock deletion is never
allowed by this workflow. An uncertain owner, unrelated running Git process,
remaining native operation, unsafe metadata or inspection budget blocks recovery.
The original lock is retained in the Git common directory. Journals are neither
removed nor resumed, so an interrupted GitNova session may still require its own
existing review/recovery flow afterward. No automatic retry, reset or rollback is
performed. Linux/Windows runtime and full native WebKit acceptance were not tested.
