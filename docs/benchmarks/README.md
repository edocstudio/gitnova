# Portable performance evidence — schema 1

The [native CI/foreground follow-up](../verification/2026-10-31/native-ci-followup.md)
adds `native-foreground-2026-10-31.json` (kind `gitnova-native-soak`): a completed
15-minute visible native run. It includes actual samples, latency distributions,
binary provenance and growing WebKit RSS; it does **not** prove whole-app memory
stability. The interrupted/occluded JSON attempts are retained with `passed: false`.
These supplemental files do not replace `latest.json` or automatically change
the website's committed benchmark snapshot.

The [2026-10-31 follow-up](../verification/2026-10-31/search-index-and-core-memory.md)
adds two supplemental reports: `search-index-2026-10-31.json` (kind
`gitnova-search-index-comparison`, alternating first-index/restart samples) and
`core-soak-2026-10-31.json` (kind `gitnova-core-soak`, ten-minute core-only
regression). Each declares its limits; neither replaces the historical
`latest.json` baseline or establishes whole-app/full-day memory stability.

`latest.json` is a measured-data report for consumers such as `gitnova-web`.
`artifacts/` contains the exact evidence files identified by byte size and SHA-256.
All inputs are synthetic real Git repositories; do not substitute confidential
project traces, paths, credentials or screenshots.

Re-export the existing M57 evidence without starting GitNova, rebuilding or
rerunning any workload:

```sh
python3 apps/desktop/scripts/export-benchmarks.py
python3 apps/desktop/scripts/test_export_benchmarks.py
```

The exporter reads the five dated inputs under `docs/verification`, checks finite
measurements and required scenarios, computes totals and medians from samples,
and writes this self-contained bundle. It preserves slower results and measurement
limits. It does not fabricate missing baseline samples or infer source revisions.
The environment record is separately captured measurement metadata; update it
only from actual run evidence. This exporter currently targets the dated M57 run;
new experiment protocols need their own input mapping and environment record.

From the sibling website:

```sh
cd ../gitnova-web
npm run sync:benchmarks
# Or import another exported bundle with the same schema:
node scripts/sync-benchmarks.mjs /absolute/path/to/report-bundle
```

The website validates the schema, sample medians, signed changes, duration and
artifact hashes before copying. Its production build reads only its committed
snapshot; it never traverses to this repository or runs native benchmarks.

## Contract

- `schemaVersion`: integer `1`; incompatible changes require a new version.
- `reportId`, `measuredAt`, `environment`, `provenance`: experiment identity,
  dates/timezone, machine and explicit baseline/candidate profiles. Revisions are
  `null` because they were not captured for these uncommitted builds.
- `comparison.operations`: `switch`, `push`, `pull`, `merge`, each with original
  `samplesMs`, `sampleCount`, `medianMs`, Git-call samples and `changePercent`.
  Negative change means shorter elapsed time; positive means longer. Never turn
  every change into an improvement badge.
- Operation totals include the declared preview/apply/refresh sequence, with
  fetch for Pull and finish review/apply for Merge. Switch uses the first feature
  refresh, not the later refresh after switching back to main.
- `comparison.search`: separate cold and immediate warm samples and fixture size.
  `largeOperations` is a single final-build sample, not a baseline comparison.
- `soak`: completion/failure, duration, cycles, fixtures, observed visibility,
  sampled RSS peaks, five-minute interval medians, native/UI latency distributions,
  and attribution limits. Units are ms, seconds and binary MiB as named.
- `limitations`: machine-readable scope flags. Surface background visibility,
  development build, network exclusion, partial search coverage and RSS caveats.
- `artifacts`: relative bundle path, exact byte size and SHA-256. Consumers should
  reject unsafe paths and verify every file before replacing a valid snapshot.

These data compare GitNova builds on one machine and combine code and compiler
profile effects. They do not isolate Rust's contribution, compare languages or
products, prove leak absence, or certify full-workday/production stability.

Full protocol and interpretation are in the accompanying dated Markdown artifact.

## Persistent Rust-core soak (M73)

