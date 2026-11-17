# Focused display-read performance review — 2026-11-15

## Baseline and scope

Started with a clean working tree at `f5deb4358352ac440b5a91d8f977cf70e976c183`, version 1.0.14.
Both measured binaries use that version and release profile; the candidate includes only the display-read patch. Version 1.0.15 is the delivery metadata bump after measurement. No commit, push or release publication was performed.

Host: macOS 26.7.1 (25G241), arm64, 10 logical CPUs, 16 GiB RAM; Rust 1.99.1, Node 26.13.0, Apple Git 2.39.2. One runner at a time; Cargo compilation limited to one job and Rust integration suites to two test threads.

## Evidence and change

- `crates/gitnova-git/src/lib.rs`: `graph_read_keys` previously called full `discover`, which also obtains the worktree root and object format even though this caller needs only Git/common directories. Normal worktrees paid for a separate `--show-toplevel` subprocess. The new request-local directory resolver queries only the needed fields in one bounded read. It canonicalizes native paths and retains the original full discovery fallback when newline-separated metadata is ambiguous.
- `crates/gitnova-git/src/tracking.rs`: `tracking_display_key` now obtains the canonical Git directory through the existing single-field path parser. It no longer discovers unused root/common/object-format fields. Snapshot latency was not separately measured in this focused run.
- Cache keys still include Git-directory identity, refs/HEAD fingerprint and config bytes. Linked worktrees retain distinct Git directories. Graph pages still perform the final fresh fingerprint check. Full discovery, allowlist checks, reviewed writes, dirty/submodule checks and Git execution restrictions remain unchanged. No new persistent cache, concurrency increase, fsmonitor or dependency was introduced.

## Release measurements

Disposable fixture: 2,000 linear commits, one tracked file; 100-row graph pages. Twenty samples per scenario per run. Cold means a fresh Application/catalog, **not** cold OS caches. The primary pair below ran after the regular desktop app was quit. Values are **median / p95 milliseconds**, nearest-rank p95.

| Scenario | Before | After | Git subprocesses/page |
| --- | ---: | ---: | ---: |
| graph_first_cold_app | 219.09 / 264.07 | 225.38 / 256.86 | 21 → 20 |
| graph_first_warm | 151.58 / 210.46 | 149.09 / 172.71 | 15 → 14 |
| graph_next | 151.61 / 176.04 | 147.69 / 167.68 | 15 → 14 |
| graph_refresh | 170.19 / 220.02 | 157.83 / 180.98 | 15 → 14 |

The reliable result is one fewer Git subprocess per normal graph page: 15 → 14 warm, 21 → 20 fresh-cache. The primary pair has modest warm/next/refresh reductions and no cold-median improvement. This is **not proof of an overall latency improvement**: exploratory candidate timings were slower in several scenarios, and an unchanged baseline control also slowed. All six runs and 480 samples, including unfavorable results, are retained in [performance.json](performance.json). No cross-product claim is made.

Reproduction from the repository root:

```sh
CARGO_BUILD_JOBS=1 cargo run --release -p gitnova-core --example perf_reads -- --graph-only
```

The baseline executable was saved before editing, then compared with the candidate executable. Main-pair raw results are `runs.before-app-closed` and `runs.after-app-closed`; exploratory runs have the normal app open and should not be pooled with that pair. Tests/compilation did not overlap benchmark runners. Other system workloads were not controlled.

## Existing mechanisms reviewed and remaining opportunities

- `apps/desktop/src/History.tsx`, `graph-state.ts`: graph windowing and bounded session rows already exist; adding another pagination layer without a measured problem would duplicate work.
- `App.tsx`: a changed deep graph refresh publishes its validated first page before rebuilding the previous anchor. Further pages still execute separately and repeat metadata checks. A future optimization must preserve lane/cursor correctness under external ref changes.
- `selection-reads.ts`: pending read selections are coalesced. An already-started superseded request can still finish in the backend. Measure wasted active work before adding cooperative cancellation; never cancel writes mid-operation.
- `src-tauri/src/command_slots.rs`: background admission already preserves one of the two workers for foreground work. Network waits can still occupy their own worker; increasing concurrency alone is not justified.
- `DiffView.tsx` and `CommitDiffDock.tsx`: lazy/virtualized file views and deferred parsing already exist. Large diff parsing/highlighting and editor interaction need dedicated UI traces before another change.
- `crates/gitnova-core/src/display_cache.rs`: bounded weighted tracking cache (4 MiB, TTL/idle expiry) is already present. Mutable status must continue to refresh rather than reusing cached write evidence.

This review does not re-certify whole-app performance. No new long-duration RAM, real-network CI on/off, Windows/Linux GUI, full native IPC latency or end-to-end rendering measurement was run. Those remain separate follow-ups; the core microbenchmark does not substitute for them.

## Verification

- `cargo test -p gitnova-core --test read_optimization --test tracking --test real_git -- --test-threads=2`: **28 passed**. Includes external refs/config changes, dirty status, shallow cursor invalidation, linked-worktree/repository isolation, detached HEAD, unborn/bare and newline-native paths; command-count assertions cover the reduced probes.
- `cargo clippy -p gitnova-git --all-targets -- -D warnings`: passed.
- `node --test apps/desktop/scripts/set-version.test.mjs`: **1 passed**.
- `npm run build:release`: passed (TypeScript/Vite and native optimized release bundle). macOS ad-hoc signature and embedded resources verified; no notarization or publication.
- Installed the release bundle over the existing `Desktop/GitNova.app` only after the user quit it. Verified 1.0.15 in the installed plist, byte-identical executable, strict deep signature and `--verify-bundle`. No GUI/user repository was opened by these checks.
- Relative file links in README/report and `git diff --check`: passed. No new UI automation or Windows/Linux runtime test was run for this backend-only change.

No source code, credentials or user repository paths are included in benchmark metrics. User Git repositories were not mutated for verification.
