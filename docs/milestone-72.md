# M72 — Bounded file/code archaeology and coordinated branch evidence

The existing transient commit-search popover adds File / code history and Compare
branches across repositories, sharing Current/Selected/Workspace scope and the
existing exact graph navigation. No dashboard, persistent panel, provider access
or Git write is introduced. Expensive investigation starts on explicit Search;
ordinary commit-metadata search retains its debounce and indexed pagination.

File history uses a literal repository-relative file/directory path. Code search
uses native Git literal pickaxe occurrence-count semantics (`-S`), optionally
restricted by that path. It does not mean every edit to lines containing a word.
Each location contributes at most 2,000 recent ordinary-history candidates;
private recovery/stash roots are excluded. A no-walk filtered traversal inspects
only those candidates. Merge deltas use the first parent; rename following,
blame/line history, regex search and persistent code indexing remain future work.
Selected results open the graph/commit inspector without checkout; a matching
literal path selects its historical file in the existing diff dock.

Branch comparison resolves exact local branch names or explicit full
refs/heads, refs/remotes or refs/tags names to immutable commit IDs. For every
location it reports source-only/target-only counts, changed-file count between
exact tips and source-only commits. Dirty work is called out; comparisons use
committed history. No fetch, merge, push or provider mutation is performed.
Cached remote refs may be stale. Different repositories are never treated as
sharing commit identity merely because their OIDs match.

Both modes accept at most 32 registered workspace locations and display at most
100 deduplicated commit results. Shared-repository result hits retain their
worktree locations. Ref fingerprints are checked after reads; changed refs,
missing branches, unavailable objects, output/time budgets and shallow coverage
remain explicit per location. A 20-second aggregate admission budget prevents
starting further locations after exhaustion; an already-started bounded native
reader can finish beyond that interval. Each Git subprocess retains its existing
20-second deadline and bounded output. Missing/limited evidence never becomes a
successful release verdict.

This is an initial release-investigation evidence slice. It does not certify
semantic safety, dependency compatibility, version/release readiness, hosted CI
approval or a cross-repository atomic operation. See native semantics in
[git-log](https://git-scm.com/docs/git-log) and the
[verification report](verification/2026-10-30-lifecycle-investigation.md).
