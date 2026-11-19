# GitNova session handoff

Updated: 2026-11-19 (Asia/Bangkok). Read the root [AGENTS.md](../AGENTS.md)
first. This checkpoint supplements the latest user request and live Git/process
state. Communicate with this user in Thai.

## Standing delivery preference

The user requires a version bump and a new installed macOS `.app` for every
GitNova code change. The durable instructions and completion checks are in
[AGENTS.md](../AGENTS.md#persistent-settings). This note was documentation only.

## Remove saved stash from the graph

Branch `fix/stash-graph-removal` (base `55f9353`) adds a right-click and
Shift+F10 removal action that opens the existing stash review for the selected
record. Removal is also visible above the restore review's file list. Drop
keeps its recovery ref and preserves working files and staging. The files
backend supports a packed stash tip with a matching reflog for Drop and retains
other packed refs. Pop with a packed stash continues to offer Apply separately.
Unsupported storage, reference-transaction hooks and inconsistent metadata
still stop the write.

All 18 Rust stash integration tests, three Playwright cases, TypeScript,
formatting and diff checks passed. The macOS 1.3.3 release bundle at
`target/release/bundle/macos/GitNova.app` passed signature and embedded UI checks.
Installed 1.3.3 at `Desktop/GitNova.app`. The staged bundle passed native
startup, catalog, real Git and single-instance smoke checks with a disposable
repository. The installed copy passed strict ad-hoc signature verification and
embedded UI verification, and exactly matches the built binary. Its SHA-256 is
`2ae42798ab33d3ee24dbe86de7b22ebfccf13f9d715bb647b1bb6517843e0b85`.
The previous 1.3.2 app remains in the hidden Desktop backup
`.gitnova-update.nQjvvdB5/GitNova-1.3.2.app`. No build runner is active.

## Line authors styling

On branch `fix/diff-line-author-contrast`, the inline author button now uses the
theme accent for the name, muted text for time, and a compact bordered surface.
Both diff layouts share the same button and keep hover, keyboard focus and commit
details. The existing real-Git Playwright case passed for both layouts, keyboard
activation and Thai labels. The minimum text contrast against the button's
normal and interaction backgrounds is 4.91:1 in dark mode and 7.37:1 in light mode.
The macOS Apple Silicon release build is installed as 1.3.2 at
`Desktop/GitNova.app`, replacing 1.3.0 with the old copy retained in a hidden
Desktop backup folder. The staged bundle passed the native startup, catalog,
real Git and duplicate window smoke checks using a disposable fixture. The
installed bundle passed strict ad-hoc signature verification, matching binary
bytes and embedded UI verification. Installed binary SHA-256:
`b21139338163d02afdc215e6b308092d483e692ab09f48c3400ee3c0f04881e7`.
This was a local macOS delivery on the styling branch. The Windows handoff below
is the earlier 1.3.1 build.

## Current request and checkout

Completed the requested real Windows `.exe` handoff for gitnova-web.
Windows 10/11 x64 / MSVC / per-user NSIS **1.3.1** is built and verified.
The Windows task left the macOS app at 1.3.0 at the time.

- Branch `build/windows-installer`, base main `81a67ab` (PR #64).
- Installer source `734d36405fa446e55471cb8ee3e945c02f4afb46` was committed/pushed.
  Full Windows run https://github.com/edocstudio/gitnova/actions/runs/44339604949
  passed core, all four native/browser shards, credential store, actual installed
  native startup/IPC/real Git/single-instance, reinstall/uninstall and handoff.
  Core: 473 passed, two existing ignored (OS credential test verified separately);
  browser: 172 workflows; frontend: 109; Windows IPC/PTY: eight.
- Actual file:
  `target/windows-handoff/1.3.1-734d364/GitNova_1.3.1_x64-setup.exe`
  Bytes 5695521; SHA-256
  `d57f808fc1ee89a94b6ee94291a2d39a2611291ebedd2b59413cf1acd19708e6`.
  Folder includes SHA256SUMS, WINDOWS-DOWNLOAD.md and exact handoff/evidence JSON.
  GitHub artifact 14448103720, named
  `gitnova-windows-handoff-734d36405fa446e55471cb8ee3e945c02f4afb46`.
  Local archive digest, PE/bytes/hash/source/version and every checksum passed.
  Final documentation commits are separate from this immutable build source.
- Earlier runs exposed Windows selectors/waits/clock/path assertions, eager
  fixture startup path resolution, and transient WebView profile cleanup locks.
  Corrected without skipping tests or weakening Git guards. Windows core uses
  two threads/120-minute cap because prior successful baseline used 86 minutes.
  Detailed provenance, original failures and final evidence are in the task report.
  Do not rerun completed suites just to reconstruct context.
- Local validation: five handoff integrity tests; 11 combined script tests plus
  one existing Linux GPG skip; both workflow YAML parses; three affected real-core
  browser cases; formatter; 11 desktop native tests; smoke JS syntax; diff checks.
- No local runner or task-owned CI run is active. Failed/obsolete runs 44319555944,
  44329598401 and 44337962235 are closed; do not restart them.
- Next step belongs to website/publication work only if requested: review the
  candidate and existing attribution scope, publish the exact installer/checksum
  under a matching version tag, verify anonymous URLs, then update gitnova-web
  following its docs/download-publication.md. Public URL fields are currently null.
  No tag, public release, merge or website deployment occurred in this task.
  Preserve unrelated gitnova-web work and its older published macOS/Linux assets.
- The installer is unsigned. Windows inventory has 56 unresolved license entries
  and auditComplete=false; no completed audit/manual GUI/provider acceptance or
  prior-version migration is claimed.
- [Task report](../docs/verification/2026-11-18/windows-installer/README.md) records
  exact deliverables/checks/limits. Prior long foreground soak remains separate.

## Start a new session

1. Read `AGENTS.md`, this file and the latest user message.
2. At the repository root, run these read-only checks:

   ```sh
   git status --short
   git branch --show-current
   git log -5 --oneline
   ```

3. Check the current version/toolchain files. If Git or the user's request differs
   from this checkpoint, update the plan from current evidence. Do not overwrite
   unrelated working changes or reset to the recorded branch.
4. Read the linked evidence/source files for the affected workflow only. Use the
   existing results as context; run checks appropriate to new changes rather than
   repeating the entire previous verification just to regain context.
5. If asked to continue the seven-item effort below, begin with the outstanding
   verification in item 7. If asked for something else, follow that request.

No clarification is needed about the six completed fixes or their intended
behavior. Ask only for genuinely missing information or necessary decisions;
respect the existing authorization and safety rules in `AGENTS.md`.

## Seven-item reliability/performance effort

| Item | Status at 1.0.20 | Result |
| --- | --- | --- |
| 1. Failing checks/CI | Code complete | Corrected stale child-ref expectations, serialized conflicting stale-lock test fixtures, removed an unused fetch argument and repaired related formatting. Production safety guards remain. |
| 2. Diff-to-editor performance | Complete | Expected-position/context fast path, bounded context and nearest-match fallback; CRLF/UTF-16 handling and LF/CRLF UI coverage. |
| 3. Save versus refresh failure | Complete | A successful save clears the draft and reports success. Retry refresh performs reads only and cannot resubmit old content. |
| 4. Endpoint/Push consistency | Complete | Review requires one fetch URL and at most one separate push URL. Existing multi-URL configuration requires explicit reviewed reduction. |
| 5. Submodule/search repeated work | Complete | One object batch/reachability walk per child, bounded unknown evidence, first-page child navigation and reused search metadata. |
| 6. Return to parent Push | Complete | Restores destination/options after child publication, requires fresh review, and rejects a moved parent source with persistent repository feedback. |
| 7. Native and long-session verification | Partial | Native workflows and 30-minute background soak passed. Uninterrupted 30-minute foreground acceptance remains open. |

The earlier diff-location, footer endpoint dialog and actionable Git-error work
was delivered in 1.0.17–1.0.19 and is included in main. Do not reimplement it.

## Verified evidence and limits

Start with the [1.0.20 report](../docs/verification/2026-11-17/reliability-performance/README.md)
and [structured checks](../docs/verification/2026-11-17/reliability-performance/checks.json).

- Core broad run: 501 passed, three stale-lock tests failed from overlapping
  process-probe fixtures, one ignored. After the test mutex repair, stale-lock
  passed 5/5; submodule diagnostics passed 4/4. There are 504 unique verified core
  cases after repair. Do not describe the initial full command as having exited
  successfully; the report distinguishes the broad run and affected reruns.
- Formatter and workspace/all-target Clippy with `-D warnings` passed. Desktop
  tests: 11. Frontend unit tests: 96 across 24 files. Affected browser workflows:
  11, including LF/CRLF × Unified/Side-by-side. Browser transport is emulated but
  uses real Rust core/Git and disposable repositories.
- [Native workflows](../docs/verification/2026-11-17/reliability-performance/native-workflows.json)
  passed in the visible packaged macOS WebView: reviewed child publication,
  return/review/push of parent, endpoint migration and distant diff editor save.
  Actual refs/content/HEAD/index were checked. This is not OS input or credential
  acceptance on every platform.
- [Background soak](../docs/verification/2026-11-17/reliability-performance/background-soak.json):
  passed at 1,805.36 seconds / 153 cycles; 50,000 commits, 10,000 tracked files,
  10 locations. Native peak 221.34 MiB; last-five-minute median 148.92 MiB; sampled
  combined peak 603.98 MiB. Warm native slope -2.64 MiB/min, WebKit +2.11 MiB/min.
  RSS alone does not prove absence/presence of a leak. Helper attribution and
  shared pages limit interpretation; JS heap/GPU allocation were unavailable.
- [Foreground attempt](../docs/verification/2026-11-17/reliability-performance/foreground-soak.json):
  **not passed**. Stopped at 88.17 seconds / six cycles after the WebView became
  hidden. Do not relabel background work as foreground or bypass visibility
  checks. The stopped task left no app process, as recorded in its diagnostics.
- Desktop/GitNova.app was updated from 1.0.19 to 1.0.20. Source/built/installed
  versions and native evidence executable hashes matched; signature, embedded UI,
  five attribution resources and installed startup/single-instance smoke passed.
  See [installed evidence](../docs/verification/2026-11-17/reliability-performance/installed-bundle.json).
  This is historical evidence: inspect the present installation before claiming
  its current version. No public release was performed by that task; PR #61 was
  subsequently merged into main.

## Next work when continuing item 7

1. Confirm the current checkout and no running GitNova instance. Establish a
   session where the app can remain visible for the whole foreground interval.
   Do not repeatedly relaunch a hidden/stalled probe or change screen-lock/session
   settings to manufacture a pass.
2. Use an already-built matching release bundle and release `ui_bridge`; read
   [soak-native.py](../apps/desktop/scripts/soak-native.py) before executing it.
   From the repository root, after prerequisites are satisfied:

   ```sh
   python3.15 apps/desktop/scripts/soak-native.py --profile release --foreground \
     --seconds 1800 --output docs/verification/foreground-followup.json
   ```

   Run only this runner. It creates disposable fixtures and reports visibility.
   If visibility is lost, preserve the failed evidence and inspect current state
   before deciding on another attempt. Never overwrite the earlier evidence with
   a different run.
3. If investigating WebKit growth, use an allocation/heap profile and controlled
   conditions; do not make speculative cache changes or claim the RSS slope
   proves a leak. Preserve native safety and memory/process budgets.
4. Separate later acceptance work: actual provider/credentials/network scenarios,
   Windows/Linux GUI, and the existing attribution audit. The inventory still
   reports `auditComplete: false`, 67 unresolved entries and 27 manual terms
   reviews, unchanged by 1.0.20. Local packaging is not public-release clearance.

These are open validation items, not a requirement to launch a 30-minute run at
every new session or permission to publish, force-push or alter real repositories.

## Where to work

| Concern | Entry points |
| --- | --- |
| Product context/surface rules | [workflows](../docs/ux/workflows.md), [surface budget](../docs/ux/surface-budget.md) |
| Architecture and broader limits | [implementation](../docs/implementation.md), [platforms](../docs/platforms.md), [contributing](../CONTRIBUTING.md) |
| React orchestration and typed IPC | `apps/desktop/src/App.tsx`, `api.ts`, `contracts.ts`; Tauri commands in `apps/desktop/src-tauri/src/main.rs` |
| Diff/editor anchor and save recovery | `apps/desktop/src/edit-location.ts`, `WorkingFileEditor.tsx`; `crates/gitnova-git/src/working/edit.rs` |
| Endpoint migration | `apps/desktop/src/RemoteEndpointDialog.tsx`, `crates/gitnova-git/src/remote_endpoint.rs` |
| Parent/child Push | `App.tsx`, `GraphActions.tsx`, `RefManager.tsx`, `PushSubmoduleHelp.tsx`; `crates/gitnova-git/src/push_submodules.rs` |
| Search focus | `crates/gitnova-core/src/search.rs` |
| Regression fixtures | `crates/gitnova-core/tests/{ref_and_push,push_submodule_diagnostics,remote_endpoint,search,stale_lock}.rs`, `apps/desktop/e2e/graph.spec.ts` |
| Native acceptance tools | `apps/desktop/scripts/native-workflows.mjs`, `native-workflows-driver.js`, `smoke-local.mjs`, `soak-native.py` |

Toolchains are pinned in `rust-toolchain.toml` (1.99.1), `.nvmrc` (26.13.0)
and `.python-version` (3.15.2). Verify availability; do not upgrade tools or
dependencies just to resume. Build/verification commands and resource limits are
in the linked contributing/implementation guides and root `AGENTS.md`.

For an actual new app delivery, follow the semantic-version, attribution,
closed-app packaging and installed-version verification rules. Documentation-only
work does not itself deliver a new app or require rebuilding it.

## Maintaining this checkpoint

At a meaningful boundary, update the current request, Git baseline, completed/open
items, evidence and exact next step. Record any active task-owned process and how
to inspect/stop it; process/session IDs alone can become stale. Keep historical
logs and benchmark data in `docs/` and link them here. Preserve qualifications on
failed/partial verification. Remove stale instructions rather than accumulating
contradictory session diaries. Never store tokens, private account details or
assume an old app-closed confirmation still applies.
