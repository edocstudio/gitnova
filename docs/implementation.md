# GitNova implementation and usage reference

For the project introduction and quick start, see the [main README](../README.md). This reference retains detailed workflow notes, verification commands and implementation limits.

A local-first, graph-first desktop Git workspace, built with Rust, Tauri 2, React and TypeScript. Apache-2.0.

**Status: early everyday Git client.** Real local repositories, graph/diff inspection, per-file stage/unstage and reviewed commit creation. Remote branch fetch, reviewed fast-forward/diverged integration and reviewed merge/conflict workflows are available; the full multi-repository MVP is still in progress.

## Run

Prerequisites: Rust stable 1.98+ with Cargo, Git, Node 20.x (20.19.0+) or 22.12.0+, and the platform dependencies listed in [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). Follow the [source-build tool installation steps](../README.md#install-the-source-build-tools-first) before running npm: npm does not install Rust/Cargo. Local checks have used Apple Silicon macOS with Rust 1.98.0, Node 20.20.2 and Apple Git 2.39.2. Windows/Linux CI is configured; consult the exact hosted run rather than treating local checks as a hosted result.

```sh
# From the repository root
nvm install
nvm use
cd apps/desktop
npm ci
npm run doctor
npm run tauri dev
```

If you do not use nvm, activate a supported Node version with your preferred version manager. No Rust installation changes are required if `cargo` and `rustc` already resolve correctly.

English and Thai are available from **Settings → General → Language** (Cmd/Ctrl+,); the preference persists locally. Repository text and commit drafts keep their original content.

The workspace picker also offers **Initialize repository…** for a new or existing local project directory. Review its full path and initial branch (default `main`), then confirm **Initialize & open graph**. The parent directory must exist. Existing files remain unchanged and unstaged; initialization creates no initial commit or remote. Existing repositories use Open instead. Bare, nested-in-repository and symlink destinations, ignore/license templates and parent-directory creation are outside this initialization slice. See [M68](milestone-68.md).

Resize history columns by dragging their header separators. Widths persist on this device; double-click or Home resets one column, and arrow keys adjust a focused separator (Shift for larger steps). All six columns share the same layout across the header and virtualized rows. The Graph column has a topology-dependent minimum width to keep lanes visible. See [M69](milestone-69.md).

Enter an existing repository's absolute path. Open the selector beside the GitNova logo to add another repository, switch workspaces, pin repositories, or open a discovered linked worktree/initialized submodule. Cmd/Ctrl+O opens this selector; Escape closes it. Cmd/Ctrl+K searches commit messages, authors, emails and SHAs in the current repository, selected repositories or the entire workspace; choosing a result opens its exact graph/diff context without checkout. Right click a commit (or press Shift+F10) for contextual checkout, branch/tag creation and copy actions. Double click its graph node or exact branch badge to open the corresponding checkout preview; ambiguous branch tips show choices. Click a commit to inspect its metadata and file-grouped, line-numbered patch. Select **Working changes** above history: Unstaged and Staged stay visible in the right inspector, selected code appears in the bottom diff dock, and Summary/Description lead to staged review and explicit commit. Unstaged files stay out of the commit. Arrow keys and Home/End navigate loaded history. History/status revalidate in the background every five seconds while visible and when the window regains focus. Refresh remains available.

Use the trash action in **Working changes** to review a selected set of changed files, or right click a file (Shift+F10) → **Discard file…**. Unstaged-only restores from the index and keeps staged content; staged-and-unstaged restores selected paths to HEAD. Inspect the list before confirming. **Copy recovery location** points to original raw files and staging. During an owned pending merge/cherry-pick/revert, the inspector heading also offers **Discard all changes…**. The existing review shows native-measured phase progress (files/bytes), with loading indicators for preparing/finalizing/refreshing. Percentages describe the current step. Its review keeps the operation pending; use **Review abort…** to cancel the operation. Large regular working-file recovery in this explicit context streams up to 256 MiB per file and 2 GiB of selected working content; staged-blob capture retains its existing bounds. For a submodule such as `_dags`, inspect its pointer diff and **Restore recorded…** separately; open the child repository to discard its own files.

For local standalone macOS use, run `npm run build:local` from `apps/desktop`. Open `target/debug/bundle/macos/GitNova.app` from the project root; it embeds the UI and does not need Vite. This is a local ad-hoc-signed debug build, not a notarized distribution. The local builder always updates `GitNova.app`. Close GitNova before building; a running instance blocks the build instead of creating an alternate app. Duplicate launches of the current build reuse the existing window. `npm run test:native:local` checks actual packaged WebKit/native/Git startup and duplicate-instance handling using an isolated temporary repository; close GitNova before running that check.

Expand **Recent activity & recovery** in Working changes to inspect up to ten recorded operations for the repository, including linked checkouts, after a restart. Historical outcomes and recovery pointers do not certify current Git state or complete backups. Started/unknown results require inspection. No operations are automatically retried and no locks are removed. Coverage is bounded and omissions are reported. See [recorded activity](adr/0020-recent-activity-recovery.md).

`npm run dev` alone is a browser-only preview: local Git access is deliberately unavailable. Use the Tauri command for the functional application.

Right click a commit → **Create worktree here…** to check it out in a separate new folder. Review a new branch, an available existing branch at that commit or detached HEAD. Current staged/unstaged work stays in the source. Open the result through **Related repositories** in the workspace selector. The destination's parent must exist; existing folders and occupied branches are refused. To remove a clean linked checkout, use **Remove worktree…** on its Related repositories item, review its folder and affected workspace entries, then confirm. Its branch and exact commit remain retained; the current/main checkout, dirty/ignored content, locks and submodules are protected. Reviewed Lock/Unlock/Repair are available through Worktree metadata on an existing linked checkout; Repair lists affected metadata locations and never deletes files or moves branches.

## Implemented

- Persisted named workspaces and repository memberships; one repository may belong to multiple workspaces.
- Canonical Git common-directory and per-worktree discovery, persisted repository/worktree UUIDs, symlink deduplication and native-path storage.
- Pinned repositories, recent-first selection order, branch/detached/bare and dirty status in the repository selector.
- Real date-ordered, child-before-parent history with parallel branch lanes, stable positions/colors across pages, a dedicated Branch / Tag column, merge/HEAD rings, compact 36px rows, paginated IPC and a virtualized graph list.
- Warm in-memory selection/scroll restoration per membership (up to eight graph sessions). Each session retains at most 1,000 rows; older-page loading evicts earlier rows with scroll adjustment.
- Bounded Moka reachability cache for repeated commit searches, up to 32 parsed repository indexes within a 64 MiB estimated resident budget, and page-only full search result materialization. Live working snapshots retain only the active checkout; native command dispatch runs at most two blocking commands with bounded admission. See [cache design and performance limits](adr/0024-bounded-read-caches.md). Catalog/index persistence is currently JSON; SQLite FTS5 remains outstanding.
- Local / Remotes branch picker with affected-path preview and explicit confirmation. Fetch a selected configured remote, create a local tracking branch or fast-forward its existing local branch; preserve ahead commits and block divergence.
- Contextual branch rename, recoverable local branch/tag deletion and explicit upstream settings. Reviewed single-branch push inspects the live destination, pins the source commit and publishes by normal fast-forward rules; optional upstream and cache updates report partial failures.
- Per-file stage/unstage with an isolated prepared index, native index lock, revalidation and atomic index publication.
- Reviewed per-file or selected/all-file discard from Working changes: restore unstaged files from the index or explicitly restore selected working/staged content to HEAD; raw private recovery precedes writes. Submodule pointer diffs and recorded-version restore stay separate from child file discard. See [discard design](adr/0018-reviewed-discard.md).
- Saved stash entries at their recorded graph bases, opening reviewed Stash / Apply / Pop / recoverable Drop within Working changes, optional untracked capture, saved-index restoration, shared-worktree entries and conflict retention. See [stash design and recovery limits](adr/0013-reviewed-stash-workflows.md).
- Ordered working-change groups, a stable Summary/Description composer and staged review in the shared diff dock; native configured hooks/signing and durable operation receipts under the app data directory’s `operations` folder.
- Blue-black UI with original SVG icons, a graph/metadata upper area and a collapsible bottom diff dock. Select changed files and switch between real side-by-side and unified patches, with fixed Before/After headings, independent horizontal scrolling, synchronized vertical scrolling, line numbers and virtualized code rows.
- Reviewed graph-context merge into a clean named branch: explicit fast-forward or pending merge commit, side selection/UTF-8 conflict editing, separate staged/unstaged groups, reviewed per-file/all-file unstage with index recovery, full staged review and native abort with retained recovery snapshots. Unsupported gitlink/custom-driver/renormalization cases stop for separate review. See [merge design and recovery](adr/0009-owned-reviewed-merges.md).
- Graph-context single-commit cherry-pick/revert with explicit merge mainline selection, conflict editing, staged review, original cherry-pick author/date and single-parent completion. Reviewed abort saves work before native reset --merge restoration, with no hard/forced fallback. See [operation design and limits](adr/0010-reviewed-cherry-pick-and-revert.md).
- Complete bounded historical file metadata independent of patch truncation, with exact per-file reads for large commits. Root commits compare against the empty tree; merges compare against the first parent. Individual text patches remain bounded.
- English/Thai UI catalogs, persisted language preference and locale-aware dates; original repository content and unknown native diagnostics remain unchanged.
- Parent switches preview pointer-only submodule changes and preserve child checkouts; clean checkout mismatches can be retained when switching back. Dirty content and submodule layout changes remain blocked.
- Recursive submodule discovery with stable parent/child identities, HEAD/index/checked-out pointers, dirty/detached/uninitialized state and child graph navigation. Preview and confirm initialization or restoration of the recorded INDEX revision; preserve the parent index, local branches and nested checkouts.
- Linked worktree listing, branch, locked/prunable detection and opening available worktrees. Reviewed creation from a graph commit supports a new branch, an available existing local branch or detached HEAD, with source work preserved. Reviewed removal checks real file content and ignored files, pins detached/unpublished commits, and removes only reviewed workspace memberships after native success. See [worktree creation](adr/0014-reviewed-worktree-creation.md) and [removal](adr/0017-reviewed-worktree-removal.md).
- Scoped workspace commit search with a persistent shared metadata index per repository, worktree-specific reachability, exact graph navigation and explicit partial coverage. See [search design and limits](adr/0011-workspace-commit-search.md).
- Bounded Git workers, explicit registration boundary, no shell command strings, output/time budgets and stale-pagination detection.

## Verify

From the project root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=2
cargo build -p gitnova-desktop --locked
```

Run these commands one at a time. For a focused change, prefer its named integration target; use `-j 1` and `--test-threads=1` when reducing local load. Compilation defaults to two jobs. Browser checks use one worker and one real-core bridge request at a time.

Frontend and browser workflow checks (start from the repository root):

```sh
cargo build -p gitnova-core --example ui_bridge --locked -j 1
cd apps/desktop
npm run build
npm test
npx playwright install chromium
npm run test:e2e
npm audit
```

Browser E2E uses an emulated transport into the **real Rust core** and temporary Git fixtures. The test-only `ui_bridge` example is not included in the production application. A separate Tauri mock-runtime test exercises actual command dispatch, argument names, serialization and remote-origin rejection. Neither substitutes for full native webview automation on every OS.

Core-only checks (without desktop system libraries):

```sh
cargo test --workspace --exclude gitnova-desktop --locked
cargo clippy --workspace --exclude gitnova-desktop --all-targets --locked -- -D warnings
cargo run -p gitnova-core --example inspect -- /absolute/path/to/repository
```

All mutating test setup is confined to newly created temporary repositories. Tests never write to a developer's repository.

## Architecture

```text
apps/desktop/src             React graph, selection context, typed IPC client
apps/desktop/src-tauri       Tauri host and command/access boundary
crates/gitnova-domain        Domain DTOs, errors and native paths
crates/gitnova-core          Application services and repository-handle validation
crates/gitnova-git           Capability traits, bounded Git adapters and safe index preparation
crates/gitnova-graph         Incremental active-lane layout, no desktop dependency
crates/gitnova-search        Offline shared commit metadata and scoped search
crates/gitnova-workspace     Workspace membership and stable identity rules
crates/gitnova-storage       Versioned, atomic JSON catalog persistence
crates/gitnova-submodule     Recursive gitlink inspection and reviewed recorded checkouts
crates/gitnova-worktree      Linked worktree discovery and reviewed creation/removal
crates/gitnova-ipc           Public DTO boundary
```

See the [multi-repository requirements](multi-repository-addendum.md), [architecture and revised roadmap](architecture/overview.md), [identity/adapter ADR](adr/0001-graph-first-workspaces.md), [UI surface justification](ux/surface-budget.md), [dependency decisions](architecture/dependencies.md), [initial milestone verification](milestone-01.md), [UI/everyday Git milestone](milestone-02.md), and [reference-led layout](milestone-03.md), [branch/diff fixes](milestone-04.md), and [parallel graph refinement](milestone-05.md), and [remote branches / submodule-aware switches](milestone-06.md), and [workflow clarity](milestone-07.md), [audit corrections](milestone-08.md), and [recorded submodule operations](milestone-09.md), and [graph context actions/count correction](milestone-10.md), and [ref lifecycle and push](milestone-11.md), and [reviewed merge/conflict resolution](milestone-12.md), and [cherry-pick/revert](milestone-13.md), and [workspace commit search](milestone-14.md), and [large-history fixes / English and Thai](milestone-15.md), and [stash workflows](milestone-16.md), and [linked worktree creation](milestone-17.md), and [stash graph / large and interrupted merge fixes](milestone-18.md), and [direct Stash/Pop and reserved graph lanes](milestone-19.md), and [safe linked worktree removal](milestone-20.md), and [reviewed discard and direct submodule restore](milestone-21.md), [local standalone app](milestone-22.md), and [recorded activity after restart](milestone-23.md), and [unstage during a pending operation](milestone-24.md), and [discard during a pending operation](milestone-25.md), and [discard progress and process batching](milestone-26.md).

The user's current execution order is [missing features → long-session performance/RAM → wider defect/recovery audit](delivery-priorities.md). Clone now lives alongside Add existing in the workspace picker and empty-workspace form, with local review, explicit confirmation and a restart journal. See [Clone into workspace](milestone-32.md).

The workspace repository picker shows five entries per page and searches registered names/local paths across the current workspace. It loads status only for visible picker rows. See [repository pagination and search](milestone-33.md).

Submodule Working changes now separates child files from the parent pointer, with Open submodule, reviewed Stage/Unstage pointer, and parent pointer commits. See [submodule workflow clarity](milestone-34.md).

The graph now keeps a collapsible Submodules list for the checked-out parent branch, including clean modules, with direct child navigation and reviewed management. See [branch submodule context](milestone-35.md).

## Current limits and next work

Push is available beside the current branch and from local branch context menus. Branch labels and the picker show commit counts such as `3↑ 2↓` against the configured, locally fetched upstream; Fetch discovers newer remote commits. See [visible Push and tracking](milestone-30.md).

A diverged Pull offers explicit merge review in the same surface, preserving both histories and using the existing staged/conflict/finish flow. See [diverged Pull](milestone-36.md).

Pull is beside Push. Fetch is available in the Remotes branch picker and graph context menu. Pull fetches the current branch's configured upstream, previews a fast-forward and requires confirmation; it never switches to a differently named upstream branch or stashes work automatically. See [direct Fetch and Pull](milestone-31.md).

Reviewed native linear Rebase, tracked-text Stage/Unstage hunk, selected initialized-submodule URL sync and worktree Lock/Unlock/Repair are available; see [M56](milestone-56.md). See later milestones below for relative-URL sync (M71), bounded archaeology/branch evidence (M72), cached remote submodule updates (M76), and registration/moved-worktree/text-addition/deletion support (M77). M78 adds bounded two-parent merge recreation/root replay, selected missing-worktree prune and gitlink dependency evidence. M80 adds bounded linear interactive editing on Unix, cousin branch points, clean octopus replay and clean linked-worktree movement. Arbitrary topology scripting, conflictful octopus replay and semantic release certification remain outside current scope. Hosted merge/checks and deployment commands remain open; initial PR/MR creation and opening are available. New local branches can be created and opened at a selected commit; remote selection creates reviewed local tracking branches; pull previews fast-forward integration or offers an explicit merge review when histories diverge. A separate local macOS build produces an ad-hoc-signed standalone debug .app; public installers, Developer ID signing/notarization and optimized release validation remain open.

Status and history are snapshots revalidated periodically and on focus; there is no filesystem watcher yet. Polling is single-flight and pauses while the working-changes UI is writing. Graph context is retained during the process, not across application restarts. History pages rerun traversal with `--skip`, so deep pages cost more CPU. Unusually large ref sets, histories exceeding the page-offset budget, or reads taking more than 20 seconds produce explicit errors. Commits and diff text display invalid UTF-8 lossily; native registered paths remain encoded independently. Windows Git paths not representable as UTF-8 currently return an explicit error.

Submodule conflict stages and removed-but-still-present child worktrees need more work. Reinitializing cached module metadata requires manual Git inspection; initialize protects nonempty destinations and restore requires its commit to exist locally. Discovery is limited to eight nested levels and 256 relations. Repository relocation, alternate object stores, catalog reconciliation after external changes, and very large/complex histories need additional reconciliation/hardening. Hash support follows the installed Git; SHA-1 and SHA-256 fixtures passed on the initial machine. Readiness and semantic safety are not inferred from Git patch applicability.

Status and Working changes use the same expanded per-path count, including files inside untracked folders. See [graph action coverage](ux/graph-actions.md) for supported gestures and the remaining important-action roadmap.

Generic per-file index writes refuse gitlinks, conflicted indexes, pending operations and bare repositories. Existing direct submodule pointer modifications use a separate Stage/Unstage pointer review and can be included in a complete parent commit review; child checkout/files/refs stay intact. M77 provides explicit Add/Remove registration previews; these staged gitlink additions/deletions can be committed or amended without checking out the child. M78 adds paired manifest/gitlink Stage/Unstage registration in Working changes; generic file index/discard actions still yield to the registration workflow. Type changes retain their separate workflow blockers. Reviewed parent switches and submodule recorded-checkout operations handle their explicit supported pointer cases separately. File hashing is bounded to 16 MiB, index reads to 64 MiB, changed-path lists to 2,000 and patch previews to 512 KiB. Historical file manifests are separate complete reads limited to 4 MiB and 20,000 paths; oversized manifests fail explicitly. Normal commit review requires a complete staged manifest and loads bounded code per file when the combined patch is truncated. Amendment uses the same complete staged manifest and bounded per-file delta previews; truncating the combined code preview does not exclude staged files or block confirmation. Fresh index/tree validation, referenced-history acknowledgment and recovery retention still apply. Stage all reviews exact eligible unstaged paths, excludes paths requiring separate workflows, and publishes a prepared index after revalidation; it is limited to 256 MiB of working content and the existing per-file/path bounds. Read previews neutralize configured clean/process filters; raw working-file diffs/status can differ from normalized filter results. Review the actual staged patch before committing. Execution uses the repository’s configured filters/hooks/signing; hooks can alter content and run external programs. Git’s native locks and ref checks protect normal concurrent activity, but reads/revalidation do not form an atomic transaction with arbitrary external filesystem/ref edits. Failed or timed-out operations require inspection and are never automatically retried or rolled back.

Next work: extend arbitrary relocation/discovery beyond reviewed reattach (M77) and selected missing-registration archival (M78), and remaining advanced submodule operations, improve status/polling and large-history search costs, and extend crash/recovery tooling beyond read-only receipt inspection. Commit search, bounded literal file/code history and coordinated branch-tip evidence are available (M72); M78 adds rename-following and registered gitlink dependency ancestry. General blame/line archaeology, package dependency semantics and broader multi-repo acceptance remain open.

Product interaction rules for current and future features: [Workflow clarity across GitNova](ux/workflows.md).

Use Stash or Pop beside Working changes in the graph. Pop restores the latest saved work onto the current branch; selecting a graph stash reviews that exact entry. Options and recovery contains saved staging, Apply-and-retain and recoverable deletion. New nonignored files are included by default when stashing. Applying requires clean tracked/working state and collision review; stash conflicts currently require Git. Pop/Drop require complete loose files-backend metadata with at most 200 entries and retain recovery refs/snapshots. See ADR 0013 before recovering a partially completed operation.

Interrupted owned sessions can be reviewed from Working changes after native Git recovery. **Review session cleanup…** archives only the session record and preserves current files/staging/refs; it cannot close a live native operation or a checkout moved from the original branch/HEAD. Large staged merge results use separate content-identity, visible-code and destructive-recovery budgets. See [milestone 18](milestone-18.md).

Implemented initial flows: [version tags and selected-tag Push](milestone-37.md), and [GitHub PR / GitLab MR creation and opening](milestone-38.md). Provider connection uses native macOS token entry and Keychain; live-account acceptance, hosted merge/checks and provider diffs remain follow-ups.

Direct local/remote branch deletion: [M39](milestone-39.md).

Reviewed latest-commit amendment: [M40](milestone-40.md).

Selected-commit [GitLab pipeline/job status and Copy commit link](milestone-41.md)
use host accounts in header Settings → Connections. CI is opt-in, read-only and uses
quiet bounded caching. [Jenkins Git-plugin build evidence](milestone-42.md)
is also available; full hosted merge eligibility remains a follow-up. Copy links support GitHub/GitLab and explicit self-hosted web origins.

[GitLab commit status in the graph (M44)](milestone-44.md) displays CI between Commit message and Author for visible commits, with bounded cache and quiet refresh. External Jenkins status appears when reported to GitLab; direct Jenkins inspection remains optional.

[Central Settings (M45)](milestone-45.md): header gear or Cmd/Ctrl+, for General language and global company/host CI/Jenkins connections (updated in M46). Commit details stay focused on the selected change.

[Global company / host connections (M46)](milestone-46.md) share GitLab/GitHub accounts across matching repositories and pair optional Jenkins controller accounts per company. GitHub statuses/checks now appear in the graph alongside GitLab commit status. No repository, commit or Jenkins job is required to connect.

[Measured operation latency (M47)](milestone-47.md) removes macOS Git dispatcher/wait overhead and redundant post-write graph refreshes. A reproducible temporary-repository benchmark covers switch, pull, push and reviewed merge/finish without touching developer projects.

[Unrelated clean submodule checkouts during cherry-pick/revert (M48)](milestone-48.md) preserve child checkouts and recorded pointers, with explicit parent baselines for selected merge commits.

[Optional local source branch cleanup after merge (M49)](milestone-49.md) is unchecked by default, runs after verified success, retains recovery refs and reports cleanup separately. Remote branches remain unchanged.

[First-project publication (M50)](milestone-50.md) introduced reviewed Stage all, complete staged manifests with bounded per-file commit previews, first Push and upstream setup. At that milestone, destinations had to exist and remote configuration was in Settings; [M53](milestone-53.md) supersedes those historical limits with contextual Push setup and reviewed hosted-repository creation. Saving an existing remote remains a local configuration step, separate from contacting or publishing to its server.

[Three-way conflict resolution (M51)](milestone-51.md) shows actual Current/Result/Incoming versions in the existing diff dock, with reviewed whole-file choices and per-block current/incoming/both draft selection for supported text conflicts in owned merge/cherry-pick/revert sessions.

[Remote Settings cards and footer context (M52)](milestone-52.md) initially reused global host connections for repository remote configuration and added actual upstream/configured remote identity to the footer. Its Settings section and copyright footer wording are historical; [M53](milestone-53.md) moves destination setup to Push, while the current UI credits GitNova · by edocstudio. Copyright and Apache-2.0 terms remain in LICENSE/NOTICE and README.

[Push destination setup (M53)](milestone-53.md) supersedes the M50/M52 Settings remote section. Settings retains global Connections; Push handles existing URLs or reviewed GitHub/GitLab empty-repository creation, then local remote setup and a separate publication review.


[Read-path optimization and native resource profiling (M57)](milestone-57.md) shares fresh status observations, bounds tracking-display reuse, narrows search ranking and avoids unchanged polling rebuilds. It retains uncached write validation. See its verification report for measured before/after latency and the background WebKit soak limits.


[Saved host accounts for HTTPS Git (M58)](milestone-58.md) connects the existing
macOS Keychain account to selected native Git transports through a process-local
credential helper. It preserves external Git credentials as a fallback, leaves
SSH independent, scopes account access to the chosen destination and prevents
app-token storage in unrelated credential helpers.

[Large amend review (M59)](milestone-59.md) retains the complete staged selection and reuses bounded per-file delta loading when aggregate code is truncated.

[Bounded HTTPS upload compatibility (M60)](milestone-60.md) adds an explicit per-push 16 MiB buffer option for diagnosing chunked-upload rejection, with fresh review and no persistent Git configuration change or automatic write replay.


[Exact-commit graph pipeline (M62)](milestone-62.md) shows GitLab’s latest exact-SHA pipeline state and number in the existing CI column, with a ref tooltip and contextual link. Existing global host connections, bounded reads/cache and hidden-window polling pause remain.


[GitHub Actions graph identity (M63)](milestone-63.md) extends GitHub aggregate CI with exact-SHA workflow numbers, names and contextual run links while retaining bounded reads and explicit incomplete coverage.


[First hosted CI corrections (M64)](milestone-64.md) gates platform-specific helpers, keeps directory-builder mutability local to Unix and serializes native tests that deliberately saturate a shared queue, without loosening checks or changing GitHub settings.


[Source-build prerequisites (M65)](milestone-65.md) adds explicit Rust/Cargo setup and bounded `npm run doctor` diagnostics before npm Tauri dev/build, without installing tools or changing native runtime behavior.


[Large branch-history Push review (M66)](milestone-66.md) supersedes the initial 500-commit publication limit with an exact pinned history summary and explicitly bounded subject list, retaining normal-push and live-destination validation.


[Quiet auto fetch (M67)](milestone-67.md) adds active-repository remote refresh with a one-minute default, General settings, bounded silent transport, operation guards and failure backoff while preserving local graph/worktree context.

- [Milestone 68: local repository initialization](milestone-68.md).

- [Milestone 69: resizable graph columns](milestone-69.md).

- [Milestone 70: hosted checks and UI state regressions](milestone-70.md).


[Submodule URL/hunk correctness (M71)](milestone-71.md) adds supported relative URL synchronization and fixes raw-versus-redacted postconditions and literal diff-command hunk content. Existing Rebase/worktree bounds remain.

[File/code investigation and cross-repository branch evidence (M72)](milestone-72.md) reuses Search commits and existing graph/diff navigation. It makes bounded, shallow, dirty and missing-ref evidence explicit without certifying release readiness.

[Persistent-core resource profiling (M73)](milestone-73.md) adds an opt-in serial
large-fixture soak and machine-readable evidence separate from native/WebKit
profiling. Its scope and completion must be read with each actual run report.

### Connection commit identities (M74)

Settings → Connections can save an optional commit name/email per Git host.
Authentication and commit authorship remain independent. The existing composer
and operation reviews show the effective committer; native Git settings are the
fallback when no rule matches. Git config files and published commits are not
rewritten. See [implementation and scope](milestone-74.md) and
[verification](verification/2026-10-30-connection-commit-identities.md).

### Repository terminal (M75)

The current graph context now opens an interactive terminal dock beside Pull/Push.
It starts at the selected repository/worktree, preserves separate bounded sessions
when switching, and distinguishes Hide from End session. Auto fetch pauses while
sessions are open. See [scope and limits](milestone-75.md) and
[verification](verification/2026-10-30-repository-terminal.md).

### Reviewed newer submodule revision (M76)

The existing submodule Manage preview can update a clean child to a proven-forward
commit from its configured cached remote branch, independently from Restore
recorded. Parent staging stays intact; staging the new gitlink remains explicit.
Nested layout changes and divergent/unknown ancestry stay blocked. See
[scope](milestone-76.md) and [verification](verification/2026-10-30-submodule-newer.md).

### Registration lifecycle, moved checkout repair and text hunks (M77)

Submodule Add/Remove registration reuses the branch group and Manage preview,
with parent index/config backups and retained child history. Moved-worktree repair
uses the original Git locator and updates matching catalog worktree paths across
workspaces without changing identity. Regular text addition/deletion hunks include
initial repositories; native normalization cases retain whole-file staging.
See [scope and supported-case matrix](milestone-77.md) and
[verification](verification/2026-10-30-registration-worktree-hunks.md).

### Lifecycle and dependency evidence (M78)

[Implemented workflows and explicit limits](milestone-78.md) include root replay, two-parent merge recreation, coupled registration staging, selected missing-worktree metadata prune, historical filename navigation through renames and gitlink dependency ancestry in branch comparisons. [Verification](verification/2026-10-30-lifecycle-dependency-evidence.md) distinguishes native, browser and platform evidence.

### Stale operation lock recovery (M79)

[Reviewed lock archival](milestone-79.md) is available from Recent activity &
recovery in normal and pending Working changes. macOS verifies an absent owner,
Git process/native metadata state and the exact reviewed lock before archiving
only the GitNova common-directory lock. Journals, files and staging remain intact;
no native Git locks are removed and no operation is automatically retried.

### M80 — advanced replay and worktree movement

[Implemented workflows and explicit limits](milestone-80.md) extend the existing reviews with linear Pick/Drop/Fixup/reordering, cousin branch points, clean octopus recreation and reviewed clean linked-worktree movement. [Verification](verification/2026-10-30-advanced-lifecycle.md) separates native, browser and platform evidence.

### M82 — committed line and package investigation

[Scope and limits](milestone-82.md) cover current-repository line attribution, historical graph/diff navigation, same-selected-commit detail refresh and committed root-package version/constraint facts with registered-repository matches. [Verification](verification/2026-10-30-archaeology-packages.md) records the native and browser checks.

### M83 — local attribution packaging

[Scope and limits](milestone-83.md) cover offline dependency/asset evidence and five verified resources in the local macOS app. [Verification](verification/2026-10-30-distribution-evidence.md) preserves unresolved audit, signing/notarization and native-platform gates; this is not public release certification.

### M84 — direct conflict block choices

[Conflict resolution](milestone-84.md) reuses the diff dock for reversible Current/Incoming checkboxes above a full Result draft. Review and native save/stage confirmation remain separate. Whole-file replacements are secondary and explicit. Merge, cherry-pick, revert and owned rebase use the same inspector/dock path.

### M85 — large stash review

[Stash preview bounds](milestone-85.md) separate code text from complete path/content identity. The existing inspector loads selected Working/Index patches, includes saved index-only paths and names partial code without blocking the entire stash. Full-content stale checks, collision/gitlink guards and reviewed Pop/Drop recovery remain in place.

Performance follow-up: [M86](milestone-86.md) uses byte-budgeted search index retention, deferred Settings tabs, and viewport rendering for working/operation file lists over 200 entries. Counts and reviewed operation scope remain complete; this is not a whole-app RAM ceiling.
