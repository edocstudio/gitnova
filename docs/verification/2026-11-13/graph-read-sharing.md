# Graph read sharing — 2026-11-13

Machine-readable evidence and all 160 timing samples:
[graph-read-sharing.json](graph-read-sharing.json).

## Scope and provenance

Starting HEAD `12a826c61555b81e33d4102c9e1f7137354f4812`, clean working tree,
version 1.0.10. Candidate delivery version: 1.0.11. Both measurement builds used
1.0.10 metadata, before the packaging-only version bump. Runtime changes were
already present in the after measurement. The focused `--graph-only` harness was
compiled from current source for the baseline before runtime edits; an earlier
interrupted full benchmark was discarded. No user repository state, credentials,
remote publication or user Git configuration was used for testing.

Host: macOS 26.7.1 arm64, 10 CPUs, 16 GiB RAM. Rust 1.99.1,
Apple Git 2.39.2 (Git-143). Release profile, one Cargo job and one test/benchmark
runner at a time. The reader resolves the active developer Git executable rather
than repeatedly invoking the system dispatcher. This behavior predates this patch.

## Evidence and implementation

Each warm graph page previously ran 17 Git subprocesses, including 7 rev-parse
calls. After this patch it runs 15, including 5 rev-parse calls, in the same
fixture. Initial uncached tracking takes 23 versus 21 subprocesses.

- `crates/gitnova-git/src/lib.rs`: `graph_read_keys` shares fresh discovery between
  the initial fingerprint and tracking cache identity. Discovery is request-local;
  no long-lived mutable repository observation cache is introduced.
- `crates/gitnova-git/src/tracking.rs`: the internal key builder still hashes exact
  worktree gitdir, fingerprint and a fresh full config observation. Display cache
  capacity, TTL and write-preview behavior stay unchanged. Tracking-key errors
  still allow readable graph history with unavailable tracking metadata.
- `crates/gitnova-core/src/lib.rs`: graph history reuses the already-read status
  HEAD observation instead of spawning an additional rev-parse. The final fresh
  fingerprint check remains; stale cursors and concurrent ref/HEAD changes remain
  errors. Native Git history order, recovery-ref exclusion, row bounds, refs,
  working-tree status and lane layout are preserved.
- Write flows still use their existing fresh identity/config/index/worktree
  checks and reviewed tokens. No workers or network concurrency were added.

## Paired core results

Synthetic disposable repository: 2,000 commits, one tracked file, two local
branches, no untracked files/submodules/network. Twenty samples per scenario
before and after. Cold means a fresh Application/catalog, **not** cold disk cache.
Values are median / p95 milliseconds; p95 uses nearest rank.

| Scenario | Before ms | After ms | Git calls |
| --- | ---: | ---: | ---: |
| First page, cold app | 269.05 / 322.03 | 260.64 / 355.64 | 23 → 21 |
| First page, warm | 193.96 / 235.80 | 192.12 / 220.59 | 17 → 15 |
| Next page | 201.64 / 229.10 | 195.43 / 257.26 | 17 → 15 |
| Refresh first page | 199.43 / 226.80 | 194.65 / 238.75 | 17 → 15 |

The deterministic improvement is two fewer subprocesses per request (11.8% in
warm reads). Median differences are small and three p95 values regress. These
sequential blocks do **not** establish a meaningful end-to-end latency improvement.
Git execution still dominates. Do not advertise a UI speed percentage from these
numbers. Serialization probes are separate encoding measurements, not real IPC.

Reproduce serially with the same toolchain and fixture, using before/after source:

```bash
cargo run --locked --offline --release -p gitnova-core --example perf_reads -j 1 -- --graph-only
```

## Verification

- Release core tests: 35 passed (`read_optimization`: 7; `real_git`: 16;
  `tracking`: 5; `patch_submodule_preservation`: 7), with two test threads.
- Added checks cover shared/uncached key equivalence, command-count reduction,
  external refs and dirty status, shallow-boundary invalidation, linked worktree
  isolation, detached/unborn/bare repositories and newline paths.
- Release Clippy (`gitnova-core`, `gitnova-git`, all targets, warnings denied) passed.
- Version synchronization test passed; Rust formatting and diff whitespace checks passed.
- Existing release checks cover multi-repo identity, symlinks/non-UTF-8 paths,
  SHA-256, pagination/merge topology, annotated tags, child work preservation and
  external-diff/textconv suppression.

## Native release artifact

The native macOS arm64 release build succeeded for 1.0.11 (production frontend
and Rust release profile). The 29.32 MiB app is ad-hoc signed, not notarized.
Installed over the existing Desktop GitNova.app without opening a workspace;
installed version and executable SHA-256 match the candidate. Strict/deep
codesign verification and `--verify-bundle` passed. Attribution resources are
bundled, but the inventory remains incomplete (67 unresolved items); this is not
a license-compliance certification. No Windows/Linux artifact was built here.

## Limits and next bottlenecks

This round does not measure real UI readiness, native IPC queue latency, network
CI on/off, long-session RAM or a large many-file production repository. Existing
selection coalescing, progressive graph refresh and bounded caches are retained.
The graph still performs 15 subprocesses and a fresh status inspection per warm
page; the latter includes child-filter safety inspection and must not be skipped.
Post-write refresh, large-diff rendering and network latency need separate paired
instrumentation before changes. No Windows/Linux runtime test or comparison with
another product was performed. No commit, push or release publication was made.

The packaging frontend build also reports a main JavaScript chunk above 500 kB
and an ineffective dynamic import of StashControls (also statically imported by
WorkingDetails). This is a concrete follow-up for loading/parse profiling, not a
measured cause of Git execution latency. No frontend loading behavior changed
in this patch.
