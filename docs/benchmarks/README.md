# Portable performance evidence — schema 1

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
