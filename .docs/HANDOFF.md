# GitNova session handoff

Updated: 2026-11-28 (Asia/Bangkok). Read the root [AGENTS.md](../AGENTS.md)
first. This checkpoint supplements the latest user request and live Git/process
state. Communicate with this user in Thai.

## Hosted request target branch dropdown

Version 1.18.0 on `feat/hosting-target-branch-dropdown` (base 84b05ad). The PR/MR form lists fetched branches for the selected remote. Provider default and manual target entry remain available, including cross-project requests. Changing the target project clears a list selection. Both GitHub and GitLab browser fixtures passed, including custom entry and review. Built and installed Desktop 1.18.0, including the preceding branch-picker fix. Verified bundle version, signature, embedded assets, binary equality and the native WebView smoke with real Git.

## Branch picker loading on macOS CI

Version 1.17.2 on `fix/branch-picker-loading-race`, based on 2e73523. In GitHub Actions run 46137578265 the macOS desktop E2E timed out waiting for the origin fetch action. The trace shows the scope controls became disabled during the click and Local stayed selected. Opening the picker now sets loading immediately. The regression test checks the selected scope before fetching and exercises real Git fetch, tracking branch creation and pull.

The failing E2E passed four consecutive times against the local bridge. All 112 frontend unit tests and the TypeScript/Vite production build passed. Desktop release packaging and installation are pending until the running Desktop app is closed. Hosted Actions evaluated the previous commit.

## CI and dependency security review

Version 1.17.1 on `fix/ci-and-security-updates`. GitHub run 46135953494 for tag `v1.7.0` built 6bc7964, whose application manifest says 1.17.0. The Linux failure logs show the Git fixture rejected an overlapping remote name at setup, and the graph assertion included the publication badge in the expected tag text. Both regression paths pass locally after the fixes.

Node 26.13.1 is the security-patched build runtime. The locked npm audit reports zero findings. OSV checks of all public Rust registry entries identified fixes in url, cookie and rustix. The GTK3 GLib dependency now uses the verified upstream VariantStrIter backport documented in [vendor/README.md](../vendor/README.md). A post-update scan of 550 registry entries reported maintenance advisories for chrono, proc-macro-error, unsafe-libyaml, zbus_names, zvariant and zvariant_derive. The separate vendored GLib source was compared byte-for-byte with the upstream archive plus the official patch. No Next.js dependency is present. GitHub Dependabot alerts were disabled at review time.

The hosting resolver rejects userinfo in HTTPS remote URLs and requires two ASCII hex digits in escaped project paths. Local targeted verification passed for the 16 remote tests, the graph E2E, nine Node prerequisite tests and 43 hosting unit tests (one ignored). The full frontend unit suite and optimized GLib string iterator smoke passed on macOS. The installed Desktop 1.17.1 bundle passed signature, embedded asset, real Git native WebView and binary equality checks. Hosted CI has not run this patch commit.

## GitHub Actions build triggers

Automatic checks run on pushed version tags matching `v*`. Manual release-candidate builds also require selecting a version tag as the workflow ref. Branch pushes and pull requests do not start builds. This change affects workflow scheduling only. The previous v1.6.0 remote fixture and graph tag assertion failures still require separate fixes.

## Workspace release profiles

Version 1.17.0 on `feat/workspace-release-profiles`. Settings on Releases configures the current workspace's environment order, labels, GitLab production evidence destination and optional Kustomize defaults. Renaming an environment preserves its identity for mappings, confirmations and history. Repository overrides can inherit unchanged template fields. Existing global environment names and repository mappings are read as migration fallbacks. Teams can record a deployment confirmation for a specific environment with a version, confirmer and evidence link. Kustomize config events and deployment confirmations remain distinct calendar activities.

TypeScript, five targeted unit tests and five end-to-end Releases scenarios passed. The installed Desktop bundle passed signature and embedded UI checks, binary comparison and the native WebView smoke test with a real Git graph.

## Calendar Git history

Version 1.16.0 on `feat/release-config-history` (base 0338fce). Config updates read the selected local deploy branch's first-parent history for each calendar month. The renderer resolves the mapped container at each historical change to its overlay, shared bases or patches. The calendar filters by environment, uses local day boundaries and limits reads to 128 matching commits per environment per month with a partial-history notice. Deployment and confirmation activities continue using observed evidence.

