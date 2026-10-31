# Conflict drafts and WebView allocation follow-up

Baseline: `4eb50cf` plus this uncommitted change. Scope is defect regression
and sustained native performance, not a whole-app optimization certificate.
All write tests and measurements use newly created disposable repositories.

## Confirmed draft defect

The existing external-conflict-change test covered only the editable view.
Extending it to return to the diff reproduced a failure: the expected unsaved
resolution was replaced in the UI by the externally edited working file.
Git files/index/HEAD were protected, but the saved draft became inaccessible.

The operation-scoped draft now survives a fresh file/token observation. A notice
in the existing conflict inspector identifies its changed baseline in EN/TH.
Review still rereads the current native file, refuses stale authority and shows
the proposed delta against the refreshed file before Apply can stage it. No
automatic merge, overwrite, staging or persistence to a real user repository is
introduced. Drafts remain in memory for the mounted operation editor; this is
not a promise of draft persistence after quitting the app.

The extended real-Git browser regression proves the original refusal, preserved
draft after returning to the diff, successful fresh review and unchanged disk,
index and HEAD before Apply. Related conflict/rebase, submodule and worktree
recovery workflows are checked separately.

## Bounded diff measurements

Each newly mounted patch previously allocated another canvas context and a
per-file text-width map. Diff widths now use one lazily created 1×1 measuring
surface and a shared LRU with a 512 KiB **estimated text/entry** budget. Exact
expanded-tab text is the key; oversized lines are measured but not retained.
The fixed font and full-file horizontal widths remain unchanged. This bounds
this cache's estimates, not total JavaScript heap or WebKit memory. It does not
cache Git write authority or replace live operation validation.

## Diagnostic protocol

The isolated debug-only native driver records callback-map size, newly created
canvas contexts and measureText calls using weak context references. Optional
`performance.memory` values are recorded only if the WebView exposes them;
absence is not a zero heap measurement. No forced GC or private WebKit API is
used. These counters distinguish some retained application resources but cannot
by themselves rule out JavaScript leaks, engine caches or GPU allocations.

The runner uses a private HOME/config/data/cache alongside its private catalog.
Foreground visibility, sampled RSS/free-memory guards and before/after fixture
HEAD/branch/refs/staging/working-state checks remain enforced. No parallel build
or test runner runs during sampling. The preceding 15-minute foreground result
is historical evidence with a different source/binary, not a controlled causal
comparison proving this cache fixes all WebKit growth.

## Regression checks

- TypeScript/Vite build passed; the existing large-main-chunk advisory remains.
- Frontend unit suite: 57 passed, including exact-text cache reuse, LRU eviction,
  oversized-line exclusion and i18n consistency.
- Ten targeted real-Git browser workflows passed: conflict blocks/direct drafts,
  revert, changed external conflict file, graph-owned rebase, selected submodule
  URL sync/newer version, moved-worktree repair/move and stale-lock recovery.
- Local macOS bundle build, embedded UI, ad-hoc signature and five attribution
  resources passed. License audit gaps remain separate from these checks.

## Memory interpretation and probe limits

The companion [ledger probes](../../benchmarks/native-retention-ledgers-2026-10-31.json)
record read-only `vmmap -summary` physical-footprint observations separately
from sampled RSS. Native, WebContent, networking and GPU processes are identified
by PID and checked before observation. Original tool units are retained verbatim.
These are point-in-time ledgers, not continuous JS heap measurements.

