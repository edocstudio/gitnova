# Large amend verification — 2026-10-29

## Defect and change

A complete 107-file staged selection was blocked because its combined HEAD-to-index patch exceeded the 512 KiB preview budget. Native amend marked this partial code preview as a blocker and did not advertise its complete staged manifest; the composer also excluded amendments from per-file loading.

The complete staged manifest now drives amendment review. Individual selected-file deltas reuse the existing token-validated preview endpoint and bottom dock. Aggregate and individual code budgets remain bounded. All listed staged content enters the exact reviewed replacement tree; selecting a file only changes displayed code.

## Verification

All writes used disposable temporary repositories; no developer project was staged, amended, pushed or reset.

- Native amend integration suite: **13 passed**, including two new large-amend regressions.
- Large fixture: 107 newly staged files, combined patch >512 KiB, plus a separate unstaged file. The last file outside the aggregate patch loads correctly; amendment includes all 107 changes and preserves the original file, author/date, root parent shape, recovery ref and private unstaged edits.
- Changed index after large review: both selected-file preview and amendment refuse the stale token; HEAD and the newly staged content remain untouched.
- Existing coverage: message-only/root/merge amendments, referenced-history acknowledgment, hooks, recovery-ref protection, linked worktrees and unsupported contexts.
- Frontend: TypeScript check; **45 unit tests passed** across 11 files.
- Real-core browser flows: **4 passed** (107-file amend, message-only amend, referenced-history amend, first publication of 383 files). Browser transport is emulated; Git results and writes use real Rust and temporary repositories. This does not certify native webview transport.
- Focused Git/core all-target Clippy with warnings denied, Rust formatting and diff whitespace checks passed.

Logs: `/tmp/gitnova-m59-amend.log`, `/tmp/gitnova-m59-ui.log`, `/tmp/gitnova-m59-clippy.log` (machine-local, not public artifacts).