TypeScript, three localization tests and three Releases browser scenarios passed. The real Git fixture starts the app after three versions have been committed, then selects historical days and verifies their original versions. License inventory regenerated. Desktop/GitNova.app was built from a672dbe and installed at version 1.16.0. Native WebView smoke, real Git graph, signature, embedded UI and installed binary comparison passed.

## Release activity calendar

Version 1.15.0 on `feat/releases-activity-calendar`. The monthly calendar replaces the daily accordion. Its fixed heat scale counts individual filtered events per local day. The selected day lists repositories, environments, versions and times. Activity filters separate config updates, deployment records and manual confirmations. Environment options follow the workspace configuration. Verified with TypeScript, three localization checks, three Releases browser scenarios and an inspected UI screenshot. The installed Desktop bundle passed signature, embedded UI and native smoke checks.

## Releases progress, snapshots and daily activity

Version 1.14.0 on `feat/releases-daily-summary`. Releases shows determinate fetch progress with reduced-motion support. Successful Kustomize image snapshots persist locally by workspace/repository and validate mapping, deploy membership and environment selection before reuse. Refresh rebuilds the view and workspace fetch updates changed mapped refs, including off-page rows. Daily summary groups observed config changes or production evidence by local day, with distinct repository counts, environment and version. It retains up to 200 observed events per workspace repository and does not backfill provider deployment history.

TypeScript, three localization tests and four browser workflows passed, including zero additional renders on reopening the cached table, daily config grouping and fetch progress at completion. Desktop 1.14.0 passed native smoke, signature and embedded UI checks.

## Fetch workspace in Releases

Version 1.13.0 on `feat/releases-fetch-workspace`. Fetch workspace processes all unique application rows serially, shares each deploy repository/remote fetch, and rechecks cached images for changed tracking refs. Unmapped rows and local branch mappings use origin or a sole remote. The report offers failed-row retry. Leaving Releases stops scheduling further fetches. Checked with twelve disposable repositories, shared remotes, filtered navigation, an unavailable remote, retry and untracked work. TypeScript, three browser scenarios and three localization tests passed. The Desktop bundle passed native smoke, embedded assets and signing checks.

## Environment CI scope

Version 1.12.1 on `fix/releases-unmapped-ci`. Releases omits branch and Jenkins CI status for all environments and no longer scans pipelines for the former Needs action filter. Config images, configuration dates and explicit production deployment evidence remain available. TypeScript and three Releases browser scenarios passed. Desktop installation passed native smoke, signature and embedded UI checks.

## Release repository navigation

Version 1.12.0 on `feat/release-code-navigation`. Releases displays ten repositories per page. The selected row's **Open repository in History** button navigates to that workspace membership through the catalog. Search covers the whole workspace. TypeScript checks, three localization tests and three Releases browser scenarios passed, including pagination with twelve repositories and navigation to a different member. Desktop 1.12.0 passed native smoke, signature and embedded UI verification.

## Environment configuration dates

Version 1.11.0 on `feat/release-environment-dates`. Releases shows Config updated in the environment cell and inspector. The timestamp is the committer date from the latest commit touching the overlay's actual render inputs, including its shared bases and patches, at the pinned revision. A history lookup failure leaves the image available. Production evidence can display its own recorded date when shown in the table.

Verified with separate dated commits for a shared base, UAT overlay and unrelated documentation in a disposable repository. Three release workflows and three localization tests passed. Desktop 1.11.0 passed native smoke, embedded UI and signing checks.

## Search and pagination in Releases

Version 1.10.0 on `feat/release-search-pagination`. Search matches literal
repository names across the workspace, ignoring case. Each page shows up to
five repositories. A serial queue follows the current view. Cached evidence and
images are reused when revisiting pages until refresh or configuration changes.
Needs action checks CI across the matching names to find pending jobs. The
repository mapping event still updates only its own image data.

Verified using seven temporary repositories, counting reads across page changes,
searches and revisits. The release overview and mapping regressions and three
localization tests passed. Installed macOS Desktop 1.10.0 passed signature,
embedded UI and native smoke checks.

## Repository-scoped release updates

Version 1.9.1 on `fix/release-mapping-refresh-scope`. Saving or removing an image
mapping updates only the affected repository's images. An unchanged save skips
the image read. Newer image reads supersede pending results, independently of
the workspace CI scan. Settings opened on Releases contains workspace-relevant
preferences and omits the History repository's commit identity editor. History
still offers repository commit identity through Settings and its footer.

Three Playwright regressions passed using temporary repositories and the real
core bridge, covering scoped saves, repeat saves, Releases settings and History
identity editing. Verified the installed Desktop 1.9.1 bundle and native smoke.

