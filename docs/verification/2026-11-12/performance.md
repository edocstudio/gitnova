# Performance investigation — 2026-11-12

Machine-readable results: [performance.json](performance.json). This is measured engineering evidence, not a speed or memory guarantee.

## Provenance

Base HEAD: `73a1396fec5f79333b2564dd1b6c7f0f61e2e26c`. The starting working tree already contained the 1.0.9 remote-tag fix, on top of HEAD metadata 1.0.8. That dirty source was preserved and used as the baseline; candidate source is 1.0.10. No commit, push or release publication was performed. The baseline file-list digest is recorded in JSON.

Host: macOS 26.7.1, Apple Silicon, 10 physical/logical CPUs, 16 GiB RAM; Rust 1.99.1 and Node 26.13.0. One build/test/benchmark runner at a time. Debug profiles retain the existing opt-level 2 overrides for core/Git/search/graph/domain; they are not entirely unoptimized. Desktop baseline 1.0.9 executable SHA-256 `b699ae63705f3be76b6ef3dd06495c78ac88c870091bf636a3ba95ef9ce20831` matched the existing debug bundle exactly and did not match the release bundle. This verifies that installation, not other users’ installations.

## Causes and changes

- `src/App.tsx` previously ignored stale detail responses after every selection had already invoked native work. `src/selection-reads.ts` now keeps one active read and only the latest pending selection per fixed lane. Detail, historical patch and working-file/hunk reads opt in via abort signals; effect sequence checks still reject stale responses across selections and repository changes. An active native read still finishes; no subprocess is killed and no write is cancelled. No DTO cache is added.
- Deep ref refresh in `src/App.tsx` waited for the bounded previously loaded range. It now publishes the validated first page immediately, restores the old anchor while untouched, and yields restoration when the user navigates/selects. One in-flight page can still complete. Existing pagination, virtualization, session caps and mutation busy guards remain.
- `src-tauri/src/command_slots.rs` retains two shared workers, but background CI/provider/remote-tag reads acquire a single background gate before a shared worker. Background admission is capped at 8 within the existing total 32. This leaves capacity for foreground local reads without increasing I/O concurrency. The deterministic permit test verifies the reservation; real CI/network latency was not benchmarked.
- `build:desktop` now uses the existing release packaging path on macOS as well as Windows/Linux. `build:local` retains the diagnostic debug workflow. Release profile alone was not faster in the baseline core measurements.
- Opt-in `performance-trace.ts` stores at most 512 timing records, with no arguments or payloads. Fixture-only native diagnostics split queue/native execution and an additional serialization probe; caller IPC totals include transport and encoding. Two animation frames are a readiness proxy, not an OS presentation timestamp. Git instrumentation records bounded command names/counts/durations, never source, credentials, URLs or paths.

Repository identity/allowlist/config safeguards, stale write validation, dirty/untracked/submodule checks, preview/confirmation and index/worktree protections remain unchanged. No fsmonitor hook is enabled. Existing caches and their invalidation are retained.

## Selection before/after

Same Chromium + real Rust core transport, 20 bursts of 12 selections each; first native detail read held until the burst is submitted, then released. This isolates obsolete pending work under a repeatable condition; it is not a claim about all normal click latencies.

| Measure | Before | After |
|---|---:|---:|
| Final detail usable, median / p95 ms | 2189.5 / 2580 | 530.5 / 1494 |
| Started detail reads per burst, median / p95 | 12.0 / 12 | 2.0 / 2 |

## Serial core results

20 samples per scenario; branch preview/apply has 40 (20 switches each way). Core execution includes Git and validation; serialization probe is separate in JSON. A post-benchmark host observation found about 6.6 GiB occupied compressed memory and 4,882 MiB swap in use. This was not sampled continuously and does not establish the cause of the timing difference. Main fixture: 2,000 commits, one tracked file, 20,000-line edit and one untracked file. Candidate-only related fixture: 121 parent commits, 10 child commits, one submodule, one linked worktree and 1,000 untracked files.

**The candidate serial core samples are slower in this run.** Git command counts are unchanged; this work does not establish a core throughput improvement. Do not attribute the difference to CPU profile or UI scheduling without a controlled follow-up. Foreground responsiveness and obsolete-work reduction are separate from core throughput.

| Scenario | Debug before median/p95 ms | Debug after | Release before | Release after |
|---|---:|---:|---:|---:|
| open_registered_cold_app | 31.479/46.963 | 43.771/68.527 | 32.065/44.852 | 45.587/94.596 |
| graph_first_cold_app | 249.862/384.928 | 403.371/591.274 | 266.501/341.015 | 417.704/873.165 |
| graph_first_warm | 188.173/283.963 | 258.556/514.436 | 187.103/238.825 | 269.141/410.296 |
| graph_next | 183.576/234.801 | 266.925/434.01 | 183.965/264.486 | 264.536/450.631 |
| deep_refresh_first_page | 187.479/233.68 | 285.448/466.615 | 191.76/243.34 | 282.736/777.743 |
| deep_refresh_all_ten_pages | 1863.174/2114.405 | 2728.695/3241.496 | 1855.346/2243.539 | 2879.918/5400.31 |
| detail | 59.634/89.486 | 86.336/133.332 | 69.987/106.94 | 106.762/174.563 |
| snapshot_clean | 204.846/289.087 | 280.044/354.644 | 217.043/303.889 | 303.708/523.544 |
| snapshot_dirty_untracked | 225.013/298.678 | 316.055/425.177 | 233.155/316.695 | 372.209/533.971 |
| large_file_preview | 232.389/293.896 | 340.602/394.378 | 221.728/289.5 | 306.846/580.374 |
| stage | 515.362/766.1 | 740.182/888.773 | 513.817/659.835 | 784.473/1340.22 |
| switch_preview | 157.681/194.689 | 242.621/375.721 | 159.571/254.904 | 244.211/524.47 |
| switch_apply | 408.878/556.852 | 579.835/843.884 | 410.983/541.723 | 595.258/1311.394 |
| snapshot_submodule_1000_untracked | not measured | 410.243/483.966 | not measured | 315.256/489.06 |
| submodule_discovery | not measured | 441.56/534.949 | not measured | 352.018/482.656 |
| worktree_list | not measured | 31.073/41.742 | not measured | 24.873/36.406 |

