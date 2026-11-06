# Header manual Fetch

Added a shortcut to the existing Fetch flow immediately before Refresh in the
header. It uses the configured/preferred remote or existing remote picker, shares
the foreground/automatic-fetch busy gates, and shows a spinning icon while fetching.
Existing English/Thai translations are reused. Refresh remains a local reread.

Validation on 2026-11-06:

- TypeScript check passed; frontend unit suite: 68 passed in 17 files.
- Targeted Playwright suite: 2 passed, one worker, real Rust core and disposable
  local repositories. Manual fetch updated graph/Pull count, preserved selected
  commit, HEAD, staging and working edits, disabled repeated clicks and Refresh,
  and displayed the loading indicator. Existing one-minute Auto fetch, visibility
  pausing and disabling behavior also passed.
- No live provider credentials or developer-repository writes were used.

Browser transport is emulated; this is not a full cross-platform native acceptance
run. No dependencies, native Git implementation or Auto fetch schedule changed.

`npm run build:local` passed, including strict bundle signature and embedded-asset
verification. This is the existing debug/ad-hoc macOS alpha without notarization;
existing shared-chunk and incomplete license-inventory warnings remain. Installation
initially stopped because the app reopened after building. After the user closed
it, the canonical Desktop GitNova.app was updated. Strict/deep signature and
installed `--verify-bundle` checks passed. The installed executable matches the
built artifact (SHA-256
`6e532bc8c41eb0ef55c91a1abd8fa805805e38d8349d2ac1f2d9b482a7bdba8d`).
No running app was closed or overwritten.
