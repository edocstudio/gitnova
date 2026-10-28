# Milestone 30 — Visible Push and branch ahead/behind

Push is available beside the current branch and directly on local branches in the graph context menu. Both entries open the existing destination check, outgoing commit review and explicit confirmation. They do not publish immediately. No new inspector or navigation surface is introduced; see [ADR 0025](adr/0025-visible-push-and-tracking.md).

The current branch, local branch picker and graph branch labels show `3↑` for three local commits ahead and `2↓` for two upstream commits ahead. Both can appear together. Synchronized branches show `✓`, missing upstreams show `—`, and unavailable, missing or shallow comparisons show `?` with an explanation. These are commit counts against the configured, locally fetched upstream, not changed-file counts or a live remote query. Fetch is required to discover newer server commits.

Native Git reads tracking in a bounded batch of at most 4,096 local branches rather than spawning a process for each branch. Tracking is separate from repeated write validation. Graph and snapshot refreshes update it; a local branch OID mismatch hides stale comparisons. Retained graph metadata participates in the existing memory estimate. Actual upstream and push destinations may differ, including a different remote branch name; ambiguous push configuration requires explicit destination selection. Existing live destination checks and write guards remain authoritative.

Validation uses temporary repositories and bare remotes only:

- Three native core fixtures cover merge history with three outgoing commits, fetched divergence, a separate push remote, missing/deleted upstreams and shallow history.
- Frontend unit coverage checks arrows, synchronized and unknown states, stale OIDs and Thai labels.
- Three browser scenarios cover first publication preserving dirty work, visible Push after a merge followed by synchronized counters, and a divergent `main` tracking `origin/release`. The divergent case checks direct graph Push and a 1,000-pixel header without horizontal overflow.
- Native IPC coverage checks tracking serialization and nullable counts for an untracked branch.

Validation passed: 3 core cases, 26 frontend unit cases, 3 browser scenarios (plus a focused rerun after correcting blocked-preview text), and 1 native IPC case. TypeScript, Rust formatting and focused Clippy checks also passed.

No user's working repository is pushed or modified during validation. This change does not establish large-history latency or long-running WebKit memory guarantees; exact Git reachability counts still have a cost on unusually large histories.
