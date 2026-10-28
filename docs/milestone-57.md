# M57 — measured read-path optimization and native resource profiling

## Changes

- Working changes derives the displayed count and file list from one fresh porcelain status observation. It no longer performs two full worktree/submodule status scans per read. Ordinary committed status metadata is batched into one rev-parse process; unborn and unusual HEAD shapes retain validated independent probes.
- Working-state fingerprints hash the index incrementally instead of concatenating a second index-sized buffer. The full index still participates in the token; write-time content, native lock and snapshot checks are preserved.
- Tracking display metadata has a disposable weighted 4 MiB Moka cache with a 60-second TTL and 30-second idle expiration. Its key includes live history/shallow/HEAD fingerprint, canonical worktree identity and current effective Git configuration. Core graph/snapshot/search navigation may reuse it; native Pull/Push previews still call fresh uncached tracking and remote validation. Errors are not cached. Weight budgets describe estimated retained values, not process RSS ceilings.
- Broad commit search partitions match descriptors in place and sorts only the requested 100-result window. Full totals, repository/timestamp/OID ordering, cursor validation and coverage limits remain unchanged; no search semantics or persistent index schema changed.
- Idle polling still reads native state. Equal fresh DTOs reuse prior React object identities so unchanged stashes, working lists, operation state, tracking and graph sessions do not rebuild their derived UI/diff state. Changed metadata is accepted even if another token happens to match.
- Local diagnostic builds optimize CPU-heavy first-party core/Git/graph/search/domain crates at opt-level 2. Debug assertions, debug information and inexpensive dependency compilation remain enabled. This is not a public optimized-release/signing/notarization claim.

No additional product screen, preference, network service or permanent panel was added. Existing cache limits, native command admission and graph virtualization remain in force.

## Measurement protocol

The serial operation benchmark uses disposable local repositories with 500 tracked files and a real local bare remote. Baseline executables are retained separately during the experiment; repeated baseline/optimized runs separate preview, apply and one refresh. Wall time includes Git, filesystem and subprocess launch overhead. Network/provider latency and human review time are excluded. Variability is reported rather than inferring speed from the implementation language.

The read benchmark uses 20,000 history commits plus a second 100-commit repository. Cold index creation, warm search, graph, working snapshot and restart are separate samples. Existing literal substring search, partial coverage and budgets are preserved.

The opt-in `apps/desktop/scripts/soak-native.py` uses the packaged app and a real WKWebView with a private temporary catalog, incognito browser storage and synthetic repositories only. It exercises repository switching beyond graph-retention capacity, older pages, commit and working diffs, broad native search and the UI search flow. A reviewed branch round trip runs only in the clean temporary large-b checkout; all fixture HEADs, refs, semantic staging and working content must match at completion. Read-only fixture index bytes must also remain unchanged. Native RSS, newly observed WebKit helpers, sampled Git descendants and DOM/visible-row counts are recorded. The driver is injected only in the validated debug fixture profile; a normal launch cannot enable it by choosing a repository. Sampling is serialized with all builds/tests and includes a 1.5 GiB sampled process-RSS guard. A task-owned idle-sleep assertion lasts only for the probe process; it does not unlock the console or change display settings.

The host console was locked during the overnight run. This profile therefore uses a real background WKWebView and explicitly clicks the existing manual Refresh action after repository switches; it does not spoof document visibility or disable normal foreground polling guards. These results do not measure foreground rendering/frame rate.

WebKit XPC helpers cannot always be attributed solely by parent PID: the report identifies newly appearing helpers and states that unrelated helpers can contaminate that sum. Sampled RSS can double-count shared pages, miss short-lived peaks, and does not measure JS heap or GPU allocations. A bounded soak does not prove leak absence, full-workday stability or live enterprise transport performance.

Results and acceptance limits: [M57 verification](verification/2026-10-28-optimization-and-soak.md).

Website consumption: [portable schema-1 report](benchmarks/README.md) and
[latest.json](benchmarks/latest.json) export the existing measured samples with
signed changes, separate cold/warm workloads, background visibility and hashed
evidence. Export/sync commands do not rerun benchmarks or publish a website.
