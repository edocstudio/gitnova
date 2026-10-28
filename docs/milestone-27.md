# Milestone 27 — Application read workload and bounded caches

The audit covered native dispatch, Git transport, graph/session state, working snapshots, historical patches/diff rendering, repository switching, workspace search, storage, submodule/worktree discovery and write/recovery paths. Inspection is broader than the discard screenshot; it does not mean every workflow has a measured latency or RSS result.

## Findings and delivered fixes

| Area | Finding | Change / current limit |
| --- | --- | --- |
| Native dispatch | Every invocation spawned a blocking worker without global admission limits | Two active command bodies, 32 outstanding commands; worker-held permits and explicit queue-full error |
| Graph reads | Initial page read status twice; fingerprint separately resolved HEAD already present in show-ref | One status observation per page; redundant HEAD subprocess removed |
| History read protection | Plain metadata log discovered clean-filter drivers unnecessarily | Exact metadata-log flag allowlist; unknown/patch/signature options keep the original protection |
| Graph rendering | Ref scans repeated per row and on dirty-only refresh; stash bases repeatedly searched | Index refs by OID/name and stash bases; stable label dependencies |
| Repository navigation | Search navigation bypassed eight-session admission; count limits ignored large text | Both insertion paths use eight-session retention plus a 32 MiB estimated graph target; inactive sessions are evicted, active data remains visible |
| Working state | File/merge snapshots accumulated for every visited membership | Only current checkout snapshot retained; typed drafts remain separate |
| Operation/stash feedback | Potentially large receipts and stash lists accumulated by membership | Eight recent entries retained; durable operation receipts remain on disk |
| Search cache | Up to eight parsed indexes retained, with admission able to exceed the cap when all were borrowed | Strict two-index resident admission; transient worker memory is separately acknowledged |
| Repeated search | Repeated history traversal and per-entry JSON serialization for size accounting | Weighted, expiring Moka traversal cache with live fingerprint validation; per-entry encoded size computed on creation/load |
| Broad search | Full subjects/excerpts/location paths copied for all matches before paging | Small ranking descriptors; only the requested 100 full results are materialized |
| Persistent storage | Contrary to the user's assumption, SQLite was not implemented | Explicitly record JSON implementation and SQLite FTS5 gap; no silent migration |

## Inspected behavior retained

Historical patch reads stay bounded and on demand; the dock keeps one active historical patch and virtualizes visible code lines. Full-file text measurement/parsing can still cause main-thread work on large patches and needs WebKit profiling. Working diffs and write previews remain fresh Git reads. Recovery copies are streamed with bounded file/byte counters, but large media and configured checkout filters can dominate elapsed time. Submodules/worktrees are discovered on opening their existing repository context; traversal/depth limits exist. Network latency/authentication and conservative preview validation are not made cheap by an in-memory cache.

The existing five-second foreground polling still performs real Git work. Filesystem-driven invalidation and cancellation of obsolete reads are the next architectural performance work, along with incremental SQLite FTS5. A language/runtime change alone does not meet the engineering targets.

## Reproducible measurements

`crates/gitnova-core/examples/perf_audit.rs` creates two temporary real repositories, measures graph/detail/snapshot, broad cold/warm search, alternating repository graph reads and search after reopening the persisted index. It records core-process RSS after each sample. It does not use the live app's catalog or modify user repositories.

Use one runner: `nice -n 10 cargo run -p gitnova-core --example perf_audit -j 1 -- 2000`. This is debug-core measurement, excludes WebKit/IPC, and is neither a release benchmark nor a leak-free certification. Hardware: Apple M1 Pro, MacBookPro18,1, 16 GiB RAM, arm64, macOS 26.7.1. Raw results and verification are recorded separately under `docs/verification/`.

The user's existing native app remains open during this work; no GUI profiling or automated native writes target that session. The new bundle can be inspected independently without replacing its running executable.

Measured in one serial run with 2,100 total commits across two repositories:

| Debug core scenario | Elapsed |
| --- | ---: |
| Initial 100-row graph | 617 ms |
| Commit detail | 216 ms |
| Working/operation snapshot | 810 ms |
| Cold workspace search and index creation | 3,294 ms |
| Warm workspace search, median of 10 | 377 ms |
| Alternating repository graph reads, median of 10 | 609 ms |
| Search after reopening persisted metadata | 658 ms |

Core-process RSS after warm search samples was 8,784–8,928 KiB. This small debug harness excludes Tauri/WebKit, and ten samples do not establish absence of a long-session leak. The main unresolved latency is live Git validation/process work: authoritative warm search still misses the master prompt's sub-100 ms engineering target. Cache alone has not solved that. Raw measurements: [cache performance](verification/2026-10-27-cache-performance.json).
