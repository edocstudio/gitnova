# Windows installer handoff — 1.3.1

Date: 2026-11-18 (Asia/Bangkok). Branch: `build/windows-installer`,
base main `81a67ab2aac3d387e09dfa6a84a0222348211785`.

The user requested a real Windows `.exe` for a separate gitnova-web publication.
Target is Windows 10/11 x64, release-profile MSVC / per-user NSIS. macOS's local
1.3.0 app is not rebuilt or replaced by this task; the new Windows candidate is
1.3.1. No application runtime logic has changed in this preparation.

## Why a new candidate run

The main checks run [44311189519](https://github.com/edocstudio/gitnova/actions/runs/44311189519)
passed native/IPC, frontend and toolchain checks before Windows graph workflows,
but had no Windows installer at initial inspection. Earlier Windows run
[44247547861](https://github.com/edocstudio/gitnova/actions/runs/44247547861) showed
an ambiguous global Remote selector, slow pending native reads exceeding 15-second
expectations, a periodic snapshot poll contaminating an exact write-refresh count,
and Windows path-separator mismatches in missing-worktree assertions.

Tests now scope the branch Remote tab, pause the background clock and drain pending
snapshots before exact write-refresh counts, and normalize both positive/negative
worktree identity checks. Windows waits are 45 seconds and default test budgets
180 seconds; expected states and safety invariants remain unchanged. There are
no added retries, skipped tests, forced writes or weakened product guards.

The Windows-only release scope keeps full Windows core and all four native/browser
shards, credential-store and installer lifecycle checks. The handoff job depends
on the complete Windows reusable workflow passing. Default full-platform CI and
four-platform release assembly remain available.

## Local checks

- Handoff integrity tests: 5 passed, covering exact bytes, tampering, mismatched
  version/source/platform, incomplete install lifecycle/IPC and publication claims.
- Both workflow YAML files parsed successfully.
- Three affected real-core browser cases passed on macOS: Thai branch review,
  exact branch-refresh count and missing-worktree pruning with unchanged branches.
  This is harness verification, not Windows native acceptance.
- Windows runner results, actual artifact hashes and download handoff are pending.

## Next step

Commit the reviewed branch with `[skip ci]` to avoid launching a duplicate full
cross-platform run, push it, and explicitly dispatch `release-candidate.yml` at
that exact branch with `validation_scope=windows-installer`, `sign_linux=false`
and an empty draft tag. This still runs every Windows check. No tag, public release
or website deployment is part of this step. The GitHub connector lacks workflow
dispatch, so the authenticated GitHub CLI is used only for that missing operation.
