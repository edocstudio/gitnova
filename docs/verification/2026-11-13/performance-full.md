# Full performance follow-up — 2026-11-13

This report separates deterministic reductions in work from measured latency.
It is not a cross-product comparison or a latency guarantee. Machine-readable
results accompany this report in [performance-full.json](performance-full.json).
Full samples are separate in [performance-full.raw.json](performance-full.raw.json),
so a website can load the summary first and raw evidence only when needed.

## Provenance and protocol

Baseline: clean HEAD `cccd7561e1b26438bb9dadf2c0fa60b3cb8ea04a`, version
1.0.11. Candidate: the uncommitted changes described below, version 1.0.12.
No user branch, stash, index, files, remotes or credentials were changed to
create measurements. All benchmark writes use disposable synthetic repositories.

Host: macOS 26.7.1 arm64, 10 logical CPUs, 16 GiB RAM; Apple Git 2.39.2,
Rust 1.99.1, Node 26.13.0. One build/test/benchmark runner at a time, one Cargo
build job, two Rust test threads and one Playwright worker. Sequential paired
blocks share the same fixture recipe and executable Git; they are not randomized
trials. Other applications and OS cache/thermal state are uncontrolled. Cold
means a fresh Application/catalog, **not** a cold filesystem cache.

The existing Desktop executable matched the baseline release binary SHA-256
`38c0a7f240c73ddabc03836c5315840041445be3d3a050f1b00be4285ecf88de`.
The installed app was therefore not assumed to be a debug build. `build:local`
remains a development/debug path; `build:release` remains the release-profile
candidate path. There is no architecture replacement or added dependency.

Twenty observations per short scenario; median and nearest-rank p95. All
observations, including outliers, are retained in the machine-readable evidence.
The native diagnostic records queue, execution, Git command counts/duration,
encoded byte counts and a separate serialization probe, without arguments,
source text, credentials or sensitive paths. UI samples stop at matching DOM
content; polling and WebView timer scheduling are included. They do not measure
physical presentation. Serialization probes are **not** timestamps of Tauri's
actual encoding. Subtracting medians from different requests would not establish
IPC or rendering cost, so no such attribution is claimed.

Core read fixture: 2,000 commits, one tracked file, one untracked file and a
20,000-line diff. Related fixture: 121 parent commits, ten child commits, one
submodule, one linked worktree and 1,000 untracked files. Operation fixture:
5,000 history commits and 1,000 tracked files, with a temporary local bare remote.
Native IPC/UI fixture: 2,000 commits and 1,000 untracked files. Local transport
excludes internet, provider authentication and server latency.

## Evidence and changes

- `crates/gitnova-git/src/merge/mod.rs`: reuse one fresh discovery and parsed
  operation marker within `merge_state`. Previously this read rediscovered the
  same repository and loaded the marker repeatedly. No mutable cross-request
  cache was added. Write callers retain fresh `load_marker` discovery and stale
  plan validation. External marker edits are observed on the next request.
- `apps/desktop/src/App.tsx`, `WorkingDetails.tsx`, `CommitDiffDock.tsx`: load
  Settings, stash controls, the file editor and conflict resolver when requested.
  This also removes the static import that previously defeated the stash module's
  dynamic import. Existing surfaces, drafts, translated loading status and
  confirmation flows remain. Initial graph/diff rendering does not await these
  optional modules.
- `DiffView.tsx`: all file summaries remain available, while initially collapsed
  files do not mount code viewports. For 200 files, initial code viewports fall
  from 200 to three. Opening a section mounts its code; subsequent collapse keeps
  its viewport state. This is not a reduction of the file manifest or patch scope.
- `CommitDiffDock.tsx`: memoize hunk indexes instead of filtering all diff lines
  for every visible row on each scroll; do not construct split-line pairs or
  measure their widths while the unified view is selected. Existing virtualization,
  large-file limits and exact hunk validation remain.
