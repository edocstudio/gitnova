# ADR 0024: Bound read caches and native work before adding services

The application must optimize actual work and memory ownership, rather than infer performance from Rust. This change reuses the graph, search and existing waiting indicators; no new UI surface is needed.

## Evidence and decision

The current catalog is atomically persisted JSON. Commit search also persists checksummed, disposable JSON metadata per repository. There is no SQLite implementation yet. SQLite FTS5 in the master specification is an outstanding architecture milestone, not an implemented feature.

Use [Moka sync](https://docs.rs/moka/0.12.16/moka/sync/struct.Cache.html) for disposable in-process search reachability. Its weighted eviction, expiration and concurrent miss coalescing suit the synchronous Git adapter running on native background workers. Configure a 4 MiB estimated weight budget, 60-second TTL and 30-second idle expiration. Weight includes strings, vector capacity and estimated allocation/key overhead; this is not an RSS ceiling. Run maintenance after lookups. Do not add a daemon, listener, socket or Python runtime.

[Redislite](https://github.com/yahoo/redislite) bundles/manages a Redis server through Python; it adds a service/runtime that this desktop client does not need. [redb](https://github.com/cberner/redb) is an embedded Rust key/value database, but does not replace the required full-text search design. [rusqlite](https://github.com/rusqlite/rusqlite) remains the planned adapter for SQLite FTS5 persistence. Replacing the index needs a separately tested schema/migration and query-semantics decision: current searches support literal case-insensitive substrings, while FTS token matching is different. Do not silently change results or migrate durable user state simply to introduce a cache dependency.

## Freshness and ownership

Reachability keys include canonical checkout path and the existing live refs/HEAD/shallow fingerprint. Every search still discovers/validates registered common-directory and worktree identity, rejects replacement/grafted history, and compares fingerprints after reading. Linked worktrees share commit-object metadata, but retain different reachable sets. Errors are not cached. Dirty status, working patches, previews, authorizations, recovery and Git writes never use Moka results as authority. External branch/HEAD/ref changes create a new key; normal dirty-file changes can reuse history reachability.

M86 replaces the historical two-index admission cap with least-recently-used retention of up to 32 indexes within a 64 MiB estimated allocation budget. This allows small multi-repository indexes to remain warm without retaining 32 large indexes. Entries include owned strings, vector storage and an allocation allowance; this is not allocator RSS accounting. Disk I/O and validation happen outside the global resident-map lock. A rebuilding index leaves resident ownership and is re-admitted with its current weight only after successful validation, so an error cannot leave a grown entry retained at its old weight. Up to three active search workers may temporarily hold indexes outside the resident budget; oversized indexes remain transient. Per-repository raw and encoded accounting remain bounded at 32 MiB, persisted files are bounded at 64 MiB, and deserialized files must also satisfy the accounting limits. Runtime encoded sizes are computed once per entry/load instead of serializing the whole index for every query's accounting.

Search ranks small `(oid, timestamp, location bitmap)` descriptors, preserving exact totals, ordering, pagination and worktree locations. Only the requested 100 results clone full commit metadata, excerpts and repository paths. With the 32-location and 20,000-commit-per-location limits, descriptor accumulation remains bounded by 640,000 matches. Large scopes still incur scanning/sorting and JSON load costs; this is not an incremental FTS implementation.

React retains at most eight graph sessions on both normal loads and search navigation, 1,000 rows per session, eight stash lists and eight recent operation results. Graph retention also estimates string/object weight against a 32 MiB target, evicting inactive sessions before the count limit when necessary. The active graph is never evicted or its repository text truncated merely to satisfy that estimate; it may exceed the target and still needs large-history profiling. This is not a total JavaScript heap ceiling. Working/merge snapshots retain only the active checkout. Typed drafts are retained separately to avoid losing user input. Ref labels are indexed by OID and remote branch name instead of repeatedly scanning every ref for every row; dirty-only polls do not rebuild labels. Stash placement uses indexed bases.

The native dispatcher admits at most 32 outstanding commands and runs at most two blocking command bodies concurrently. Permits are held by the actual worker, even if the caller goes away. A full queue returns a translated busy error; there is no automatic replay of a write. A search command internally uses up to three read workers, so this is not a two-child-process global cap. Active operations are not interrupted by semaphore admission.

## Shared Git read work

Graph loading reuses one status observation for initial HEAD-lane reservation and returned status, with the existing final fingerprint validation. Fingerprinting hashes `show-ref --head`, which already includes resolved HEAD; remove the redundant `rev-parse HEAD` child process. Narrowly allowlisted metadata-only history log commands skip clean-filter discovery. Signature/patch/unknown log options still take the original protected route; working diffs/status retain nested filter neutralization.

## Validation and remaining work

Tests cover resident admission under borrowed entries, weighted traversal eviction, real external ref changes and dirty-file reuse, multi-repo/worktree search and paging, cache corruption, native queue refusal/release, graph bounds and labels, and everyday stage/unstage/commit safety. The serial temporary-repository benchmark is `cargo run -p gitnova-core --example perf_audit -j 1 -- 2000`. It measures debug core latency and RSS, not WebKit or the shipped app's total memory.

Still required: native/WebKit long-session profiling on the new build, large binary diff/recovery profiling, filesystem-driven invalidation and obsolete-read cancellation, incremental SQLite FTS5, disk-cache retention policy, and network/corporate repository measurements. Bounded caches are not proof of absence of leaks. No claim of 60 FPS, sub-100 ms authoritative search, or superiority to other products follows from these changes.


## M57 follow-up

The same bounded in-process policy now covers tracking display rows, keyed by live history/shallow/HEAD, canonical worktree identity and current effective configuration. It never authorizes a write; Pull/Push retain fresh native tracking and remote validation. Fresh status counts/lists share one observation, search ranks only the requested result window, and unchanged fresh UI observations reuse React identities. See [M57](../milestone-57.md) for the measured slice and its remaining limits. SQLite FTS5 and foreground full-workday profiling remain separate work.

## M86 UI loading

Settings mounts Connections and FAQ only when each tab is first opened; visited tabs retain drafts while the dialog stays open and release their content on close. Working changes and operation file lists over 200 entries render only their viewport with four-row overscan. Existing headings/counts and Stage all/commit/operation validation still use the complete manifest. Arrow/Home/End navigation can reach offscreen rows; each mounted list item exposes its logical position and total. Short lists retain ordinary DOM/tab behavior. No new screen, control, service or dependency was added. See [M86](../milestone-86.md) for measured scope and remaining memory work.