## Installed-format native release probe

After-only packaged macOS release candidate, isolated temporary catalog and 2,000-commit fixture with 1,000 untracked files; no provider accounts. Native Tauri IPC is real. The first native trial timed out waiting for animation frames and is excluded; the final driver ends UI samples at matching DOM content, not physical presentation. The launcher checks that fixture HEAD/index/worktree tracked state was not modified. Visibility: `hidden`. This is a hidden-window IPC/DOM probe, not a foreground rendering or perceived-latency benchmark.

| Scenario | n | Median ms | p95 ms |
|---|---:|---:|---:|
| graph_first_ipc | 20 | 272.0 | 426.0 |
| graph_next_ipc | 20 | 240.5 | 327 |
| snapshot_ipc | 20 | 281.0 | 372 |
| file_preview_ipc | 20 | 214.0 | 408 |
| detail_ipc | 20 | 84.5 | 136 |
| detail_ui_dom_ready | 20 | 123.5 | 202 |
| working_diff_ui_dom_ready | 20 | 643.5 | 1083 |

## Reproduction and retained evidence

Run `cargo run -p gitnova-core --example perf_reads --locked --offline -j 1` and repeat with `--release`. For selection use the `selection performance` Playwright case with `GITNOVA_PERF_SAMPLES=20`; set `GITNOVA_PERF_BASELINE=1` only on the preserved baseline. Use the identical fixture/test protocol, and run each command serially.

For native packaging, regenerate host license evidence then use `npm run build:desktop`. The macOS fixture launcher is `node apps/desktop/scripts/bench-native.mjs <packaged-executable> <ui-bridge> <output.json>`. Quit the real app first. It creates only a temporary synthetic repository and private catalog, launches with explicit acceptance/diagnostic flags and closes only its own process. It does not install or publish the app.

Raw core/UI/native evidence and preserved starting patch are retained locally under ignored `target/performance-2026-11-12/`; the preserved patch is not part of this report or a public artifact. Aggregate results and exact UI samples are in the adjacent JSON.

## Limits and remaining work

- Synthetic 2,000-commit/one-tracked-file fixture, not a large production monorepo.
- Cold means a fresh Application/catalog, not an evicted OS disk cache.
- Chromium selection protocol holds the first read until 12 selections have been submitted; both versions use the same gate and real Rust core. It is not native IPC.
- Unpaired serial core measurements show slower candidate samples; no core throughput speedup is established. Host scheduling/temperature were not controlled.
- Native release measurements are after-only. No paired native baseline, real network/CI-account on/off measurement, physical-paint timing, long RAM soak or competitor benchmark.
- Windows/Linux native builds and GUI behavior were not tested in this macOS run.
- Already-started reads finish; obsolete queued selections are rejected before native invocation. Writes are never cancelled.

A warm graph page still executes 17 Git commands in the simple fixture (including 7 rev-parse calls); native Git spawn/validation dominates the serial measurements. Further reduction needs a request-scoped observation design with identity/config/refs checks preserved and external-change tests. This round deliberately avoids a long-lived mutable status cache. Large-production-repo scrolling and rapid-file selection need additional timed native scenarios; related/submodule/large-diff samples measure core work only. The native fixture measures Working changes readiness with one of its synthetic untracked files. Git mutation completion remains protected until safe refresh, so post-write refresh latency remains a bottleneck.

## Verification

Checks actually run on macOS in this working tree:

- Frontend TypeScript check and Vitest: 22 files, 88 tests passed. Version synchronization script: 1 test passed.
- Browser regression: 7 Playwright cases passed with one worker, covering external-ref refresh/deep anchors, late submodule responses, branch switch, hunk staging, larger UTF-8 editing, rapid selection and progressive refresh.
- Core integration: 37 tests passed across read optimization, tracking, patch operations and related maintenance.
- Native tests: 11 passed, including queue permits, isolated acceptance, PTY lifecycle and real Git operations through Tauri IPC. The final queue-only rerun also passed (1 test); that debug rerun emitted a macOS 11/13 deployment-target linker warning from mixed cached objects. The packaged release explicitly targets macOS 13 and passed bundle verification.
- Clippy for desktop/core/Git all targets with warnings denied passed. Rust formatting and whitespace checks passed.
- License inventory regenerated as packaging evidence; it still reports unresolved reviews and is not a complete compliance audit.

Release 1.0.10 was installed into the existing Desktop app after the probe; its version and executable SHA-256 match the packaged candidate (JSON). This is ad-hoc signed, not notarized. Native release build/probe results are recorded above when present. Hosted Windows/Linux CI, all UI workflows and long-term RAM were not verified in this run.