- `performance-trace.ts` and `App.tsx`: bounded, opt-in post-write refresh timing
  distinguishes refresh from the operation result. No mutation is cancelled and
  the existing write exclusion remains until a safe state is observed.
- `soak-native.py` and native acceptance startup: permit the isolated release
  fixture to use the existing soak driver. Regular app startup does not inject
  diagnostics. The acceptance catalog guards, resource limits and task-owned
  process cleanup remain. Performance and soak drivers are mutually exclusive.

Existing selection lanes already coalesce pending replaceable reads; this patch
does not claim to have introduced them. An active superseded backend read still
finishes. Foreground/background worker separation, cache bounds, graph
virtualization and progressive first-page refresh also predate this patch.
Two worker slots were retained rather than increasing I/O concurrency.

## Safety and coverage

Repository identity/allowlists, final fingerprints, preview/confirmation, dirty
and untracked checks, child-repository checks and write-time validation remain.
No arbitrary fsmonitor, external diff, textconv, custom update command or preview
hook was enabled. No real provider credentials or user repository were used to
benchmark writes. Cache invalidation and stale-response tests cover external Git
changes and repository/worktree boundaries.

The CI visibility test uses real Git with a controlled provider response: it
checks host/repository isolation, stale responses and pausing while hidden.
The auto-fetch test uses a disposable local remote and simulated clock. These
are behavioral checks, not measurements of live GitHub/GitLab/Jenkins latency.
There is no available comparable measurement of another Git client. Native
Windows/Linux GUI and memory behavior were not measured on this macOS host.

## Paired core measurements

All times below are milliseconds; each cell is median / p95. These are core
execution timings, not end-to-end UI timings. The unchanged control scenarios
also drift between sequential blocks, so these results do not isolate causality.

| Read scenario | Before | After | Median Git calls before → after |
| --- | ---: | ---: | ---: |
| open_registered_cold_app | 28.0 / 45.8 | 42.1 / 51.4 | 2 → 2 |
| graph_first_cold_app | 276.0 / 402.0 | 325.4 / 376.9 | 21 → 21 |
| graph_first_warm | 186.2 / 236.2 | 217.8 / 296.9 | 15 → 15 |
| graph_next | 210.4 / 263.7 | 217.8 / 298.1 | 15 → 15 |
| deep_refresh_first_page | 190.4 / 248.7 | 227.5 / 288.1 | 15 → 15 |
| deep_refresh_all_ten_pages | 2026.9 / 2205.4 | 2238.2 / 2703.8 | 150 → 150 |
| detail | 72.6 / 98.2 | 83.3 / 101.0 | 6 → 6 |
| snapshot_clean | 230.0 / 277.5 | 204.7 / 279.9 | 21 → 17 |
| snapshot_dirty_untracked | 259.3 / 319.5 | 246.9 / 378.1 | 23 → 19 |
| large_file_preview | 263.2 / 292.7 | 274.7 / 361.6 | 21 → 21 |
| stage | 559.2 / 654.0 | 609.0 / 741.6 | 44 → 44 |
| switch_preview | 191.6 / 254.5 | 203.6 / 315.7 | 16 → 16 |
| switch_apply | 474.8 / 620.9 | 517.8 / 654.0 | 36 → 36 |
| snapshot_submodule_1000_untracked | 277.6 / 355.8 | 291.3 / 341.4 | 26 → 22 |
| submodule_discovery | 310.2 / 354.8 | 339.4 / 410.0 | 27 → 27 |
| worktree_list | 20.9 / 27.1 | 25.7 / 35.2 | 2 → 2 |

Snapshot observations remove four redundant Git calls (21 → 17 when clean).
The clean snapshot median improves, but its p95 does not. Graph, large-file and
several control medians regress; no general speedup is established. In the
candidate, summed subprocess duration accounts for roughly 98–99% of median
execution time in warm graph, snapshot and file-preview requests. This points
to subprocess work, rather than Rust computation alone, as a remaining cost.

