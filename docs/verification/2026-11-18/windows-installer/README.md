# Windows installer handoff — 1.3.1

Date: 2026-11-18 (Asia/Bangkok). Branch: `build/windows-installer`,
base main `81a67ab2aac3d387e09dfa6a84a0222348211785`.

The user requested a real Windows `.exe` for a separate gitnova-web publication.
Target is Windows 10/11 x64, release-profile MSVC / per-user NSIS. macOS's local
1.3.0 app is not rebuilt or replaced by this task; the new Windows candidate is
1.3.1. Normal user startup is unchanged. The isolated native acceptance startup now
resolves the user data directory only when no validated fixture catalog is used.

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

## First Windows candidate result

Source `af7209f0d5b2090ea7191f255f696ee6cb118ed1` was pushed and dispatched in
[run 44319555944](https://github.com/edocstudio/gitnova/actions/runs/44319555944).
All four Windows browser shards passed. NSIS successfully built the 1.3.1 x64
installer and verified its five attribution resources (56 unresolved inventory
entries). Installed startup then failed with `setup hook: unknown path` before
the native catalog marker. This run is not a passing installer handoff.

The isolated smoke profile supplied a validated fixture catalog, but eager
`unwrap_or` still resolved Windows user application data. Its redirected profile
folders had not been created. Startup now selects the fixture catalog lazily and
the smoke driver creates all redirected profile directories before launch. Normal
user catalog resolution and fixture validation remain unchanged. A fresh candidate
must pass the full Windows workflow; no failed artifact is delivered or published.

## Next step

Local formatter, smoke JS syntax and all 11 native desktop tests passed.
Commit the startup correction with `[skip ci]`, push
the task branch and dispatch a new exact-source Windows installer candidate.
Track the previous core job separately for diagnostics. Download the successful
handoff only after all Windows jobs and installer lifecycle pass, then verify its
archive digest, PE executable, metadata and checksums locally. No tag, public
release or website deployment is part of this preparation.
