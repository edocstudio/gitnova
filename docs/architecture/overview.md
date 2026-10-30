# GitNova architecture

Status: alpha 0.1.0. Multi-repository history, reviewed Git writes, scoped workspace commit search and optional host integrations are implemented with documented bounds. The broader MVP and native cross-platform acceptance remain in progress. The multi-repository addendum takes precedence over the original milestone numbering. See the [current limits](../implementation.md#current-limits-and-next-work) and [platform status](../../README.md#project-status); milestone-specific restrictions below describe their original delivery stage unless explicitly marked current.

## Product boundary

A local-first, graph-first desktop Git workspace. A workspace contains memberships in repository working directories; it is not a superproject. One repository can belong to several workspaces. Local history, status, diffs and supported local Git writes work offline without a GitNova account or connected provider. Operations that contact a remote, such as Fetch, Clone and Push, require network access unless their source/destination is local. There is no source-code telemetry.

GitHub/GitLab hosting and CI connections, and Jenkins accounts, are optional. Their global host/company mappings live in **Settings → Connections**. On macOS, the native hosting adapter verifies an account and stores its token/login in macOS Keychain: GitHub/GitLab use the `gitnova.hosting.v1` service scoped by provider plus normalized HTTPS origin/base path; Jenkins uses `gitnova.jenkins.v1` scoped by controller plus username. Connection mapping metadata is nonsecret local JSON; tokens are not stored in that catalog. The current non-macOS adapters return an unsupported-account-storage error; this is not a claim of Linux Secret Service or Windows Credential Manager support.

On macOS, explicit HTTPS Git transports can use a matching saved host account through the native executable's process-local credential helper. Selection is restricted to the validated destination; missing app credentials can fall back to configured Git helpers. SSH uses the user's configured authentication/agent. Denied Keychain access is reported rather than silently selecting another account. See [M58](../milestone-58.md) and the [hosting adapter](../../crates/gitnova-hosting/src/lib.rs).

## Domain and identity

- WorkspaceId: persisted UUID, independent of paths.
- RepositoryId: persisted UUID for a canonical Git common directory, scoped to this installation. Names and remote URLs are not identities. Linked worktrees share this identity.
- WorktreeId: persisted UUID for a canonical per-worktree Git directory. A bare repository has no WorktreeId.
- Workspace membership points to a repository plus an optional worktree. Bare repositories remain browsable.
- NativePath retains OS-native bytes/code units in hexadecimal and provides a lossy display string separately. UI commands pass registered membership IDs, not arbitrary paths after registration.
- Submodule relation fields in the Submodule DTO connect explicit parent relation/repository/worktree identities, a native relative Git path, child identities, index gitlink, HEAD gitlink and checked-out commit. Independent nested repositories do not create this relation.
- StorageIdentity is currently the canonical common directory. Alternate object databases and relocation need explicit reconciliation later; paths are discovery locators, UUIDs are domain identities.

## Dependencies and ports

React/TypeScript → typed command DTOs → Tauri host → application services → capability ports → Git CLI and JSON storage.

Rust crates: domain (DTOs and identities), git (capability traits and CLI reader), graph (incremental lane algorithm), search (shared commit metadata and scoped queries), workspace (membership rules), submodule (relation discovery and reviewed recorded-checkout/URL-sync services), worktree (inspection and reviewed creation/removal/metadata maintenance), hosting (optional provider/CI APIs and macOS credential storage), storage (atomic local persistence), core (orchestration), ipc (public DTO exports).

Core crates do not depend on Tauri. Archaeology, changes, release and readiness are extension boundaries, not empty crates. Add them when they have behavior to implement. The provider supplies RepositoryReader, HistoryReader, RefReader, StatusReader, DiffReader, WorkingTreeReader, IndexWriter and CommitWriter. Write traits include snapshot revalidation and operation coordination. Do not advertise unsupported operations.

CLI first: use installed Git for a compatibility baseline. Add gix/git2 per measured capability, after API, hash-format, config and license checks. No automatic fallback after a partially completed write. No adapter-specific objects cross the domain boundary.

## History and graph

Git provides date-ordered traversal that keeps children ahead of parents while interleaving parallel histories. Rust computes stable sparse lanes with explicit incoming strokes and persistent color identities. Version 2 graph cursors carry these slots between pages; the checked-out revision reserves column zero without a phantom incoming edge. See ADR 0004. Traversal uses shared refs plus the current worktree HEAD with `--single-worktree`; other worktrees’ private detached HEADs are not implicit roots. A refs/HEAD/shallow snapshot fingerprint invalidates stale cursors. Pagination currently reruns bounded `git log --skip` traversal: memory and IPC are bounded, but deep-page CPU cost increases. This is an explicit initial limitation; persistent revision walkers are the next performance step. Each page includes at most 100 commits. UI windowing bounds rendered rows. An octopus merge may require many lanes; horizontal scrolling preserves correctness.

Commit inspection validates full OIDs and reads commit metadata plus a first-parent diff (root commits use the empty tree through `diff-tree --root`). Merge inspection is explicitly labelled first-parent; combined/per-parent inspection is deferred. Diff output is bounded, and truncation is visible. A separate complete bounded numstat manifest supplies historical file lists/counts; selected-file reads recover exact patches beyond the aggregate preview budget. No historical blob indexing. See ADR 0012.

## Safety boundary

Only local application UI may invoke registered commands. CSP prevents remote scripts/connections. UI has no shell, filesystem or generic Git command permission. Repository handles must be registered by explicit path entry. IPC validates IDs, OIDs, page bounds and cursors. All CLI invocations use argument arrays with a fixed executable. Git environment overrides are removed; pager, optional read locks, fsmonitor, external diff, textconv and configured clean/process filter commands are disabled during reads. Offline read protocols are blocked to prevent implicit promisor fetching. Explicit push-destination inspection allows only standard Git transports; authentication follows the optional macOS host-account bridge and configured Git-helper/SSH paths described above, and inspection never runs pre-push hooks. Inspection never initializes submodules or invokes custom update commands. Git reads run on blocking workers with subprocess time/output bounds. The UI has no arbitrary shell-command API; native authentication helpers may execute programs, and confirmed writes can run configured hooks, signing and filters. The macOS account-entry flow uses a native hidden-answer prompt through osascript. Native Git remains an external prerequisite.

Catalogs, search indexes and operation receipts use bounded local JSON persistence under Tauri's application data directory with atomic replacement. Read caches are separate from authoritative write validation; SQLite FTS5 remains planned. Recovery snapshots can contain original source files and must be treated as sensitive local data. Optional macOS tokens persist in Keychain as described above, not in these JSON records. See [bounded caches](../adr/0024-bounded-read-caches.md). Git writes use native index locks, a common-directory operation guard and durable receipts; an in-process mutex is not protection against external Git clients. Reads detect ref changes, but working-tree status is a point-in-time observation, not a transaction.

## Delivery plan and acceptance

The user's current execution order is maintained in [delivery priorities](../delivery-priorities.md): missing planned features first, then long-session speed/RAM measurement, then broader real-use defect/recovery auditing. Essential per-feature safety tests remain part of every increment.

The following phases express delivery order, not a list of wholly unimplemented features.

0. Foundation: workspace, architecture, ADR, desktop host, contracts, CI.
1. Multi-repo foundation: persisted workspaces and memberships, explicit add-existing, pinning, fast switching, branch/dirty status and per-membership graph context.
2. Graph/everyday Git: first deliver real paginated history → Rust lanes → Tauri → interactive graph → commit metadata/diff. Then design and test stage/unstage, commit, branch writes and remote operations in small milestones.
3. Submodule/worktree: first read-only recognition and linked-worktree discovery; later safe initialization/restore with recorded-vs-newer distinction, dirty/unpushed checks and explicit previews. Recursive writes are never implicit.
4. Workspace search/archaeology: shared repository commit index, worktree-specific reachability, scoped search, blame/history and exact navigation.
5. Smart preview: snapshot source/target, avoid hooks/filters and active-worktree mutations, uncertainty and stale-result handling. No execution.
6. Cross-repository release intelligence: evidence-linked assessments, Unknown when unverified. No deployment.
7. Advanced Git writes and recovery: separately reviewed merge/rebase/cherry-pick/revert/conflict workflows. No blind batch writes.
8. Artifact/review integration and cross-platform hardening. Trusted approvals must not unlock deployments.

Current implemented scope includes reviewed Clone from the workspace picker; recorded submodule initialize/restore and selected initialized-child URL sync; linked-worktree list/open/create/remove and metadata Lock/Unlock/Repair; and workspace-wide commit search. Clone is described in [M32](../milestone-32.md). Worktree repair currently requires an existing registered linked directory; it does not discover moved checkouts or prune missing ones. See [M56](../milestone-56.md) for metadata maintenance and hunk/Rebase bounds. These individual capabilities do not establish full lifecycle, cross-platform or native desktop acceptance.

The current workspace picker also supports reviewed local initialization ([M68](../milestone-68.md)), preserving existing files without staging or creating an initial commit. A bounded active-repository auto-fetch scheduler ([M67](../milestone-67.md)) shares operation guards and noninteractive transport; local refresh and remote fetch remain distinct. History column widths ([M69](../milestone-69.md)) are device-local presentation preferences, separate from repository identity and graph topology. [M70](../milestone-70.md) records targeted CI/UI regression fixes and the limits of local verification.

Remaining work includes worktree prune/moved-directory reconciliation, submodule add/remove and update-to-newer-remote workflows, broader Rebase/hunk support, file/code archaeology, smart preview and cross-repository release intelligence.

## Risks

Git version and SHA-256 availability vary by OS. Symlink/relocation/alternate-object-store identity needs care. Non-UTF-8 paths must survive storage even when display is lossy. Status may be expensive on huge trees. Git subprocesses need budgets. External edits can invalidate snapshots. Webview rendering, installers, signing and accessibility need actual OS testing. Local measurements and a bounded background-WebKit soak are recorded in [M57](../milestone-57.md); they do not establish foreground/full-day behavior, cross-platform performance or latency guarantees. Dependency licenses, assets and transitive notices still need a complete audit and artifact-level verification before binary distribution; use the [distribution checklist](../contributing/binary-distribution.md).

## Everyday write foundation (historical scope)

Per-file stage/unstage uses an isolated temporary index while holding Git's native `index.lock`, then revalidates HEAD/index/file content and atomically publishes that index. A shared-common-directory GitNova lock coordinates operations across linked worktrees and app instances. Git's locks remain authoritative for external clients. Never remove an existing lock automatically.

Commit requires an explicit staged patch review and a snapshot token bound to HEAD, symbolic HEAD and index bytes. It uses native Git commit with configured hooks/signing and does not stage additional paths. This foundation originally covered normal commits; the current composer also offers a separately reviewed Amend workflow, including complete staged manifests and bounded per-file delta inspection ([M59](../milestone-59.md)). Refuse commits during merge/rebase/cherry-pick/revert, conflicts and bare repositories. Hooks can change content; receipts link to the actual resulting commit and report errors without automatic retry or rollback. Preview reads disable external diff/textconv and never run hooks or filters. Stage execution may invoke configured clean filters; commit execution runs configured hooks/signing.

Operation receipts are persisted before and after execution under the application data directory. Subprocess failure/timeout is not proof that nothing changed. Reinspect and report the resulting HEAD; never blindly repeat a write. At the initial write milestone, hunk staging, gitlink writes, conflict resolution, stage-all, amend and push were excluded. Those exclusions are historical: current supported workflows and their narrower limits are documented in [implementation](../implementation.md#current-limits-and-next-work). Blind batch writes remain unsupported. Per-file safety budgeting can still refuse unusually large files/indexes.


## Remote branch and parent-switch foundation (historical scope)

The Tauri-independent Git adapter now exposes remote listing, explicitly selected fetch, immutable-OID preview and reviewed local fast-forward integration. Core records fetch separately from local integration and coordinates each through the shared operation guard. This foundation did not perform automatic network fetches or implicit merge/rebase/stash/reset/force-checkout/push. Current explicit write workflows extend it; optional connected CI can poll visible commit evidence, which is separate from Git fetch. The browser branch picker hosts both Local and Remotes scopes. Selecting a remote branch fetches it, then reviews the local create/switch/fast-forward result. A per-remote Fetch discovers uncached heads. Configured Git helpers and SSH remain available. The later macOS host-account connection and HTTPS credential bridge described above supplement this original authentication path; they do not require an account for local Git.

Parent switching permits reviewed submodule pointer-only changes without updating child checkouts. Index gitlink changes and retained child revisions are shown explicitly; dirty child content and submodule add/remove/type transitions remain blocked. Clean child checkout mismatches can be preserved during subsequent parent switches. See ADR 0005 for cursor/token boundaries, partial failure behavior and transport limits.


## Workflow clarity

The repository product rules reference `docs/ux/workflows.md` for every flow. Working changes reuses the existing right inspector and diff dock: persistent Unstaged/Staged groups and a stable composer, with code inspected below the graph. CommitDiffDock accepts either historical CommitDetail or a typed working/review patch source. WorkingDetails owns preview/token/review state; App routes that source only for the active membership and clears it on unmount. Summary/Description remain the existing per-repository message draft. Review selection is file-scoped and confirmation invalidates when the working snapshot changes. Backend mutation ports and IPC remain unchanged.


## Recorded submodule services and audit corrections

The submodule crate owns bounded recursive discovery and snapshot-bound operation preview/apply services, independent of Tauri. The Git adapter exposes native initialize/recorded-restore capabilities and scoped operation/index guards. Core assigns persistent discovered identities without implicit membership, coordinates selected/containing/child repositories, and persists targeted receipts before/after execution. Initialize uses explicit native checkout strategy and restricted transports; restore uses a local immutable OID with Git fetching disabled. Read subprocesses deny all Git transports even when per-protocol config would otherwise permit them.

Patch presentation retains raw byte path identity through C quoting and prevents unrelated-file fallback. Root App owns repository-scoped operation feedback; epoch checks reject pre-write background snapshots. Ref-unchanged graph refresh retains its cache; ref-changed refresh re-anchors inside the bounded loaded range, with an explicit notice if the old anchor is unavailable. See milestones 08/09 and ADR 0006 for limits and evidence.

## Ref lifecycle and push services

RefOperation/PushRequest previews and PlannedOperation receipts live in the domain, with native implementations in the Git adapter and registered-membership validation in core. Tauri and the browser bridge expose the same typed entry points. Local delete transactions retain the exact source object under an internal recovery ref before deleting with the expected OID. Internal recovery refs are omitted from normal graph refs/reachability roots. They remain reachable to Git GC and recoverable through the receipt.

Live push-destination reads use a dedicated standard-transport allowlist, process-group timeout and bounded output channels. Normal reads remain offline. Push applies one pinned source-to-destination refspec; no generic Git command port exists. Conditional cache updates use expected old OIDs, and optional upstream mutation is a later separately verified effect, so failure reports must not imply remote rollback. See ADR 0008 and milestone 11 for limits and coverage.

## Owned merge workflow

The Git adapter predicts merge trees in an isolated object store and owns per-worktree pending sessions. Core applies the same common-repository operation guard and durable receipts used by other reviewed writes. IPC exposes typed start/state/file/action DTOs; React reuses graph context, Working changes and the diff dock. Recovery refs retain both tips and bounded private snapshots retain edits before resolution/abort. See [ADR 0009](../adr/0009-owned-reviewed-merges.md).

## Selected change operations

Cherry-pick/revert extend the existing owned-operation request/state/receipt boundary, defaulting older records to merge. Scratch synthetic commits express the explicit delta base; native no-commit application, per-worktree preparation/readiness markers, shared conflict resolution and one-parent postconditions preserve the distinction from merging histories. Ordinary writes also reject clean pending markers. See [ADR 0010](../adr/0010-reviewed-cherry-pick-and-revert.md).

## Workspace commit search

The Tauri-independent search crate groups registered memberships by RepositoryId, persists immutable commit metadata once per shared repository and computes worktree-specific reachability for every query. Core validates active workspace scope; typed IPC returns repository-labelled hits, coverage and stale-aware cursors. One transient header search leads directly to the existing graph/inspector/diff through a read-only exact-position service. Fingerprints protect query and navigation consistency; unreachable or unsupported history never becomes a guessed selection. See [ADR 0011](../adr/0011-workspace-commit-search.md) for budgets, partial results, cache retention and remaining archaeology scope.

## English and Thai presentation

The frontend locale store and explicit en/th catalogs localize product text, accessibility labels and dates without changing domain DTOs, Git identity, messages, paths or code. The persisted global language preference is in **Settings → General**, alongside the theme preference; graph sessions and composer drafts stay mounted. **Settings → FAQ** contains the guide. The workspace selector manages repository/workspace navigation, not language settings. Unknown native diagnostics retain their technical text. See [ADR 0012](../adr/0012-localization-and-complete-history-files.md).

## Stash services

Typed stash list/request/preview DTOs extend the existing Git adapter, core operation guard and durable planned receipts. The Working changes inspector hosts saved work and existing diff comparisons. Immutable OIDs and original metadata snapshots bind entry removal; a scratch native rewrite and live files-backend locks preserve exact identity, with retained recovery data and explicit partial-failure reporting. See [ADR 0013](../adr/0013-reviewed-stash-workflows.md) for the restricted writer, supported backend and conflict limitations.

## Reviewed worktree creation

Graph context supplies the immutable revision; a typed creation request selects new/existing branch or detached HEAD and an explicit absent destination. The worktree crate delegates bounded native operations to the Git adapter, with core membership/operation guards and durable planned receipts. Source work stays in place; successful rediscovery verifies shared repository and distinct checkout identity before the existing related picker can open it. Stale branches/destinations/configuration invalidate review. Native hook/checkout failures preserve partial work and never trigger automatic deletion. See [ADR 0014](../adr/0014-reviewed-worktree-creation.md).

## Saved-work projection and large/interrupted operations

Stash DTOs include each entry’s actual first-parent base. The frontend projects bounded saved-work rows into the existing virtual graph without treating stash index/untracked commits as history roots. Per-repository polling, virtual scroll anchors and exact search offsets include this projection.

Merge content identity streams independently of text/recovery allocation budgets. Finish review carries a partial-code flag while retaining the full expected tree/path scope and native postconditions. Explicit interrupted-session release archives only a validated owned record after original-checkout/native-marker checks, through existing typed merge actions and durable receipts. See ADR 0015 and milestone 18.


## Direct saved-work interaction

The existing typed stash request/preview/receipt APIs serve contextual Stash/Pop beside Working changes. Launch intent identifies the active membership and exact/latest saved entry; native branch/HEAD revalidation continues to bind execution. Default working-content Pop and optional index restoration are distinct. The graph projection allocates saved-node lanes after the complete set of real Git slots and carries them to the correct base without changing commit topology. See ADR 0016 and milestone 19.


## Reviewed linked worktree removal

The existing related-worktree entry supplies the exact native path. The Git adapter validates shared Git identity, absence of local/ignored/hidden work, native locks, submodules and pending operations. A disposable zero-stat index hashes tracked content without trusting or modifying the real index. Core binds all affected workspace memberships and the retired WorktreeId into confirmation. Exclusive target HEAD/index/branch guards precede pinning the exact commit and native non-forced removal. Verified native completion then removes only reviewed memberships and retires the checkout ID; the shared RepositoryId, branch, source checkout and recovery ref remain. Catalog publication and native removal are separate effects, with explicit partial results and no automatic rollback/retry. See ADR 0017.


## Reviewed discard

Typed DiscardRequest/Preview extends core receipts and the Tauri-independent Git adapter. A read-only snapshot hashes selected raw content, staged blobs, HEAD/index, configuration, attributes and directory identity. Common operation plus native index/HEAD/branch guards protect confirmed writes. Private recovery contains original index, selected raw files/symlink targets, staged blobs, metadata and complete manifest; token hashing never expands raw bytes into JSON. Fixed native restore arguments operate through an isolated source-local index. Unstaged-only leaves the real index unchanged; all-content publishes the prepared selected-path index after revalidation. Partial outcomes retain recovery without automatic rollback/retry. Gitlinks are inspection-only here and reuse the separately reviewed submodule updater. See ADR 0018.

## Reviewed partial staging and native linear Rebase (M56)

The current adapter supports exact native-generated hunk application against a private index, with complete-patch and snapshot checks before atomic publication. Whole-file staging remains the explicit fallback for unsupported patch shapes. Rebase has a dedicated bounded linear plan and journal inside the shared operation envelope; native Git owns replay while the graph inspector owns reviewed conflict resolution, Continue/Skip/Abort and durable recovery. Foreign or inconsistent native state is never silently adopted.

Related metadata maintenance uses the same operation guards and durable receipts: selected initialized-submodule URL synchronization preserves checkout/pointers and saves configuration backups; worktree Lock/Unlock/Repair previews the native metadata scope. See [M56](../milestone-56.md) for implemented bounds and remaining lifecycle work. These extend the earlier initial-stage limitations recorded above.


## Bounded local history investigation (M72)

Typed investigation requests reuse registered workspace/repository/worktree source selection, fixed offline Git argument arrays and post-read ref fingerprints. The core aggregates per-location coverage and immutable branch-tip comparison evidence; no Tauri types enter the domain/Git services. Literal file/code history uses bounded recent candidates rather than a new persistent content index. The existing search result navigator opens the exact graph commit and matching historical file. This initial read-only slice does not infer semantic or dependency release readiness. See [M72](../milestone-72.md).