| Operation phase | Before | After | Median Git calls before → after |
| --- | ---: | ---: | ---: |
| switch_feature_preview | 150.0 / 197.9 | 147.5 / 168.4 | 16 → 16 |
| switch_feature_apply | 398.3 / 611.9 | 388.0 / 451.9 | 36 → 36 |
| switch_refresh | 651.2 / 945.9 | 621.4 / 904.4 | 43.5 → 39.5 |
| switch_main_preview | 217.2 / 343.2 | 223.3 / 285.5 | 16 → 16 |
| switch_main_apply | 587.0 / 763.7 | 662.8 / 794.7 | 36 → 36 |
| push_preview | 459.6 / 540.0 | 486.7 / 554.4 | 35 → 35 |
| push_apply | 1717.3 / 2048.9 | 1937.7 / 2139.7 | 108 → 108 |
| push_refresh | 614.1 / 738.7 | 622.1 / 698.7 | 42 → 38 |
| pull_fetch | 303.0 / 432.2 | 294.5 / 336.8 | 18 → 18 |
| pull_preview | 439.7 / 485.0 | 400.0 / 465.6 | 43 → 43 |
| pull_apply | 1332.0 / 1651.6 | 1276.3 / 1452.4 | 119 → 119 |
| pull_refresh | 473.2 / 542.4 | 436.6 / 549.9 | 42 → 38 |
| merge_preview | 603.9 / 777.1 | 579.2 / 684.2 | 49 → 45 |
| merge_begin | 2472.6 / 3288.8 | 2200.3 / 2599.4 | 151 → 133 |
| merge_finish_preview | 832.3 / 1165.0 | 738.4 / 967.6 | 51 → 46 |
| merge_finish | 2459.4 / 3202.4 | 2382.0 / 2604.3 | 156 → 137 |
| merge_refresh | 719.2 / 867.9 | 659.7 / 865.9 | 42 → 38 |

Switch refresh pools 40 observations (two switches in each of 20 rounds).
Merge begin removes 18 median calls; merge finish removes 19. Push execution
retains its checks and its median regresses. Refresh phases are reported
separately rather than describing operation completion as UI readiness.

## UI work and bundle size

Initial static JavaScript import closure: 1,032,273 → 949,529 bytes (8.0% less);
its sum of per-file gzip sizes: 266,098 → 241,894 bytes (9.1% less). This includes
the shared runtime chunk, not only the entry file. It is a transfer/parse-work
proxy, not a measured startup latency improvement. A >500 kB chunk warning
remains. The dependency graph and lockfile dependency versions are unchanged.

Two real-core browser experiments each run 20 rounds of 12 rapid selections
(commit selection and working-file selection). Each round starts two backend
reads: the already-running superseded read and the final read; ten intermediate
reads never start. Final content matches the final selection. The first read is
artificially gated, so their timing is not a native responsiveness benchmark.
The already-running superseded subprocess is not killed.

## Native release IPC and UI measurements

Same synthetic recipe, 20 samples per scenario in each version. Times are
median / p95 milliseconds. The native app and its WebView perform the request,
not a browser mock. CI/network accounts are absent in these timing trials.

| Scenario | Before | After |
| --- | ---: | ---: |
| graph_first_ipc | 265.5 / 570.0 | 327.0 / 468.0 |
| graph_next_ipc | 208.0 / 433.0 | 232.5 / 357.0 |
| snapshot_ipc | 297.5 / 525.0 | 269.0 / 318.0 |
| file_preview_ipc | 203.0 / 260.0 | 225.5 / 304.0 |
| detail_ipc | 79.0 / 107.0 | 95.0 / 129.0 |
| detail_ui_dom_ready | 120.0 / 163.0 | 123.5 / 184.0 |
| working_diff_ui_dom_ready | 731.5 / 1385.0 | 725.0 / 969.0 |

