# ADR 0011 — Scoped commit search and graph navigation

Status: accepted for the first commit-search increment, 2026-10-26.

## Context and interaction

The multi-repository addendum requires search within the current repository, selected repositories and the entire workspace. Loaded graph rows cannot answer queries against unloaded history or other repositories. The surface justification is recorded in [surface-budget.md](../ux/surface-budget.md). Add one transient search popover in the existing header, opened with Cmd/Ctrl+K; use the existing graph, inspector and diff dock for results. No permanent dashboard or second history viewer.

Search matches plain, case-insensitive words across full commit messages, author names, author emails and hexadecimal commit IDs. Every word must match; there is no regex or file/code search in this increment. The scope selector explicitly names Current repository, Selected repositories or Entire workspace. Selected membership names include paths. Results identify their repository, eligible locations, author, date and OID. Shared worktree results prefer the current location when available, otherwise the first eligible location. Opening a result selects its membership and exact graph row without checkout, ref movement or index writes. Failed navigation retains the original graph and result context.

## Domain and service boundary

`gitnova-search` is independent of Tauri. Core resolves IDs only from the active workspace and verifies the request's current membership. Paths are not accepted from the search UI. Sources are grouped by persisted RepositoryId; each worktree retains its membership, WorktreeId, canonical gitdir and common-directory expectations. Rediscovery must agree with those expectations. Bare repositories use repository membership without a worktree.

Each repository has one disposable, versioned JSON metadata cache under the app data directory's `search` folder. Entries use immutable full OIDs. The cache contains commit metadata and messages, not working files, blobs, credentials or remote data. Immutable metadata survives scope changes and is evicted outside the current reachable set only when a storage budget requires space. Worktrees sharing a repository do not maintain duplicate metadata indexes. Separate repositories with the same OID remain separate search hits.

Reachability is computed afresh for every selected membership. Both graph and search traverse `--single-worktree --exclude=refs/gitnova/recovery/* --all`, adding that location's HEAD when present. Common branch refs remain visible; another linked worktree's private detached HEAD does not silently become a root. A shared commit is returned once per RepositoryId with all eligible selected locations. Internal recovery refs do not become graph/search roots; their objects can still appear when reachable from ordinary refs.

Git documents that `--all` normally examines other worktrees unless `--single-worktree` is supplied: [git-rev-list](https://git-scm.com/docs/git-rev-list). This increment also corrects the graph traversal to use that same scope. Replacement refs and grafts are explicitly unsupported because their interpretation can change indexed metadata without changing the requested OID.

## Reads, persistence and consistency

Use bounded `cat-file --batch-check` to inspect commit type/size and `--batch` to retrieve missing objects. Parse each payload by its declared byte length, not separators inside messages. Embedded NULs, newlines and forged batch-header text remain within their object. Author fields come from the header before the blank line; body text cannot supply a forged author. See the primary [git-cat-file batch format](https://git-scm.com/docs/git-cat-file).

The Git adapter adds a bounded stdin-capable read path: writer and output drains run concurrently within the existing subprocess timeout and process-group termination rules. Reads retain the existing offline environment, disabled filters/fsmonitor/diff helpers and no signature verification or hooks. No lazy network fetch or submodule initialization occurs. Tests configure executable helpers in real temporary repositories and verify that indexing/focus do not run them.

A checksum detects malformed or accidentally altered persisted entries; it is not an authenticity boundary against someone able to rewrite the app's files. Invalid version/checksum/structure rebuilds from Git. Private temporary cache files are flushed and atomically persisted. The source repository is not modified. Source fingerprints are checked before and after the read. Scope, query, fingerprints and coverage contribute to pagination tokens; changed inputs reject stale cursors. Reads do not form an atomic transaction with external Git/filesystem changes.

## Exact navigation

`focus_commit` validates workspace membership and full OID, finds the commit's date-ordered graph rank within a bounded history read, and reconstructs the same preceding lane state as ordinary pagination. It returns one 100-row page, its base rank, selected row and continuation cursor. The UI prepares that target before selecting the repository and replaces only its graph session; other repository contexts remain cached. Focus checks fingerprints and the exact OID at the selected rank. Ref changes, unavailable objects, excessive history output or an unreachable result produce an explicit error rather than opening a different commit. Native focus is read-only; catalog selection is a separate application-state update.

## Budgets and honest partial results

- Query: 1–512 UTF-8 bytes, no NUL, at most 16 distinct words; UI input is limited to 128 UTF-16 units.
- Scope: 1–32 registered membership locations; up to three repository groups read concurrently. One search request runs per service; the UI waits/retries a busy read without repeating Git writes.
- Reachability: latest 20,000 commits per location, at most 40,000 distinct selected OIDs per shared repository.
- Objects: 64 commits per batch, 64 KiB maximum raw commit object; bounded raw and serialized metadata accounting of 32 MiB each per repository, 64 MiB encoded cache file. Up to eight inactive/active repository caches are retained in the service map under normal operation; inactive entries can be evicted as new repositories are read.
- Results: 100 hits per page; ordered by descending author timestamp, RepositoryId and OID. Dates are author dates, not a promise of global topological order across repositories.
- Graph focus: bounded approximately 20,000-commit traversal, existing history output/time limits, at most 512 retained lanes. Deep focus reconstructs preceding lanes and therefore costs more than opening cached rows.

Coverage identifies scanned/indexed commits and issues per repository group. Shallow history, truncated ranges, skipped oversized objects and lossy text are partial coverage. An unavailable or invalid source can exclude its shared repository group; other groups still return results. An empty partial result is not proof that no matching commit exists. A restored source can be searched again. Switching to Selected repositories can narrow an oversized workspace scope.

Cold indexing can involve many bounded subprocess reads; there is no whole-query latency SLA or cancellation of an already running native read. Client sequence guards prevent superseded responses from replacing current results. Search does not yet cover filenames, historical code, blame, reflogs, unreachable objects or arbitrary unregistered submodules/worktrees. Full code archaeology, large-history optimization, relocation and alternate-object-store reconciliation remain separate work.
