# Performance follow-up: M80/M81

This follow-up covers the advanced replay/worktree increment and a reduction in
publication-reference queries. It is not an app-wide optimization or release
readiness certificate. All write fixtures are disposable; verification runners
are serialized with one Cargo job/test thread and one browser worker.

## Inspected bounds

- Branch display cache: Moka, weighted 4 MiB, 60-second TTL/30-second idle limit;
  cached display data never authorizes writes (`display_cache.rs`).
- Search traversal cache: weighted 4 MiB/60-second TTL/30-second idle limit;
  loaded metadata indexes retain at most two shared repository caches. Existing
  index content and result budgets still apply (`gitnova-search/src/lib.rs`).
- UI graph: 1,000 retained rows per location, at most eight recent sessions and a
  32 MiB *estimate* for inactive session eviction. The active session can exceed
  that estimate; it is not a whole-WebView memory cap (`graph-state.ts`).
- Git readers: bounded output and 20-second deadlines. Write previews remain
  live, revalidated snapshots; adding a cache must not hide changed refs, index,
  working content, native operation ownership or submodule state.

## Changed publication query

Rebase previously spawned one `for-each-ref --contains` for each original
commit. The same bounded original range now supplies multiple filters to one
process. Git defines those filters as an OR, so remote refs containing dropped
or folded commits remain in the evidence. The regression exercises two remote
refs containing different parts of the original replay range.

The [Git manual](https://git-scm.com/docs/git-for-each-ref#_notes) documents this
filter combination. This only reduces the publication-evidence substep; it does
not establish a speedup for complete Rebase, branch switching, network Pull/Push
or conflict recovery. No additional application cache or dependencies were added.

## Measured publication query

Five serial fixed-order rounds over 100 commits/10 synthetic remote refs gave
identical reference sets in every round. The old query used 100 Git calls per
round, the new query one. Median elapsed time was 2979.10 ms
before and 33.16 ms after. This is a query microbenchmark,
not complete operation latency; warm filesystem caches and fixed ordering are
explicit confounders. [Raw report](../benchmarks/rebase-publication-m81.json).

## Regression evidence

The first workspace run reached 353 passing tests and one failure before Cargo
stopped at `gitnova-core/tests/writes.rs`. That failure exposed a pre-existing
broad gitlink allowance: a staged deletion without a registered manifest could
reach commit review. Commit and Amend now validate registration A/D against
HEAD and the staged manifest, without reading the working manifest or executing
includes/hooks. Valid paired registration publication is preserved.

After correction, Commit/Amend/submodule lifecycle reruns passed **34 tests**.
The amended negative regression also passed, proving rejection retains HEAD and
index bytes. The other non-desktop/non-core workspace suites passed **61 tests**.
The prior broad run includes all 25 Rebase tests and all 16 maintenance tests.
These are serial macOS results with a corrected targeted rerun, not a new hosted
all-platform CI pass.

Workspace/all-targets Clippy with warnings denied, TypeScript checking, 53 UI
unit tests and local app build/embedded asset verification passed. The cached
build-script newer-macOS linker advisory and Vite chunk-size advisory remain;
these results do not certify older macOS or foreground WebView performance.

## Sustained native/WebKit run

The current local ad-hoc debug app completed **909.34 seconds**
and **60 cycles** over ten synthetic locations, a 50,000-commit/
10,000-file seed, two submodules, linked worktrees, 200 tracking branches,
merge commits and a thousand-file change. No build or test ran while sampling.
All post-run fixture HEAD/branch/staging/normal-ref/working-state checks passed;
read-only fixture index bytes were unchanged. The dedicated clean checkout used
explicit reviewed branch roundtrips and returned to its original branch.

Peak native RSS: **274.27 MiB**. Peak sampled
native/WebKit/Git RSS total: **741.06 MiB**.
Last-five-minute native median: **206.91 MiB**.
Post-five-minute native RSS fitted slope: -1.10
MiB/min; WebKit slope: **+10.31 MiB/min**.
WebKit growth requires a longer foreground/heap investigation. This passing
functional run is **not** memory-stability acceptance or evidence of no leak.
RSS and fitted slopes do not isolate JS allocations, engine caches or paging.

| Scenario | Samples | Median ms | p95 ms |
| --- | ---: | ---: | ---: |
| branch_roundtrip_review_apply_ipc | 6 | 2620.50 | 2978.00 |
| detail_ui | 60 | 202.50 | 408.00 |
| manual_refresh_ui | 60 | 1561.50 | 3314.00 |
| older_graph_ui | 20 | 816.50 | 1816.00 |
| snapshot_ipc | 60 | 571.50 | 1015.00 |
| switch_repository_ui | 60 | 479.50 | 1234.00 |
| search_ui_current | 10 | 1456.00 | 2455.00 |
| workspace_search_ipc | 10 | 15118.00 | 38865.00 |

Workspace search remains a measured bottleneck, not an instant-search claim.
The two-branch roundtrip includes two reviewed switches, not one click's latency.
The observed background WebView state is `hidden`;
maximum sampled DOM count 10120, visible graph rows 25.
No network/provider or cross-product comparison is included. Other applications
can affect latency and new WebKit helper attribution; summed RSS may double-count
shared pages and sampled Git peaks may be missed. The source was uncommitted;
the exact measured executable hash is retained in the raw report.

[Raw native report](../benchmarks/native-soak-m81.json),
[memory samples](../benchmarks/native-soak-m81-memory.csv),
[portable artifact sizes/hashes](../benchmarks/followup-m81-artifacts.json).
The runner now sanitizes inherited Git overrides before fixture/provenance reads.
It uses a task-owned idle-sleep assertion and stops above 1.5 GiB sampled RSS or
below 8% reported system free memory. Neither guard fired. No user repository
was a write fixture; the isolated app was terminated after measurement.

The earlier [30-minute core soak](2026-10-30-core-soak.md) remains a separate
historical sample with its own source/executable hashes and limitations.