The snapshot IPC median/p95 improve. Working-diff DOM readiness median is
roughly unchanged, with a lower p95. Several graph/detail/file-preview medians
regress. These data do not establish faster end-to-end startup or scrolling.
Native queue, execution and serialization-probe distributions remain separate
in the JSON evidence. A 21-second baseline working-diff outlier is retained
rather than excluded. Both native trials verify fixture Git state preservation.

## Queue and remaining work

In the short native trial, most measured graph/detail/file-preview requests
wait under a millisecond at p95. Startup catalog/submodule requests can wait
longer, but their sample counts are too small for general conclusions. These
trials do not simulate congested provider networks. Existing worker limits and
foreground reservation are retained; no higher concurrency was introduced.

Pending reads are coalesced; active obsolete reads still consume work. Native
read cancellation, immutable-detail cache expansion, and further observation
sharing remain potential follow-ups requiring process ownership and invalidation
evidence. No write may be interrupted midway to improve a responsiveness metric.

A foreground soak attempt stopped after 6.11 seconds when WebKit reported
`hidden` during manual refresh, after initially reporting `visible`. It completed
zero cycles and is not foreground acceptance or a 30-minute memory result.
The attempted 1,800-second run and failure are retained in the JSON evidence.
A separate background run is measured explicitly rather than relabeled.

## Thirty-minute release background soak

Completed 1809.36 seconds and 144 interaction cycles,
with the fixture described above. WebKit reported **hidden**, not foreground.
Fixture HEAD/branch/staging/refs/working state checks passed after the run; the
reviewed branch roundtrip was restricted to the clean synthetic large-b repo.
No build or other test runner ran during sampling.

- Native RSS peak: 226.39 MiB.
- Sampled native + new WebKit helpers + sampled descendant Git RSS peak:
  490.22 MiB.
- Native median during the last five minutes:
  165.22 MiB.
- Linear warm native slope after minute five:
  0.0046 MiB/minute.
- Linear warm WebKit slope: 3.14 MiB/minute.
- Maximum DOM nodes: 2224; maximum rendered graph rows:
  25. Observed Tauri callbacks at cycle samples: zero;
  formatter count ends at four and canvas context count at one.

Native RSS is stable by this slope estimate, **but WebKit RSS has positive
growth**. This is unresolved follow-up evidence, not proof of a leak or proof
of no leak. New WebKit helper attribution cannot exclude unrelated helpers;
summed RSS can double-count shared pages and miss short-lived Git peaks. No
JS heap API value was available; GPU allocation is not measured. There is no
paired baseline long soak, so this run does not establish lower RAM usage.

Background latency (median / p95 ms):

| Action | Samples | Median / p95 |
| --- | ---: | ---: |
| branch_roundtrip_review_apply_ipc | 13 | 2425.0 / 2890.0 |
| detail_ui | 144 | 106.0 / 216.0 |
| manual_refresh_ui | 144 | 1627.5 / 2660.0 |
| older_graph_ui | 48 | 919.0 / 1185.0 |
| snapshot_ipc | 144 | 422.5 / 774.0 |
| switch_repository_ui | 144 | 544.5 / 1096.0 |
| search_ui_current | 24 | 935.0 / 2206.0 |
| workspace_search_ipc | 24 | 3026.5 / 3358.0 |

Branch roundtrip has only 13 samples and does not meet the short-benchmark
20-sample protocol; the separate paired operation suite does. Background UI
timers/throttling are part of these numbers. Search over the ten-location
fixture remains a bottleneck; its cold outlier is retained.

## Same-candidate debug / release comparison

Fresh Application with the identical 2,000-commit graph-only fixture recipe;
20 samples per scenario/profile. Sequential blocks, no OS cold-cache claim.

