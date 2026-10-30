# M86 verification: search and UI loading

Base checkout: `392ad20` (`main`) with this increment uncommitted. Existing user work was preserved; no commit, push, PR or release was performed. All Git write fixtures are disposable. Runners were serialized with one Cargo job/test thread or one Playwright worker.

## Controlled core slice

`cargo run -p gitnova-core --example perf_audit -j 1 -- 2000 10`

The opt-in benchmark now accepts an optional 2–10 repository count; its two-repository default is preserved. Each run creates independent synthetic histories/catalogs in a temporary directory. Warm queries verify identical totals and first-hit identity. Before/after use ten repositories with 2,000 commits each and ten warm queries. Fixed run ordering, filesystem cache and host load are confounders; no multi-host statistical conclusion is implied.

| Measurement | Before | After |
| --- | ---: | ---: |
| Warm workspace search median | 1,121.40 ms | 331.96 ms |
| Warm workspace search maximum | 1,192.24 ms | 351.36 ms |
| Sampled core RSS maximum | 27.75 MiB | 44.03 MiB |

RSS is sampled after operations, not a high-water measurement. These runs exclude WebKit, authentication, transport, GPU allocations and large working patches. The 64 MiB resident estimate excludes active workers, allocator overhead and query temporaries. [Machine-readable raw results](../benchmarks/workspace-search-m86.json).

## Correctness checks

- `cargo test -p gitnova-search -j 1 --lib -- --test-threads=1`: **7 passed**, including resident count/weight pressure while borrowed, oversized-entry refusal, ten-index reuse, independent cache directories, live refs and bounded ranked pages.
- `cargo test -p gitnova-core --test search -j 1 -- --test-threads=1`: **13 passed**, including all scopes, exact pagination/stale cursors, worktree identity, corruption rebuild, unavailable repositories, raw paths/Unicode and read-only navigation.
- `cargo clippy -p gitnova-search -p gitnova-core --all-targets -j 1 -- -D warnings`: passed.
- TypeScript checking and `npm test -- --maxWorkers=1`: **55 unit tests passed**. Final local build also runs TypeScript checking.
- Eight-flow Playwright selection after the working/operation-list changes: **8 passed**. Covers compact composer/draft preservation, Merge conflict resolution, Cherry-pick block selection, Rebase conflict continue, Stage all/publication of all 383 files with bounded mounted rows and Home/End navigation, large Amend of 107 files, deferred Settings and EN/TH FAQ query preservation.
- Two additional real-core browser search cases passed: workspace scopes/navigation and unavailable-repository error context.

The compact workflow initially exposed an offscreen commit action after author details consumed the available height. The composer was corrected and the original viewport assertion passed unchanged; its test was not relaxed. Earlier ordinary Stage/Unstage, submodule pointer and hunk browser cases also passed during the serial regression selection.

## Native artifact and sustained measurement

Local `npm run build:local` passed embedded asset, ad-hoc signature and five bundled attribution-resource checks. It remains a debug, non-notarized local macOS artifact. The existing bundle chunk-size advisory and incomplete third-party audit remain explicit limitations.

Six further browser flows passed after extending the shared viewport renderer to historical diffs: general graph/navigation, synchronized split diff scrolling, compact working composer, 256-file historical comparison with exact late-file loading and Home/End/Arrow navigation, full 383-file publication, and Dark/Light draft preservation.

### Paired sustained native runs

Both runs used the same fixture construction (50,000 large-history commits, 10,000 tracked files, 10 locations, two linked worktrees, two submodules, 200 tracking branches and ten merge commits), with separate isolated states and native/WebKit processes. Both completed 900 requested seconds and verified preserved fixture HEAD/branch/index/ref/working state afterward. The only intentional soak-time Git writes are reviewed branch round trips in the disposable clean fixture.

The first M86 run already had the 32 MiB search cache and deferred Settings/working lists, but still rendered every historical diff file. The candidate uses a 64 MiB resident estimate and virtual historical file rows as well. These are sequential compound-change runs, not isolated per-change causal measurements. Their fixed-duration drivers completed different cycle counts.

| Measurement | Initial M86 tuning | Final candidate |
| --- | ---: | ---: |
| Measured duration (s) | 910.04 | 904.97 |
| Completed UI cycles | 62.00 | 70.00 |
| Workspace search median, including first query (ms) | 13,916.50 | 3,094.00 |
| First workspace query (ms) | 32,277.00 | 30,085.00 |
| Warm workspace search median, first excluded (ms) | 13,681.00 | 3,083.00 |
| Current-repository UI search median (ms) | 1,725.50 | 850.00 |
| Repository switch UI median (ms) | 451.50 | 484.00 |
| Maximum observed DOM nodes | 10,120.00 | 2,214.00 |
| Peak sampled native RSS (MiB) | 256.58 | 237.83 |
| Peak sampled combined RSS (MiB) | 629.89 | 616.53 |
| Last-five-minute native RSS median (MiB) | 216.98 | 184.17 |
| Post-warm-up native RSS slope (MiB/min) | 0.57 | -3.72 |
| Post-warm-up WebKit RSS slope (MiB/min) | 1.38 | -1.82 |

The graph remained virtualized at a maximum of 25 mounted rows in both native runs. First indexing remains expensive; warm latency does not describe opening an unindexed large workspace. Repository switching was not faster in this sample and no general Pull/Push/network speed claim follows. The initial two-index-to-64-MiB debug-core comparison also traded higher sampled core RSS for latency; it must not be presented as an across-the-board RAM reduction.

The final native run had decreasing fitted native/WebKit RSS after five minutes, including a substantial late WebKit reclamation. This is encouraging evidence for this workload, not proof of leak freedom: no foreground full-workday, JS heap or GPU retention analysis was performed. WebKit helper attribution is inferred from processes absent before launch, may include unrelated new helpers, and summed RSS can double-count shared pages. Short-lived Git/native peaks can fall between samples.

Raw data: [initial native run](../benchmarks/native-soak-m86-before-tuning.json), [candidate native run](../benchmarks/native-soak-m86.json), [initial RSS CSV](../benchmarks/native-soak-m86-before-tuning-memory.csv), [candidate RSS CSV](../benchmarks/native-soak-m86-memory.csv). The [website-consumable summary](../benchmarks/performance-m86.json) includes scopes, limitations and artifact hashes. No website files or deployment were changed.

Remaining performance work includes first-index latency/incremental persistence, obsolete read cancellation, other large review/checklist surfaces, and foreground/full-workday memory and live network measurements.

### Installed local candidate

The existing `Desktop/GitNova.app` was updated only after all native processes had exited. Its executable matches the measured candidate SHA-256 `5e20b5d6311cef8ea3d02ab9d03bde78e6261945804666630d3e2d039baa9f29`. Deep/strict ad-hoc signature verification, all five attribution resources, and embedded UI verification from `/tmp` with only the system PATH passed. No alternate Updated app, signed/notarized release, commit or push was created.

Final document checks found no missing links among 129 relative links in changed Markdown, and no forbidden competitor/company-domain text in 566 tracked/nonignored files. The text scan excludes Git history, ignored artifacts and visual/OCR content. `git diff --check` passed; five exported artifact hashes matched.
