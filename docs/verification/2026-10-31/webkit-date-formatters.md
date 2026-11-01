# WebKit date formatter allocation investigation

Base source: `0fde87d`, plus this uncommitted change. All native reads/writes
use disposable fixtures. No real repository, account or Git history is changed.
The previous [30-minute run](../../benchmarks/native-retention-2026-10-31.json)
remains unchanged, including its unresolved-growth result.

## Evidence and scope of the fix

The previous run observed bounded DOM rows/callbacks/canvas counts while WebKit
RSS continued growing. An isolated native IPC run repeatedly requested the same
large commit detail without changing the graph/diff DOM. It completed 268 cycles
in three minutes with WebKit helper RSS approximately 171 MiB through the
recorded 30-second observations. This did not reproduce the UI growth.
[IPC raw data](../../benchmarks/native-isolation-ipc-2026-10-31.json).

Inspection found `toLocaleDateString`/`toLocaleString` invoked for each visible
row on repeated React renders, including timestamp tooltips. The same allocation
pattern appeared in commit details, CI/activity and search results. Repeated
locale/options formatting can create new ICU-backed formatters. MDN recommends
reusing an [Intl.DateTimeFormat formatter](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toLocaleDateString)
for repeated calls. This recommendation alone is not proof of the observed
application behavior; the native control experiment below tests that hypothesis.

Two separate one-minute runs used the same packaged app and fixture dimensions
(1,000 commits, 100 files, ten locations). Each performed 2,000 short-date plus
2,000 full-date formatting calls, then idled. One constructed formatters via
Date locale methods; the other reused two Intl formatters. Neither changed the
graph/diff DOM during the controlled loop. Both passed fixture-state validation.

| Control | WebKit helpers RSS near end (MiB) |
| --- | ---: |
| Fresh formatter per call | 256.4 |
| Reused formatter | 142.8 |

The approximately 113.6 MiB difference isolates substantial formatter allocation
in this environment. It does not prove that every growing allocation in every
WebView is this cause. Raw [fresh](../../benchmarks/native-isolation-dates-2026-10-31.json)
and [reused](../../benchmarks/native-isolation-dates-cached-2026-10-31.json) reports
include samples, fixture dimensions, binary SHA-256 and source identity. These
short, hidden-WebView diagnostic controls are not foreground acceptance or
full-application latency benchmarks. They ran serially, not alongside builds.

## Implementation

`date-format.ts` owns a finite set of five display styles and two supported
locales. At most ten formatter objects are retained for the current system zone
signature. Timestamp values and formatted strings are never cached. A changed
system time-zone/DST signature clears the formatter map. Locale changes select
the corresponding formatter immediately; relative-date labels remain dynamic.
Native EN/TH date/time output, including Thai calendar behavior, is preserved.

All production Date locale-method call sites now use this helper: graph dates
and tooltips, selected commit, search results, CI checked times, recorded activity
and auto-fetch time. The auto-fetch tooltip now follows the application language
consistently. Git DTO timestamps, write validation and recovery are unchanged.
No forced GC, WebView restart, private engine API or arbitrary timer-based cache
purge is introduced. This bounds formatter ownership, not total WebKit memory.

## Code quality and verification

- Python soak runner now uses explicit `from` imports, named subprocess aliases,
  conventional formatting and explicit file/pipe cleanup. Critical preconditions
  and fixture-state checks raise errors instead of `assert`, so Python `-O`
  cannot disable them. An invalid duration is rejected even under `python -O`.
- Ruff import/error checks and formatting pass for the touched Python runner.
  Ruff was used as an external tool; no project dependency/lockfile was changed.
- TypeScript typed formatting API and shared ownership replace scattered locale
  formatter creation. Frontend unit tests: 61 passed, covering EN/TH parity,
  reuse, locale switches, invalid dates and system-zone invalidation.
- Five real-Git browser workflows passed: graph/diff/workspace/keyboard, large
  historical patch loading, global language/settings, host-scoped CI, and
  conflict draft/language preservation.
- Seven native command tests passed. The local macOS bundle passed embedded UI,
  ad-hoc signature and existing attribution-resource verification. Full Rust core
  and hosted Windows/Linux suites were not repeated in this turn.

