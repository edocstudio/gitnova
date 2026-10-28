# Milestone 26 — Visible discard progress and fewer Git launches

Discard could leave the review disabled with only Discarding… while the worker repeatedly launched Git. A temporary owned-merge fixture with 24 small untracked files reproduced 1,121 Git launches in apply. This implementation overhead, rather than the choice of Rust alone, dominated the measured case.

The existing review now shows an animated waiting indicator, named phases and native-measured file/byte counters. Percentages describe the current measured step. Preparing/finalizing/refreshing remain indeterminate; no timer invents an overall percentage. The progress area stays visible in the scrolling operation composer. Existing Git-write and graph-refresh status also gains a spinner, respecting reduced-motion preferences. EN/TH labels preserve repository data and selected scope.

A scoped native-independent observer emits counters from the actual Git worker. The native adapter receives an optional JavaScriptChannelId and creates a typed Tauri Channel on the invoking webview. Delivery is throttled to about 100 ms plus phase boundaries/completion. UI failures or cancelled previews do not change Git outcomes. Request-scoped callbacks ignore obsolete preview messages; no background progress polling or disk journal writes are added. The test bridge forwards real worker events to the real frontend Channel class.

## Performance changes

Batch selected index flags/entries and HEAD membership. Exclusively untracked leaves use native locks, per-file byte checks and direct filesystem guards instead of per-file Git subprocesses. Full state/config checks still surround the loop; tracked restores retain per-file Git validation because checkout filters may run. Recovery still completes before deleting any selected file. Metadata-only Git commands avoid irrelevant filter-driver discovery; status/working diffs retain filter neutralization. Repository discovery combines metadata queries, with the existing raw-path fallback for newline-containing paths. No long-lived repository-identity cache is introduced.

## Measured local fixture

| Case | Preview | Git launches | Apply | Git launches |
| --- | ---: | ---: | ---: | ---: |
| Before, 24 files | 5.404 s | 98 | 67.418 s | 1,121 |
| After, 24 files | 2.454 s | 36 | 13.233 s | 196 |
| After, 136 files | 2.131 s | 36 | 13.098 s | 196 |

These are single sequential instrumented macOS runs with small newly untracked text files after reviewed unstage, using the debug core and a PATH wrapper to count Git launches. The wrapper adds overhead, background host load varied, and frontend refresh is excluded. They demonstrate fewer launches and improved scaling in this case, not a guarantee for large media, tracked restore, corporate repositories or comparisons with other products. The fixed CLI/validation overhead is still visible. No developer repository was modified. Raw results: verification/2026-10-27-discard-benchmark.json. Reproduce on macOS after building ui_bridge with one job: python3 apps/desktop/scripts/bench-discard.py 24 current.

## Validation

Four operation-discard integrations, fourteen ordinary-discard integrations and sixteen repository/identity integrations passed, including a mid-operation metadata change that stops the optimized loop, raw recovery, stale content/config, hooks/filters, symlink safety, bare repositories, linked worktrees, submodules, SHA-256 and newline paths. A byte-copy unit check verifies exact progress counters and preserved content. Twenty frontend unit/catalog tests passed. Three focused browser workflows passed with real-core progress forwarding, followed by an explicit visible percentage/progressbar check. Native command dispatch passed with a channel argument and remote-origin refusal. Final workspace formatting and focused strict core Clippy passed. GitNova.app built successfully (57.48 MiB); signature and embedded-asset self-checks passed. The user’s existing GitNova Updated.app process was left running. No native GUI write automation was run against that live session.

Screenshot: [Measured step progress in Thai](screenshots/discard-progress-thai.png).