## Configurable release environments

Version 1.9.0 on `feat/configurable-release-environments` moves repository image
mapping to Releases → select a repository → Configure release images. On this
device Settings → General → Visible environments controls names and column order
for all repositories. Defaults are SIT, UAT, Production. A 1.8 mapping keeps its
deploy ref, container and overlay paths and gains overlays/sit. Re-enabling
a hidden environment restores its saved path. Custom environments show configured
images. Production deployment evidence and release-branch UAT CI keep their scopes.

Two Playwright workflows covered the real Rust bridge, Kustomize rendering,
repository selection, a custom column and workspace evidence. Four targeted unit
tests passed including 1.8 migration and localization. Verified the installed
Desktop 1.9.0 signature, native smoke, embedded assets and executable comparison.

## Kustomize release images

Version 1.8.0 on `feat/kustomize-release-images` adds mapping under Settings → General.
Select the app, deploy repository in the workspace, local or fetched remote branch,
two overlay paths and exact container name. Releases renders the selected Git
revision with local kustomize or kubectl, shows the desired image independently
per environment, and keeps GitLab/Jenkins/manual evidence separate. No cluster
connection is used. Fetch the deploy repo to update a remote-tracking branch.

The Playwright release overview regression and cross-repository mapping test
passed with the real Rust bridge and kubectl. The latter verified a base Pod with
two containers, a digest, an inline UAT patch and an uncommitted production-overlay
edit. Three localization tests passed. Live cluster rollout was outside scope.

Installed Desktop 1.8.0 passed native smoke, signature, executable comparison and embedded UI checks. The update retained the previous Desktop app as a backup.

## Workspace Releases

Version 1.7.0 on `feat/workspace-release-overview` adds Releases beside History.
Select a repository to inspect its production evidence and UAT jobs. Needs action
filters the release pipeline. GitLab production evidence uses the latest
successful deployment to the environment named `production`. UAT follows the
latest pipeline on the branch named `release`. Repository-scoped Jenkins jobs
can be opened and builds are matched to the fetched release SHA. Manual production
confirmation records a SHA, version, source link, name and timestamp on this device.

Validated with the GitLab HTTP fixture, three localization tests and two
Playwright workflows covering manual evidence, filtering, direct job links,
repository selection and workspace isolation. External responses were simulated.
Installed Desktop 1.7.0 passed native smoke, signature and embedded UI checks.
The previous app is in `Desktop/.gitnova-update.w3xfbZOa/GitNova-1.6.0.app`.

## Production hotfix for release-only UAT

Version 1.6.0 on `feat/production-hotfix-workflow`. Right-click the deployed
local tag and select Production hotfix. Worktree creation, review branches,
publication actions and tag creation reuse the graph context. Fetch the chosen
remote before both release requests and check the target SHA again at merge.

The UAT review commit has the hotfix tree, the parked release as first parent
and the fix as second parent. Local refs under `refs/gitnova/hotfix/` keep the
checkpoint across sessions and review-branch deletion. After UAT and production
are confirmed, restoration checks the new tag and fetched release tree and
applies the production-to-hotfix delta to the parked release with a three-way
merge. Conflicts retain the checkpoint. Deploy and provider approvals remain
with the team's pipeline or Jenkins.

Sixteen targeted frontend tests and two Playwright flows with real disposable
Git repositories passed, including stale previews, deleted review branches,
candidate validation, dirty work and conflicting restores. Desktop 1.6.0
passed signature, embedded UI, native graph and single-instance checks.
Previous bundle: `Desktop/.gitnova-update.6zb2chDt/GitNova-1.5.0.app`.
Installed binary SHA-256:
`9fe137a37294b9345d751a6be199519b243999e3041ef8919c48ec6ca89c9c3c`.

## Standing delivery preference