The debug-only probe workload is selected from a fixed allowlist and requires
the existing private fixture catalog. Default interaction cadence is unchanged.
Repository guidance records explicit imports, bounded ownership and error/cleanup
expectations for Python, TypeScript and Rust. This is a focused workflow audit,
not a claim that all source files have undergone a complete style/security audit.

## Foreground visibility attempt

The first long verification attempt stopped after 32.92 sampled seconds because
the WebView remained hidden before the initial foreground wait completed.
[Failure record](../../benchmarks/native-date-formatters-hidden-2026-10-31.json)
is retained. It completed zero interaction cycles and provides no foreground
performance acceptance. It is not silently relabelled or combined with a retry.

A background attempt was explicitly cancelled after 67.99 sampled seconds and
five cycles when a foreground window became available. Its
[cancelled raw record](../../benchmarks/native-date-formatters-cancelled-2026-10-31.json)
has `passed: false`; the interrupted runner version left `failure: null` during
KeyboardInterrupt cleanup. This record is neither acceptance nor evidence of a
memory-limit failure. The runner now records explicit diagnostic cancellation
and still terminates only its own native child. The foreground retry is a fresh
run with a separate report; samples are not concatenated.

## Interrupted interaction verification and remaining allocation

The foreground retry observed 93 completed cycles before losing visibility at
1,169.75 sampled seconds. It is **failed**, not a completed 30-minute foreground
acceptance. The [raw interrupted result](../../benchmarks/native-date-formatters-interrupted-2026-10-31.json)
records its visibility failure and continuing WebKit RSS growth. Its Native peak
was 250.34 MiB and combined sampled peak 627.00 MiB. Its post-minute-five
WebKit slope was +8.44 MiB/minute. This run demonstrates that formatter reuse
alone does **not** resolve all observed WebKit growth. Different durations and
visibility prevent treating its lower peak as a controlled whole-app improvement.

A read-only `heap -s --noContent` observation during that retry reported a
WebContent physical footprint of `151.8M`, peak `264.4M` (original tool units).
It did not expose a JavaScript object retaining path. This probe and the
[vmmap ledgers](../../benchmarks/native-date-formatters-ledgers-2026-10-31.json)
can perturb nearby timing samples and do not establish a JS leak diagnosis.
There was no forced garbage collection or WebView restart during sampling.

Further inspection found unnecessary aggregate-patch parsing for large commits:
the dock parsed the incomplete 512 KiB aggregate preview and then replaced it
with the exact selected-file patch. Historical diffs now parse only the selected
file when the manifest requires lazy loading. Collapsed historical docks skip
parsing/loading. A response key includes membership, commit and exact path, so a
previous file response cannot be rendered as the current selection. Mutable
working/operation previews retain their existing caller-owned content and badge
counts. Four related real-Git browser workflows passed, including collapse/expand
request counts and exact late-file content. This removes verified redundant
work; a full memory-leak fix is not inferred merely from this change.

## Completed background follow-up

The final [background run](../../benchmarks/native-date-formatters-background-2026-10-31.json)
completed 608.62 sampled seconds and 45 cycles with fixture Git state preserved.
This is a hidden-WebView diagnostic, not foreground acceptance. Native RSS peak
was 204.33 MiB and sampled combined peak was 491.27 MiB. WebKit helper RSS
medians were 213.4 MiB in minutes 0–5 and 258.6 MiB in minutes 5–10; its fitted
post-minute-five slope remained +8.68 MiB/minute. **Continuing growth is not
resolved. Neither the lower peak nor the formatter microprobe establishes a
complete memory-leak fix.** Different visibility/duration and other newly opened
WebKit helpers prevent a controlled total-memory comparison against the original
foreground run. Raw attribution caveats still apply.

The final debug counters observed at most four newly constructed date formatters,
one measuring canvas, zero callback-map entries at cycle boundaries and 2,214
DOM elements (25 visible graph rows). These counters support bounded ownership
of those particular application resources; they are not JS heap snapshots.
An additional read-only native `leaks --noContent --nostacks` scan of the
identified WebContent PID reported four Foundation allocations totalling 1,664
bytes. That scan does not explain the larger growth or certify absence of JS
retention, and may perturb nearby timing samples. No allocation contents or
real repository data were published.

The confirmed changes are formatter reuse, lazy historical patch processing and
safer Python diagnostic guards/cleanup. The remaining task is allocation-owner
analysis with strict per-process attribution, rather than another identical soak
or a forced-GC/restart workaround. The whole-app RAM issue remains open.
