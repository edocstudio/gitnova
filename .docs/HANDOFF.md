# GitNova session handoff

Updated: 2026-11-18 (Asia/Bangkok). Read the root [AGENTS.md](../AGENTS.md)
first. This checkpoint supplements the latest user request and live Git/process
state. Communicate with this user in Thai.

## Current request and checkout

User requested a Windows `.exe` to hand to gitnova-web for public downloads.
Prepare an actual release-profile Windows x64 NSIS installer and exact metadata;
website publication remains separate. Current preparation version: **1.3.1**.

- Branch `build/windows-installer`, base main `81a67ab` (PR #64). Workspace rename
  and diff authors are now merged. Local Mac app remains 1.3.0; do not rebuild it.
- Initial main CI run 44311189519 was in progress with no Windows artifact. Earlier
  Windows run 44247547861 exposed stale UI selectors, slow native-read waits,
  refresh polling in a count assertion and Windows worktree separator assertions.
- Harness fixes retain expected states; Windows expect/default budgets 45/180s,
  one Playwright worker. No product runtime logic changed. Source is bumped 1.3.1
  with regenerated local inventory; Windows jobs regenerate their target inventory.
- Added `windows_only` reusable CI input and `windows-installer` release scope:
  full Windows core + all four native/browser shards + credential-store + actual
  NSIS install/installed smoke/reinstall/uninstall, then checked handoff assembly.
  Default full CI is unchanged; no publication or signing gate is declared passed.
- `prepare-windows-handoff.mjs` rejects tampered bytes, wrong source/version/target,
  missing native/lifecycle evidence and unsupported publication/signature claims.
  It emits `.exe`, SHA256SUMS, original evidence and null-public-URL metadata for
  gitnova-web. Existing website files / old published versions are untouched.
- Local checks: five integrity tests, workflow YAML parse and three affected
  macOS real-core browser cases passed. Windows acceptance is not yet passed.
- Next: push the reviewed commit with `[skip ci]` to avoid duplicate cross-platform
  jobs, then explicitly dispatch existing release-candidate.yml on this branch
  with validation_scope=windows-installer, sign_linux=false, draft_tag empty.
  Follow all Windows jobs, fix actual failures, retrieve the successful handoff,
  verify hashes locally and update this report/checkpoint. No merge/public release
  or website deployment authorized by this native preparation step.
- [Task report](../docs/verification/2026-11-18/windows-installer/README.md) records
  provenance and limits. Native CI / long foreground soak from prior tasks remain
  separate. No local test runner is left active at this checkpoint.

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