Resident pages and physical footprint answer different questions: clean mapped
pages can be discarded, while dirty and swapped allocations contribute to the
process footprint. Consequently a high RSS alone does not identify a retained
JavaScript heap or prove a leak. See Apple's
[Analyze heap memory](https://developer.apple.com/videos/play/wwdc2024/10173/).
The probes take approximately one second per process and can perturb nearby
latency samples; the run is instrumented evidence, not a clean timing benchmark.
No forced garbage collection, WebView restart or private engine API is used.

The earlier [15-minute foreground report](../../benchmarks/native-foreground-2026-10-31.json)
is retained unchanged. The longer follow-up uses the same fixture scale and
interaction cadence, but source, binary, instrumentation and machine/cache
state differ. Comparing observations does not isolate the effect of the text
measurement cache. Network push/pull, provider services, other OS targets and a
full working day remain outside this measurement.

## Completed 30-minute foreground observation

Measured on Apple M1 Pro / 16 GiB, macOS 26.7.1, system Git 2.39.2.
The packaged debug app uses optimized first-party crates, not a signed release.
[Raw report](../../benchmarks/native-retention-2026-10-31.json): **passed**,
1,807.95 sampled seconds, 151 completed interaction cycles, only `visible`
WebView observations. Before/after fixture HEAD, branch, refs, staging and
working-state assertions passed. The deliberately reviewed branch round trips
were confined to the disposable `large-b` fixture.

The fixture contains 50,000 large-history commits, 10,000 tracked files, ten
locations, two linked worktrees, two submodules and 200 tracking branches.
No concurrent build/test runner ran while sampling. Three read-only ledger
probe groups were taken during the run and may affect nearby timings.

| Elapsed minutes | Native RSS median (MiB) | WebKit helpers RSS median (MiB) | Sampled total median (MiB) |
| --- | ---: | ---: | ---: |
| 0–5 | 219.24 | 320.20 | 541.67 |
| 5–10 | 220.27 | 412.28 | 633.52 |
| 10–15 | 220.38 | 453.26 | 677.86 |
| 15–20 | 220.45 | 509.06 | 729.53 |
| 20–25 | 204.88 | 513.66 | 718.91 |
| 25–30 | 207.12 | 567.25 | 775.81 |

Native peak: **252.53 MiB**; sampled combined peak: **819.97 MiB**.
After minute five, fitted Native RSS slope was −0.82 MiB/minute and WebKit
helper RSS slope was +7.08 MiB/minute. WebKit slopes were +9.14 during minutes
5–15 and +6.08 during minutes 15–30. An automatic RSS reduction around minute
20 did not establish a stable plateau. **WebKit retention remains unresolved.**
Do not publish this run as proof of flat application RAM or a memory-leak fix.

The WebContent point-in-time physical-footprint readings rose from `138.9M`
to `159.2M` to `243.9M` (original vmmap units); its observed peak ledger reached
`385.8M`. This warrants further allocation/heap investigation. It is not solely
a high RSS observation, but these snapshots still do not identify an owner.

The instrumentation observed one measuring canvas throughout, at most six
Tauri callbacks, at most 2,184 DOM nodes and 24 visible graph rows. Text measures
rose from five to 104 across changing content. These observations narrow the
investigation; they do not prove other resources cannot accumulate.
`performance.memory` was unavailable, so **JS heap was not measured**.

| Scenario | Median (ms) | p95 (ms) |
| --- | ---: | ---: |
| `branch_roundtrip_review_apply_ipc` | 1474.0 | 1601.0 |
| `detail_ui` | 101.0 | 202.0 |
| `manual_refresh_ui` | 1625.0 | 3393.0 |
| `older_graph_ui` | 713.0 | 1320.0 |
| `snapshot_ipc` | 357.0 | 841.0 |
| `switch_repository_ui` | 506.0 | 808.0 |
| `search_ui_current` | 746.0 | 1220.0 |
| `workspace_search_ipc` | 2205.0 | 2488.0 |

These are fixture interaction/IPC timings, not internet pull/push/merge
benchmarks. Repository switching includes UI observation; a branch round trip
includes two reviewed changes. The source commit and measured binary SHA-256
are embedded in the raw JSON, which explicitly identifies uncommitted changes.

Two additional real-Git browser checks passed after sampling: normal history,
diff scrolling/workspace switching/keyboard context, and a large historical
commit with exact late-file patch loading. Total targeted browser checks for
this change: **12 passed**. No full Rust suite or hosted Windows/Linux run was
repeated for this frontend/script change.

Next investigation: allocation/heap captures across equivalent warmed cycles,
then an uninstrumented repeat. Avoid increasing cache budgets, restarting the
WebView or forcing collection to conceal retained allocations.