The user requires a version bump and a new installed macOS `.app` for every
GitNova code change. The durable instructions and completion checks are in
[AGENTS.md](../AGENTS.md#persistent-settings). This note was documentation only.

## Tag publication symbols

Branch `feat/tag-publication-badges` (base `7e6bfc4`) adds publication
symbols to graph tag badges. The graph checks its selected fetch remote.
A tick means the exact tag object matches, the upload arrow means its name
is absent there, an exclamation mark means a different object has that
name, and a question mark means the result is unverified. Separate push
URLs can have a different inventory. Local and advertised annotated tags
retain their raw tag object IDs alongside the commit used for placement.
Tag Push invalidates the native inventory, and the graph rereads it after
the write without waiting for the normal polling interval.

All 13 graph-state unit tests, 7 Rust tag integrations, and 3 targeted
Playwright workflows passed. TypeScript, Vite, cargo release compilation,
formatting and diff checks passed. GitNova 1.5.0 is installed at
`Desktop/GitNova.app`. The signed bundle passed embedded UI verification and
native startup, catalog, real Git graph and single-instance smoke checks.
The installed binary matches the release bundle. SHA-256
`5e0278efb6115f2fab0c586ad4f20630f35c7c4299dae1696bd94a5d5f9c9c76`. Previous 1.4.1 bundle retained at
`Desktop/.gitnova-update.RWMpdriZ/GitNova-1.4.1.app`.

## Clear labels for remote branches deleted after merge

Branch `fix/prune-deleted-remote-labels` (base `2105061`) fixes a stale
source-branch badge after GitLab merges a promotion and deletes its source.
Full Fetch and Auto Fetch check advertised heads for the selected remote
with a standard fetch mapping. An atomic compare-and-delete clears stale
remote-tracking refs. Existing local branches, tags, symbolic remote HEADs
and working files are preserved. Branch-specific Fetch and custom mappings
retain existing tracking refs. No live GitLab repository was used in testing.

The Rust remote suite passed all 16 cases. Both targeted Playwright flows
passed with real Git fixtures, including the deleted promotion badge and
the retained merged commit. Formatting, TypeScript and the release build
passed. macOS 1.4.1 is installed at `Desktop/GitNova.app`. Native startup,
catalog, real Git graph, single-instance, strict signature and embedded UI
checks passed. The installed binary matches the release bundle. SHA-256
`fdb967cf68f786e26f812adf8927839654394c32482af424b841f3e54f213f1b`.
The previous 1.4.0 bundle is retained at
`Desktop/.gitnova-update.rl4vBvcs/GitNova-1.4.0.app`.

## Promote selected files from Changes

Branch `feat/promote-selected-files-ui` (base `c199551`) adds a dialog from the
existing Changes dock and preselects the focused file. Choose remote source and
target, review branch and exact files. The dialog shows a bounded colored diff,
pinned tips, actions and identity. A matching selection reports a no-op.
It uses the isolated promotion engine from Nova CLI and validates the same
fetch/push destination. Once the preview is approved, publication repeats the
review and uses the existing create-only lease. Provider scope is matched via
Settings → Connections. The published branch remains visible in the dialog
with its recovery directory while PR/MR review and creation run as separate
actions. No live project or provider account was used in testing.

The browser scenario passed with real Git fixtures for preselection, no-op,
exact patch, explicit publication and preservation of checkout/index. The CLI
integration passed. Hosting fixture suite passed 41 cases with the existing
OS credential-store round-trip ignored. TypeScript, formatting and diff checks
passed. Built and installed macOS 1.4.0 at `Desktop/GitNova.app`. The staged
bundle passed native startup, catalog, real Git and single-instance checks.
Installed version, strict signature, binary match and embedded UI verified.
Installed binary SHA-256 `e7e95759ef62bfb5cb8974104814173ae92e838a9c5575a0ec932aea4f4cc64f`. Previous 1.3.4 app retained in a hidden
Desktop backup.

## Refresh a stale remote branch label

Branch `fix/refresh-stale-remote-branch` (base `4f570b0`) fixes the remote
deletion review when the selected graph label is stale. At 1.3.4, normal Fetch
retained stale labels. The review distinguishes a moved branch from an
absent branch and offers Refresh branch label for that tracking ref when fetch
and push share the same destination and a standard mapping. The scoped refresh
fetches objects for a changed tip or clears a verified absent tracking ref.
It compares the selected local OID before writing and preserves local branches,
tags, staging and working files. Deletion still needs a fresh review and an
exact server-side lease. The user's remote was not used for testing.

Seven Rust remote deletion integrations and two Playwright workflows passed
on disposable Git repositories. The new browser scenario passed after
targeting the graph row when local and remote refs share a badge. TypeScript,
formatting and diff checks passed. GitNova 1.3.4 is installed at
`Desktop/GitNova.app`. The staged bundle passed native startup, catalog,
real Git and single-instance checks. The installed copy passed signature and
embedded UI checks and matches the built binary. Installed binary SHA-256:
`56b69e7b3080c55cf7665b9a533c8c4d22e98b124862a301e9a9b9485571141c`. The previous app is retained in a hidden Desktop backup.

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
