# M57 verification — optimization and sustained resource use

## Scope and build

Measurements use synthetic **real Git repositories** in private temporary
directories. No developer project was switched, staged, committed, pushed or
otherwise used as a write fixture. Tests and measurements ran serially with
one Cargo build/test worker. Baseline means the retained pre-M57 debug benchmark;
after means M57 code plus opt-level 2 for first-party core/Git/search/graph/domain
crates. This comparison includes both code and build-profile effects.

The canonical debug `GitNova.app` was rebuilt in place, verified against its
embedded UI, and is 70.75 MiB on disk (previously 83.57 MiB). Disk size is not RAM.
This local build is not a signed/notarized public release.

Host: Apple M1 Pro (10 physical cores), 16 GiB RAM, arm64, macOS 26.7.1, Apple Git 2.39.2 (Apple Git-143).

## Before/after native operations

Three serial interleaved baseline/after runs, 500 tracked files, local bare
transport. Totals include preview, confirmed apply and one graph/snapshot/stash
refresh. Pull includes fetch; merge includes begin and finish reviews. Human
review time, provider authentication and network latency are excluded.

| Flow | Before median | After median | Change | Git calls before → after |
| --- | ---: | ---: | ---: | ---: |
| Switch to feature branch | 954 ms | 834 ms | −12.6% | 107 → 92 |
| Push | 2,460 ms | 2,539 ms | +3.2% | 190 → 181 |
| Pull | 2,434 ms | 2,112 ms | −13.2% | 255 → 220 |
| Merge including finish | 6,710 ms | 6,172 ms | −8.0% | 490 → 439 |

Push has **no demonstrated latency improvement** in these samples despite fewer
Git calls. The small sample and overlapping run times do not establish a
significant regression or improvement. Do not extrapolate these local timings
to corporate remotes or compare them with another application.

## Search and larger operation fixture

The read fixture has 20,000 commits plus a second 100-commit repository.
Ten warm queries have a median of **232 → 111 ms (−52.1%)**. Initial graph load
was 569 → 281 ms in one sample. Cold index creation remained **12.67 → 12.84 s**:
it is still slow and is not fixed by the warm-cache/ranking improvement.

A separate final-build fixture with **50,000 history commits and 10,000 tracked
files** completed switch, push, pull and merge with native result assertions:
1,793 / 3,712 / 3,178 / 6,780 ms respectively, using the same total definitions.
These are single absolute samples, not before/after comparisons. An initial
large fixture attempt encountered receive-triggered background Git repacking
during local clone; the benchmark now disables automatic GC in its own bare
and writer fixtures and caps pack threads. No product recovery behavior changed.

Raw timings and command counts:
[optimization-timings.json](2026-10-28-optimization-timings.json).

## Sustained native/WebKit run

Completed **1,809.56 seconds (30 minutes), 117 cycles, 874 RSS samples**.
Fixture: 50,000 seed history commits, 10,000 small tracked text files,
10 repository/worktree locations, two linked worktrees, two initialized
submodules, 200 tracking branches, ten merge commits and a thousand-file change
including a binary file. The current search coverage limit remains 20,000
commits per worktree; this is not full indexing of all 50,000 commits.

The actual packaged native app and real background WKWebView exercised repo
switching, explicit Refresh, historical/working diffs, older graph pages,
workspace search, UI search and ten reviewed branch round trips on the clean
fixture. Final assertions verified unchanged HEADs, branches, refs, semantic
staging and working content across all locations; read-only fixture index bytes
were unchanged. The branch-writing fixture may refresh its own index stat cache.
No action failed and neither resource guard fired.

| Interval | Native median MiB | WebKit median MiB | Sampled sum median MiB |
| --- | ---: | ---: | ---: |
| 0–5 min | 175.5 | 319.2 | 494.8 |
| 5–10 min | 185.5 | 394.2 | 581.8 |
| 10–15 min | 183.5 | 348.0 | 530.9 |
| 15–20 min | 176.9 | 382.0 | 566.7 |
| 20–25 min | 180.0 | 377.3 | 556.6 |
| 25–30 min | 180.8 | 395.3 | 577.8 |

