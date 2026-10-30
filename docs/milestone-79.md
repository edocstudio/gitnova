# M79 — Reviewed stale operation lock recovery

Open **Working changes → Recent activity & recovery → Recover stale lock…**.
The same disclosure is available in the pending-operation inspector. Inspection
is read-only. If eligible, **Archive stale lock** revalidates and moves only
`gitnova-operation.lock` to a unique `gitnova-operation.lock.stale-<pid>-<id>`
file beside it. The original bytes remain available at the displayed location.

This action covers the repository common directory, including linked worktrees.
It does not clear native `index.lock` or other Git locks, erase journals, continue
an interrupted operation, retry a write, reset staging, change branches, or
modify working files. After archival, inspect the existing operation separately.
There is no automatic stale-lock deletion based on age.

## Eligibility and confirmation

The current implementation requires macOS process verification. Other platforms
return an explicit refusal when a lock exists. Checks include:

- A regular, non-symlink file, bounded to 128 bytes, with the exact known PID format.
- A positive PID whose absence is confirmed by the OS. A live/reused PID, permission
  failure or uncertain ownership blocks recovery.
- No native merge, cherry-pick, revert, rebase, sequencer or bisect state, and no
  recognized native locks in the current/common or other linked-worktree metadata.
- No active Git executable/helper visible to the bounded macOS process probe.
  A Git job in another repository can conservatively block recovery; wait for it.
- Matching lock inode/content/time, common-directory identity and fresh HEAD/index
  snapshot between preview and confirmation. Unknown, changed or oversized state
  requires another inspection.

Confirmation uses an advisory exclusive lock on the existing inode to serialize
recovery attempts without creating another stale recovery-lock file. It checks
ownership/native state again, then archives only the reviewed pathname. A new
normal operation may acquire a fresh lock afterward. File inspection and rename
are not an atomic transaction with arbitrary filesystem tools; external manual
metadata edits must stop during recovery. No claim of general crash recovery or
atomic Git-operation rollback is made.

## Validation

See [verification](verification/2026-10-30-stale-lock-recovery.md). Real repository
writes in tests use temporary fixtures only. Existing project work and the M78
changes were retained; no commit, push or hosted setting change is part of M79.
