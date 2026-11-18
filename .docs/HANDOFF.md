# GitNova session handoff

Updated: 2026-11-18 (Asia/Bangkok). Read the root [AGENTS.md](../AGENTS.md)
first. This checkpoint supplements the latest user request and live Git/process
state. Communicate with this user in Thai.

## Current request and checkout

The user approved implementing in-app CI notifications after discussing a manual
GitLab pipeline screenshot. First scope: active repository's current HEAD/branch,
header bell with an outstanding-action count and a small popover; required,
optional and unknown manual evidence; open commit/provider pipeline; deduplicate
refreshes, retain qualified partial/error evidence and clear changed scopes.
No OS notifications, workspace-wide watch or provider execution/approval in-app.

- Work branch: `feat/ci-action-notifications`, from `a8ae64e` on `main` (PR #62).
- Source version advanced from 1.0.20 to **1.0.21** for this app delivery.
- Previous app work `72b93d0` was merged through PR #61; persistent handoff work
  `acf7753` was merged through PR #62. Do not recreate either feature set.
- Delivered **1.0.21** to `Desktop/GitNova.app`; source/built/installed versions
  and binary SHA-256 match. Strict deep ad-hoc signature, embedded UI, five
  attribution resources and installed native startup/IPC/single-instance smoke
  passed. The previous 1.0.20 app is retained in a task-owned temporary backup.
- New browser flows passed 4/4; existing CI/MR regressions passed 2/2. Frontend
  units passed 103/25 files (seven new classification cases); hosting passed 41
  with one deliberately ignored isolated credential-store roundtrip. Core CI
  integrations passed 2; desktop Rust tests passed 11. TypeScript/build, formatter
  and all-target Clippy passed (desktop rerun after fixture changes).
- **Visible native CI acceptance remains open.** The focused runner stopped
  before CI checks because its WebView stayed hidden. An earlier browser-style
  native invoke stub was invalid (Tauri makes it non-writable); the runner now
  uses typed synthetic CI through real native IPC, gated by explicit acceptance
  flags and a validated private fixture catalog. Do not describe the failed
  attempts as passes or relaunch hidden probes repeatedly. Default native
  workflows remain the old Push/endpoint/editor flow; use `--ci-only true` for CI.
- Detailed behavior, screenshot, structured checks, failed native evidence and
  installed smoke are in the [1.0.21 report](../docs/verification/2026-11-18/ci-notifications/README.md).
- Implementation and local delivery are complete. This handoff is included in
  the feature commit on the named branch; obtain its SHA from Git. No source push or public
  app release is authorized. No test/build/native process is left running.
- Next optional verification: run the focused CI native workflow in a session
  where the WebView remains visible, then actual provider/credential acceptance.
  This does not restart the older seven-item effort or its 30-minute soak.
- Relevant new files: `apps/desktop/src/{CiNotifications.tsx,useCiActions.ts,
  ci-actions.ts,ci-actions.test.ts}`. `PipelineJob.allow_failure` is a nullable
  bool; missing data never implies required approval. UI is English/Thai.
- CI detail reads reuse the existing serialized `readCi`/`pipelines` path and
  bounded native cache. Poll interval is 60 seconds while visible, explicit
  refresh is throttled, and in-flight results are invalidated on scope/config
  changes. Lists survive graph scrolling; reading does not resolve provider work.
- This task does not resume the old foreground soak in item 7 below. New app
  features are tested with disposable repos and simulated provider HTTP/CI
  responses; no real company CI job is started, approved or retried.

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

For an actual new app delivery, follow the existing patch-version, attribution,
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