| Core scenario | Debug median / p95 ms | Release median / p95 ms |
| --- | ---: | ---: |
| graph_first_cold_app | 305.6 / 354.7 | 323.0 / 428.5 |
| graph_first_warm | 203.7 / 277.8 | 228.0 / 255.9 |
| graph_next | 198.8 / 244.9 | 204.1 / 271.4 |
| graph_refresh | 201.5 / 242.1 | 222.3 / 258.6 |

These timings do not justify attributing the user's reported slowness to a
debug build: the existing Desktop binary already matched the baseline release.
Most core execution is spent waiting for Git subprocesses in these fixtures.

## Verification actually run

- Frontend TypeScript/Vite production build: passed. The bundle-size warning
  remains and is reported above.
- Frontend unit suite: 23 files, 89 tests passed. Includes selection coalescing,
  cache/invalidation helpers and the 200-file deferred-code regression.
- Debug Rust targeted suites: 123 tests passed across hunks, merges, read
  optimization, rebase, submodule lifecycle/update, tracking and worktree
  creation/removal. Tests mutate disposable fixtures only.
- Release Rust read-optimization + merge suites: 32 tests passed, including
  external marker edits, linked worktrees, stale plans/index and custom-driver
  refusal.
- Targeted browser suites: 15 distinct scenarios passed after correcting one
  fake-clock test harness. Initially 11 of 12 passed; lazy Settings loading did
  not settle while the test clock was paused. The test now advances its own
  clock while waiting for Settings, and that scenario plus three related tests
  passed on rerun. No production timeout was loosened.
- Native release desktop binary suite: 11 tests passed, including foreground
  worker reservation, permit lifetime, real Git IPC origin rejection, fixture
  catalog confinement and PTY lifecycle. Version-script regression: one passed.
- Native release bundle built with `build-release.mjs`; embedded UI/resources,
  strict ad-hoc signature and `--verify-bundle` passed. No trusted signing,
  notarization, upload or publication is implied. Existing license evidence
  still reports an incomplete audit (67 unresolved entries); no compliance
  certification is claimed.
- Native baseline and candidate each produced 140 short IPC/UI samples and
  preserved fixture Git state. Core read/operation and debug/release comparisons
  use the protocols and sample counts reported above.
- Local Markdown links in README and this report, JSON parsing, sensitive-path
  pattern scan of evidence, and `git diff --check`: passed. No configured
  repository-wide Markdown/link checker was found.

Reproduction entry points (run serially):

```text
target/release/examples/perf_reads
target/release/examples/perf_operations 1000 5000 20
node apps/desktop/scripts/bench-native.mjs <release-executable> <ui_bridge> <report.json>
python3 apps/desktop/scripts/soak-native.py --profile release --seconds 1800 --output <report.json>
```

Build the executable/examples first; the soak does not compile while measuring.
Foreground mode requires actual continuously visible WebKit; a requested flag
does not substitute for observed visibility.

## Conclusions and limits

The verified work reductions are redundant operation observations, smaller
initial static JS and fewer mounted code viewports. Snapshot IPC and several
merge phases improve in these trials; other measured latencies regress. No
blanket speedup, zero-defect claim or cross-product comparison follows.

Remaining priorities are Git subprocess overhead (including unchanged graph
reads), live network/provider scheduling under contention, native cancellation
for obsolete reads, workspace search and the positive WebKit RSS trend. Any
future sharing/cache must retain repository/worktree/config/refs invalidation
and fresh pre-write checks. Foreground long-run memory and native Windows/Linux
results remain unverified. No remote push or release publication was performed.

## Local delivery

Desktop/GitNova.app was replaced with the verified **release-profile 1.0.12**
bundle after the application was closed. Info.plist version and executable
SHA-256 match the measured candidate:
`e52ade3e071c71329b0e87ab2971fe3a2493cc2c4bc0a4d61f30656d8b440c65`.
Strict/deep signature and embedded UI verification passed again after copying.
Only one GitNova*.app remains on Desktop. No normal app window was launched
during verification or installation.