- Native peak: **211.7 MiB**; final five-minute native median: **180.8 MiB**.
- Sampled native + newly observed WebKit + Git peak: **650.1 MiB**.
- After minute five, fitted native slope: **−0.39 MiB/min**;
  WebKit slope: **+0.17 MiB/min**. Both fluctuate and show memory reclamation;
  this run did not show runaway monotonic growth, but does not prove leak absence.
- Rendered commit rows: maximum **25**. Maximum sampled DOM nodes: **10,104**,
  including the thousand-file change list; row virtualization does not imply
  every other UI list is virtualized.

Background latencies (median / p95): repo switch **835 / 2,185 ms**,
commit detail **205 / 530 ms**, manual Refresh **1,967 / 3,177 ms**.
Workspace search across all ten locations remains slow: **12,207 / 22,440 ms**.
This latter workload includes expired traversal caches and reloading metadata
across more repositories than the two retained indexes; it is not equivalent to
an immediately repeated two-repository warm query. It remains an optimization
priority. The test does not establish the exact contribution of each cost.

The console was locked overnight: document visibility was `hidden` throughout.
The driver used the existing manual Refresh action rather than spoofing visibility
or disabling foreground polling guards. These results cover native IPC and a real
background WebView, **not foreground rendering, frame rate, full-workday use,
GPU allocations or JS heap size**. CI/provider polling and real network transports
were not enabled. A temporary task-owned idle-sleep assertion ended with the
probe; no display/lock settings changed.

Native RSS uses the exact process PID. WebKit includes newly appearing XPC
helpers, so unrelated new helpers can contaminate that attribution. Summed RSS
can double-count shared pages, and two-second sampling can miss transient Git
peaks. It must not be presented as exact whole-app physical memory.

Artifacts: [raw soak data](2026-10-28-native-soak.json),
[memory samples CSV](2026-10-28-native-soak-memory.csv).
The run started on October 28 and finished just after midnight on October 29.


## Correctness checks

- Full Rust workspace: **329 tests passed**, serialized; no failed suites.
- Frontend: **45 tests passed**; TypeScript check passed.
- Three browser flows passed: graph/diff/workspace navigation, hunk Stage/Unstage,
  and Rebase conflict/Continue.
- Strict workspace/all-target Clippy passed with warnings denied.
- New checks cover one status scan for count/list, unchanged index on reads,
  external ref/config/dirty changes after a warmed display cache, oversized cache
  admission refusal, equal-observation identity, and exact search rank windows.

## Remaining performance work

Cold search indexing/JSON persistence, foreground full-workday profiling,
large real corporate transports/hooks/filters, incremental SQLite FTS5,
obsolete-read cancellation and filesystem-driven invalidation remain open.
Existing cache limits are estimated retained-value budgets, not whole-process
RSS guarantees. This bounded experiment cannot prove the absence of memory leaks.


## Reproduce without concurrent load

```sh
CARGO_BUILD_JOBS=1 cargo build -p gitnova-core --examples -j1 --offline
target/debug/examples/perf_operations 500
target/debug/examples/perf_operations 10000 50000
target/debug/examples/perf_audit 20000
# Close the normal app; build the single canonical bundle first, then run alone.
python3 apps/desktop/scripts/soak-native.py --seconds 1800 \
  --commits 50000 --files 10000 --small-commits 5000 \
  --output /tmp/gitnova-soak.json
```

The soak runner refuses an already running GitNova, creates an isolated catalog,
and terminates only its own probe. Inspect output before repeating. Baseline
comparison requires retaining a pre-change executable before rebuilding; current
commands alone do not recreate the historical baseline.