The separate core-only protocol leaves the normal app open and does not launch a
WebView. Run it as the only build/test/benchmark workload, after building the
probe (never compile while sampling):

```sh
CARGO_BUILD_JOBS=1 cargo build -p gitnova-core --example soak_history --locked -j1
python3 apps/desktop/scripts/bench-core-soak.py \
  --seconds 1800 --commits 50000 --files 10000 \
  --output /tmp/gitnova-core-soak.json
```

The runner requires a Unix `/bin/ps`. On macOS it also checks memory pressure;
failures are exported as failed reports, not silently retried. Inspect `passed`,
`completion.fixtureStatePreserved`, duration and evidence limits before use.
The kind `gitnova-core-soak` is separate from the M57 report schema: do not merge
its RSS numbers into native/WebKit totals or replace historical baseline data.
Latency observations include a cold sample and a capped warm tail per scenario;
`partialObservations` records deliberate history/index coverage limits.

Memory samples identify fixture preparation versus workload. Five-minute window
medians use workload samples only; sampled descendant RSS may miss short-lived
Git peaks. A flat short run is not a leak-free guarantee. Other running programs
can affect elapsed latency. The executable SHA-256 and base-source/uncommitted
flags identify the measured build; there is no new release/version claim.

Current evidence: [core report](core-soak-latest.json),
[memory CSV](core-soak-memory.csv), [integrity manifest](core-soak-artifacts.json)
and [interpretation](../verification/2026-10-30-core-soak.md). A website can read
this supplemental report directly; no website implementation is changed here.

In this protocol, “cold” means first observed in a new Application instance.
It does not imply cold OS caches: fixture creation itself reads/writes the files.


## Advanced lifecycle follow-up (M81)

New samples remain separate from the historical M57 `latest.json` and M73
core-only data. `rebase-publication-m81.json` has kind
`gitnova-rebase-publication-query`: five fixed-order old/new query samples,
Git-call counts and an equivalent-result assertion. This substep comparison
must not be displayed as a complete Rebase or application speedup.

`native-soak-m81.json` has kind `gitnova-native-soak`: the packaged local debug
app, real background WKWebView, selected Git subprocess samples, UI/IPC latency
and explicit attribution limits. Inspect `passed`, `failure`, actual duration,
visibility and fixture validation before displaying it. Report units are named
in the fields (milliseconds for latency; binary MiB for RSS). A fifteen-minute
background run does not certify a full workday, foreground GPU/JS heap, remote
network/provider performance or other OS targets. The data includes measured
build identity and states whether its source had uncommitted changes.

Reproduce separately, with the native app closed and without concurrent
builds/tests; each script changes only fresh temporary fixtures:

```sh
python3 apps/desktop/scripts/bench-rebase-refs.py --output /tmp/rebase-query.json
# First build the local app and ui_bridge, then finish those builds:
python3 apps/desktop/scripts/soak-native.py --seconds 900 --commits 50000 --files 10000 --output /tmp/native-soak.json
```

See [the follow-up report](../verification/2026-10-30-performance-followup.md).


## Foreground retention follow-up (2026-10-31)

[30-minute raw run](native-retention-2026-10-31.json),
[point-in-time memory ledgers](native-retention-ledgers-2026-10-31.json),
[integrity manifest](native-retention-artifacts-2026-10-31.json) and
[interpretation](../verification/2026-10-31/conflict-drafts-and-webview-retention.md)
remain supplemental to historical data. A website may read the raw JSON
directly using its `gitnova-native-soak` kind. Check `passed`, visibility,
actual duration, source/binary identity and limits before displaying results.

This run completed 151 cycles with preserved fixture Git state, but WebKit RSS
and physical footprint continued growing. Do not describe it as flat RAM or a
leak-free certificate. It measures a packaged local macOS debug build; there
is no new cross-platform, provider-network or cross-product speed claim.
The ledger JSON has a distinct diagnostic kind and original tool units.

To reproduce after all builds finish, close GitNova and keep its isolated
fixture window visible for the full foreground interval:

```sh
python3 apps/desktop/scripts/soak-native.py --seconds 1800 --foreground \
  --output /tmp/native-retention.json
```
