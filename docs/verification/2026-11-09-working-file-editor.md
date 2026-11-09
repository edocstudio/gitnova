# Working file editor size regression — 2026-11-09

The reported UTF-8 configuration file was approximately 544 KiB. The current-file
editor refused it because its independent 512 KiB safety limit was lower than
ordinary configuration-file sizes. This was a size restriction, not evidence
that the file was corrupt or binary. No project file was edited to reproduce it.

The existing diff-dock editor now accepts tracked, writable UTF-8 text up to
4 MiB. The native response includes `maxBytes`; the frontend uses that value for
byte validation instead of a separate hardcoded input limit. Large text layout
and full-document selection also caused a browser timeout during the first
regression run. The editor now renders bounded sections, with Previous/Next
controls in the existing dock; saved output still includes every section.
Sections initially contain at most 32,768 UTF-16 code units and edits are capped
at 65,536 per section. Oversized section paste is rejected without truncation. UTF-8 size checking does not allocate a
second encoded buffer on every keystroke. Up to four drafts remain bounded.
No new screen or panel is introduced. Section controls are necessary because
full-document rendering/selection can stall the existing text editor; the
existing diff controls cannot provide bounded text editing.

Read and save both enforce the raw byte limit, including CRLF expansion on save.
Non-UTF-8/binary data, mixed line endings, symlinks, nested repositories and
unresolved conflicts retain their existing restrictions. Save keeps HEAD and
staging unchanged, preserves executable permissions and original line endings,
retains private original-byte recovery, and rejects stale file/branch/index
snapshots. The change does not raise diff, merge, stash or other write budgets.

Validation (macOS Apple Silicon, sequential runners):

- `cargo test -p gitnova-git --test working_file_edit --locked -- --test-threads=2`:
  7 passed. Includes large multilingual UTF-8/CRLF save, recovery, stale preview,
  the exact 4 MiB read boundary, rejected oversized save and CRLF expansion.
- `npm run build`: passed (TypeScript and Vite).
- `npm test`: 18 files, 72 tests passed, including UTF-8 boundary and i18n checks.
- `cargo clippy -p gitnova-git -p gitnova-core --all-targets --locked -- -D warnings`:
  passed.
- Playwright: both historical-file/draft/stale-save and large multilingual
  file section-edit/save regressions passed with the real core and disposable
  Git repositories (2 passed, 35.7 seconds). The first unrestricted full-document
  fill timed out; the final section-based flow passed without extending timeouts.
- `npm run build:local`: passed with one Cargo job. The macOS debug/ad-hoc-signed
  1.0.0 bundle was installed over the existing Desktop `GitNova.app`. Strict code
  signature verification and `--verify-bundle` passed; installed and built binary
  SHA-256 values matched. No real workspace was opened during verification.
  This is not a notarized release. The bundle attribution check reported five
  resources present and 67 unresolved inventory entries; it is not a complete
  dependency-license audit.
- No new long-session RAM benchmark or native platform performance certification
  was performed for this fix.
Windows/Linux native acceptance is not implied by local macOS results.
