# Workflow clarity across GitNova

Product direction, 2026-10-25: make every GitNova workflow clear, ordered and consistent. Apply this rule to future workflows, using original interactions and visual assets.

## Interaction contract

Every flow answers: Where am I? What is selected? What will change? What is the next action? What happened? Preserve the graph as the common context and show only relevant detail. Prefer stable positions and direct verbs to a new screen, wizard or large instructional introduction. A disabled action must have a nearby explanation. Empty groups convey state; do not remove a stage of the workflow because its count is zero.

Maintain one primary action per decision, with exact scope in its surrounding context. Keep content selection separate from mutation. Show pending state and prevent duplicate writes. Preserve drafts and selection when inspecting content or returning from a review. Treat errors and stale previews as states with a clear recovery action, never as success. A blocked prediction must not describe a computed outcome; a dirty-state blocker leads to the existing Working changes inspector. Historical file counts come from complete metadata rather than bounded patch text. Confirmation remains bound to reviewed Git state.

## Existing flows

| Flow | Context and sequence | Result and next action |
| --- | --- | --- |
| Workspace | Existing workspace picker → choose repository, or enter an existing repository path | Graph switches to that repository; cached context restores |
| Graph actions | Right click or keyboard menu on a commit/ref → exact action and preview → confirm | Checkout/ref creation updates the same graph; double click opens the unambiguous checkout preview |
| Commit search | Cmd/Ctrl+K → explicit current/selected/workspace scope → repository-labelled results → choose commit | Exact graph row and existing inspector/diff open without checkout; partial coverage and failed navigation remain explicit |
| Language | Header Settings → General → English / ไทย | Current UI changes in place; preference persists, repository data and drafts remain exact |
| History | Branch / Tag → colored parent relationships → message → selected commit | Inspector describes commit and comparison; bottom dock shows selected file |
| Working changes | Branch + change count → Unstaged / Staged groups → inspect selected file below → stage or unstage | File moves to the appropriate group; counts and next commit scope update |
| Merge | Graph source → predicted paths/conflicts → explicit start → resolve/review in Working changes → staged review and Finish | Two-parent commit or explicit fast-forward; native abort saves edited/staged work to its reviewed recovery location |
| Cherry-pick / revert | Graph commit → selected parent for merges → preview delta/inverse → start → resolve and staged review → finish | New single-parent commit; original authorship retained for cherry-pick, history preserved for revert; reviewed saved-edit abort |
| Stash | Stash/Pop beside Working changes or select a saved graph row → current branch and file scope → confirm | Saved work with retained recovery; Apply keeps entry, Pop removes only after success, Drop preserves a recovery ref; conflicts remain for Git resolution |
| Commit | Staged group → summary and optional description → staged diff review → explicit commit | Receipt and new graph commit; unstaged work remains outside the commit |
| Local branch | Choose local branch → source/target and affected paths/submodules → confirm switch | Branch context and graph update; protected work remains intact |
| Remote branch | Choose remote → fetch latest → local create/fast-forward preview → confirm | Local tracking branch and graph update; divergence explains why integration stopped |
| Related repository | Existing repository picker → inspect submodule/worktree identity and state → open | Graph opens that child/worktree without conflating it with a workspace group |
| Recorded submodule | Related item → Initialize or Restore recorded → compare parent INDEX and checkout, local work and source → confirm | Child reaches the reviewed revision; parent index stays intact and related state refreshes |

## Working-change layout

The existing workspace picker lists at most five repositories per page, with search across registered repository names and local paths in the current workspace. The graph only represents the current repository, so repository discovery/navigation belongs in this existing picker rather than another graph panel. Previous/next and clear-search controls are necessary for this requested bounded list, not new global navigation. Filter first, then paginate; pinned entries retain priority and recency order is stable. Open the selected repository's page by default, reset to the first page on a query change, and preserve context when changing language. Show paths so equal folder names remain distinguishable. Read Git status only for visible entries, using bounded workers; search/pagination does not scan the filesystem or load every repository's status. Existing Add/Clone and graph selection flows remain in place.

Use the existing right inspector for the two file groups and composer, and the existing bottom diff dock for code. Both Unstaged and Staged headings remain visible with counts and explicit inclusion semantics. Each group scrolls independently so a long unstaged list does not bury staged files. The composer stays after these groups in a stable footer, including during review. Remove marketing copy and inline code beneath the composer, which previously forced users to scroll past the next action to inspect evidence.

Selecting a file changes the dock, not commit membership. Stage/unstage acts on a named file with a validated preview token. A file with both staged and unstaged content appears in both groups; its scope is explicit. Summary/description remain a per-repository draft. Review shows the actual staged patch in the same dock; staged-file selection can inspect each included file without leaving review. Changes to the reviewed state invalidate confirmation. Bulk stage/amend were outside that initial layout refinement; reviewed discard is specified below. Owned pending operations now separate staged and unstaged files and offer reviewed per-file/all-file unstage in the same inspector (ADR 0021). This index-only action keeps working files and the operation pending.

## Review criteria

At narrow desktop sizes and with long file lists, both groups and the next action remain discoverable. File selection, stage, unstage, review, return to edit and commit form a coherent sequence. Switching repositories preserves drafts and cannot display the previous repository's diff. Existing graph, branch/remote and fixed split-diff behavior must keep working. Verify with actual temporary Git repositories; screenshots alone cannot prove behavior.

Graph action coverage, remaining important operations and double-click ambiguity rules are maintained in [graph-actions.md](graph-actions.md).

## Linked worktree creation

Right click the exact graph commit → Create worktree here → choose new/existing branch or detached HEAD and an absent absolute destination → Review → Create. Show source, canonical destination, exact revision, target branch, complete path count and uninitialized submodule pointers. Occupied branches, existing destinations and stale reviews cannot confirm. Creation preserves source selection/drafts; use the existing Related repositories item to open the result. A failed native checkout may retain files/branch/metadata and must not suggest that automatic rollback occurred.

## Saved work and interrupted operations

Saved stashes are typed rows in the existing graph, anchored to their recorded base; they open the existing stash inspector directly and never act as ordinary commit checkout targets. Unknown/unloaded bases and bounded coverage remain visible. Search/scroll account for saved rows without indexing stash implementation parents.

Merge method is selected inside the existing graph action preview, without another menu item or panel. The Merge action defaults to Create merge commit: even when the source descends directly from HEAD, explicit start prepares `--no-ff --no-commit`, and reviewed finish records both parents. Fast-forward only is a separate explicit choice in that preview; it moves the branch without a new commit and refuses divergence. Changing the choice invalidates the old preview. Both are valid Git operations and the graph renders the resulting actual parents. Existing fast-forward history remains linear; do not invent a junction or rewrite history to make a visual match.

The context status distinguishes an operation pending/incomplete from a clean working tree. A blocked second start returns to the existing operation. Incomplete owned sessions reuse the abort-review position for explicit session cleanup when native Git is no longer active and original branch/HEAD match. Archive only the app record; retain files, staging and recovery tips. Show the preserved Working changes afterward. Large merge code previews can be partial while the full staged tree remains explicit and immutable; replacement/abort recovery limits remain separate.


## Direct Stash/Pop

Use the two current-work actions beside Working changes. Stash opens a compact scope review; Pop directly reviews the latest saved entry on the current branch. An older graph stash opens its own Pop review. No Apply/Pop/Drop mode selection is required for the normal path. Keep the destination branch prominent and place index/recovery choices under optional details. Completion returns to the actual working files; preserve drafts and conflict guidance. Stash glyphs reserve lanes beyond every real Git lane, including incoming/outgoing strokes. See ADR 0016.

Zero changed files does not mean an active merge has ended. After discarding a merge result, Pop remains protected until the user explicitly finishes or reviews/confirms abort. The stash inspector's existing recovery action names the owned pending operation and leads to its current Working changes inspector. After a known Discard result, an empty pending operation emphasizes existing abort review; a legitimate no-ff merge with zero net file changes instead keeps reviewed Finish primary because it still records both parents. Do not automatically abort, delete Git markers, close an active session or replay Pop. Revalidate open stash previews on changes to the live checkout/operation context, retaining the selected file and saved comparison; only an explicit new confirmation writes to the current destination.


## Linked worktree removal

Workspace picker → Related repositories → exact worktree → Remove worktree… → folder, branch/commit and affected workspace entries → Remove checkout. The current/main checkout cannot be removed. Dirty or ignored files, hidden index flags, locks, submodules and pending operations stop the workflow with a reason. Cancel keeps everything. Success removes that clean folder and its reviewed workspace entries, preserves its branch/commit and leaves the source graph and drafts open. Partial native/catalog outcomes retain the plan and recovery ref and require inspection before any new review.


## Discard selected changes

Working changes trash → explicit branch, selected paths and Unstaged only / Staged and unstaged → inspect files in the same dock → confirm the matching reviewed scope. Right click or Shift+F10 on a file chooses a single-path review. Cancel retains the composer draft; completion returns to actual Working changes and preserves the draft, HEAD and unrelated files. Original raw content, selected staged blobs and index are saved privately before writes. Untracked selected files are removed explicitly; ignored files and nested repositories are not swept by clean. Stale scope/content cannot confirm. Partial failure retains recovery and leads to inspection rather than retry.

Submodule changes show a real gitlink diff. Reuse Restore recorded version inline for checked-out-vs-recorded commits; dirty child content must be reviewed in the child graph/Working changes. Discard never recursively resets child content.


## Submodule pointer versus child files

Working changes identifies a gitlink as a Submodule item. Its existing inspector distinguishes parent commit, parent index and child checkout, with separate commit-changed, pointer-staged and uncommitted-child states. Open submodule leads to its real graph/Working changes; double-clicking the row has the same navigation effect. Commit/discard files inside that child, then stage the resulting commit pointer in the parent when ready. Parent file/item counts do not imply a count of files inside children.

Stage/Unstage pointer uses the same submodule review surface and existing row context menu. Review shows the exact parent-index SHA transition and that child files remain outside the parent commit. When only child files are dirty and its commit equals the recorded pointer, the selected row action opens the child rather than offering an ineffective file-stage operation. Pointer modifications can be reviewed and committed in the parent while child edits remain untouched. Generic submodule add/remove/type-change commit blockers remain. Restore recorded is a separate child-checkout review with dirty/unpushed-work protections; it never means discard all child files.

## Submodules in branch context

The graph's collapsible Submodules group lists clean and changed submodules for
the active parent checkout. Its heading names that branch. Click a module name
to open its graph; Manage or right click reveals the existing reviewed pointer
and recorded-checkout actions. Parent commit, parent index and child checkout
remain separate. Switching the parent branch refreshes this list without moving
the child checkout. See ADR 0029 for read lifecycle and bounded layout.

## Diverged Pull

After fetching, a divergent current branch offers Review merge in the same Pull
review. Carry the fetched remote OID/ref and the reviewed local branch/OID into
the existing merge preview, then explicit Start → staged/conflict review → Finish.
A context change stops confirmation. Native Pull remains fast-forward only; no
automatic merge/rebase/reset/stash is performed. Hide uncomputed path counts on
blocked previews throughout Pull, remote branch selection and graph checkout.

## Tags and provider requests

Right click a commit → Create tag → exact revision/type/message review → Create.
Push tag is available in that result and on the tag's graph menu; publish one
immutable tag object after live destination review. Existing branch Push excludes
tags. Right click a local branch → Create/open PR or MR → remote/host/account →
source/target/title/body/Draft → review → explicit Create or Open existing. Missing
publication hands off to reviewed branch Push and preserves the request draft on
return. Provider result details stay in this existing graph review. See milestones
37/38 for current verification and follow-ups.

### Fetch placement

Fetch remains in the existing branch picker under Remotes, beside each remote.
The duplicate toolbar Fetch beside Pull is removed. Both paths used the same
fetch operation; the picker makes its remote scope explicit without a second
entry point. Pull still fetches before reviewing integration into the current
branch. Local Refresh only rereads local state.

Auto fetch is a global preference in existing Settings → General, defaulting to one minute with Off/30 seconds/1/2/5 minutes choices. This persistent policy does not belong in a commit action; no new panel, toolbar item or duplicate Fetch button is introduced. Only the active checkout’s upstream remote, otherwise origin or its sole remote, is fetched. Ambiguous remotes without that fallback and bare repositories are skipped. Scheduling waits until the interval elapses, pauses new starts while hidden/offline/in an action review, and defers after foreground writes. A small existing-context status shows running/completed/paused state, with details on hover rather than error dialogs. Graph refresh preserves selection and scroll. Native operation locks and pending-operation checks independently protect the transport.

Failures increase the delay exponentially up to fifteen minutes. Background attempts are bounded to twenty seconds and do not start another attempt until completion. Already-running requests may finish after the window is hidden or the setting is disabled. Fetch does not pull, prune, fetch tags/submodules, write FETCH_HEAD, run local hooks or create per-minute recovery receipts. Authorize a saved HTTPS account or SSH host/key manually first; background work never invokes interactive credential fallback. Custom core.sshCommand wrappers are reserved for foreground Git; background SSH uses standard ssh with the user’s SSH config/agent, BatchMode and strict host verification. Failed fetches may have updated some remote refs; no atomic-fetch guarantee or rollback is claimed.

### Branch deletion from the graph

Right-click the exact local/remote branch badge → Delete local branch… or Delete
remote branch… → review the branch, commit and recovery → confirm. Local deletion
reuses the existing ref review directly, removing the hidden Manage step. Remote
deletion uses the same graph action surface and explicitly names one live push
destination. It never also deletes a local branch. Require a matching fetched
commit, preserve a local recovery ref, refuse the advertised default branch and
use an exact expected remote OID for server-side compare-and-delete. Keep cached
tracking refs untouched when fetch/push destinations differ; custom fetch mappings
require separate inspection and block deletion here. Unknown
network outcomes require inspection before retry; server protections remain in
force. No combined/batch deletion or new permanent navigation is introduced.

### Hosted request direction

The existing PR/MR graph review shows Source → Target above its form, review and
result. Source and target branch fields are visible before account connection;
connection must not hide the request's direction. The checked-out branch is
labelled separately and never presented as an inferred target. An empty target
remains the provider default, explicitly unresolved until Review returns its
actual name. Keep the user's chosen target and project across connection and
Push handoff. This rearranges the existing form without new navigation.

### Quiet submodule refresh

Keep submodule rows and count in place during background reads. Show the initial
loading line only before any successful snapshot; explicit Refresh uses the
existing heading icon, without inserting a new row. Background discovery runs
at most once per minute, pauses when hidden/collapsed/managing/writing and
coalesces focus/visibility events within 30 seconds. A parent graph reload (such
as explicit Refresh or an app write) still revalidates immediately; unrelated
working-file token changes no longer trigger recursive discovery. Branch/repo/HEAD
changes remount the context and discard late responses. Unchanged snapshots keep
the existing row state; failures retain and label the last known state. All
mutation previews continue to validate live Git state.

Quiet-refresh verification: 3 real-Git browser scenarios passed (branch context,
late response isolation, and timed background/manual/failure behavior), plus 26
frontend checks. The timer test confirms the former 15-second read is absent,
background reads keep section height stable, focus events are coalesced and
manual recovery detects real edits inside the child. The local macOS bundle at
`target/debug/bundle/macos/GitNova.app` passed TypeScript/native build and
signature/embedded-asset verification.

### Repository toolbar placement

Workspace/repository switching and universal search stay in the global header.
Move the existing Branch, Pull and Push controls beside Repository history and
the selected repository name: they act on this checkout, not the whole workspace.
Retain their current callbacks, state guards and preview flows. Keep operation
status to the right on wide windows and on a stable second row on narrow windows.
No duplicated actions or new navigation are introduced. Increase only the header
logo footprint to 44px (40px on narrow windows); keep the native icon artwork.
Footer credit becomes GitNova · by edocstudio, with the alpha badge still marking
the app's release stage in the header.

### Reviewed Amend

Use Amend last commit in the existing composer. Preserve the ordinary commit
draft in a separate state and prefill the original message without reading its
whole historical patch. The selected HEAD, preserved author/parents, staged delta
and recovery ref appear in the same review and diff dock. Message-only amendments
are explicit; referenced tag/cached-remote history requires acknowledgment before
confirmation. A changed HEAD requires selecting Amend again. Keep confirmation
visible in a scrollable composer at small window sizes. No new screen or global
button is introduced; no automatic push or reset follows the replacement.

### Global CI connections and shareable commit links

Header Settings → Connections → company/host profile. Add GitLab or GitHub HTTPS origin and connect the saved account; optionally attach a Jenkins controller and username to that company. No repository, branch, SHA or job is required to connect. These are global account scopes, shared by matching fetch remotes across all workspaces. Keychain holds tokens; the Settings list holds only nonsecret host/account mapping metadata. A global Disconnect confirms all-repository and shared PR/MR effects. Removing the automatic host mapping retains the saved credential, with that consequence explicit.

The graph CI column sits between Commit message and Author. Only visible commits are read, with exact SHA and native fetch-host validation, bounded caches and quiet polling that pauses while hidden. GitLab supplies the exact commit’s latest pipeline status and number when available, with commit-level status as a fallback. Hover shows its branch and last-check time; the existing commit context menu opens the validated pipeline URL. Pipeline data from a different SHA is rejected, even if displayed near that commit in a merge request. GitHub uses combined commit statuses plus latest checks and a bounded exact-SHA Actions workflow list. One matching workflow adds its display run number; multiple workflows, or a workflow whose state differs from the aggregate, use a count and list individual names, states and refs on hover. Context-menu links open each validated workflow run. Check evidence remains available with an incomplete marker when Actions metadata cannot be read; missing metadata cannot turn an otherwise unknown result green. Unknown, inaccessible and incomplete checks cannot become a false success. A late response cannot paint another repository. Settings does not load commit-specific jobs or list an active repository path. The selected commit inspector stays focused on commit evidence.

Jenkins connection verifies the controller/account separately from Git credentials. The graph includes Jenkins when it reports CI results to GitLab/GitHub. Controller-wide automatic job discovery is not implied by connecting an account, and the UI states this limitation. Legacy explicit job mappings remain stored; the native bounded/exact-revision Jenkins build adapter stays available internally and tested. No job/branch is silently inferred from a Git hostname and no build is triggered.

Copy commit link sits beside Copy SHA because a hash alone cannot open a hosted commit for another team. A single recognized remote copies directly; multiple remotes require selecting the intended project. Use the full selected SHA and a credential-free HTTPS URL without fetching, pushing or changing checkout. Successful copying does not claim an unpublished commit is accessible. If clipboard access fails, retain a selectable URL. Remote selection does not change Git's configured upstream.

### Merge and an already-correct submodule

Compare current/incoming/predicted child revisions in the existing merge review.
A tip difference is not a merge result. If prediction keeps the current pointer,
allow normal reviewed merge of the feature's other files without touching the
child. If the source is already included, preserve that reason and label the
existing disabled action Already included. Do not replace it with a gitlink
warning. Actual changed gitlinks or submodule conflicts remain blocked with exact
path/revision evidence and an honest app-limit explanation. Opening a child or
restoring an older pointer is not a prerequisite to bypass that limitation.

### Settings ownership

The upper-right header gear and Cmd/Ctrl+, open one Settings dialog. Persistent application, workspace and repository preferences belong there with explicit scope. Language and CI account connections are global. CI mappings identify the company/Git host and optional Jenkins controller, while credentials remain in Keychain; they do not identify the active repository or commit. The graph CI column continues independently after the dialog closes. Commit details contain commit evidence and actions, not connection setup. Keep Git action source/target, previews and confirmations in their existing contextual workflows. Settings must preserve drafts, selected commit, graph position and underlying action reviews when changing language; background keyboard/outside-click handlers yield to the native dialog.

## Cherry-pick/revert parent baseline and unrelated submodules

For a selected merge commit, retain explicit parent selection and show `selected parent → merge commit` beside the affected files. Changing this selection clears the old preview and disables Start until the new preview arrives. Do not guess a mainline parent.

A clean child checkout differing from the parent gitlink is distinct from uncommitted child files. For an unrelated cherry-pick/revert, show the recorded and checked-out commits in the existing action review and preserve both. Do not stage or update that child. Verify the preserved state throughout Start/Finish/Abort; actual submodule/layout changes retain their separately reviewed workflow.

## Optional source branch cleanup after merge

Offer a named local source branch checkbox, initially unchecked, in the existing graph merge review. Repeat the selected deletion in the finish confirmation; retain it across a pending session restart. Bind toggles to a new preview. Local cleanup follows verified merge success, including fast-forward, and retains the source tip for recovery. Abort, conflict and failed merges keep the source branch. A changed source or a source used in another worktree stays intact; cleanup failure is reported separately from merge success. Remote deletion remains a separately selected graph action.


## Initialize a local repository

Use the existing workspace picker alongside Add existing and Clone. Initialization has no commit graph yet, so the picker is the natural entry point; no new screen, navigation or permanent toolbar is needed. A collapsed Initialize repository form takes the full folder path and initial branch (main by default). Review displays the canonical destination and whether it creates a folder or initializes an existing project. Confirmation creates Git metadata, registers the repository in that workspace and opens its empty graph. Existing files remain unstaged; there is no initial commit, remote or network request.

The parent must exist. Existing repositories (including bare repositories, gitfiles and destinations inside a repository), files and destination symlinks are rejected. Initial branch names use Git ref validation. Execution rechecks the reviewed destination identity and exclusively reserves .git; it never reinitializes existing metadata. Empty templates prevent installation of user template files/hooks. Failure retains partial metadata/files for inspection, and registration failure directs the user to Add existing. Initialization is local only; remote setup remains in Push. Ignore/license templates, bare initialization and explicitly nested repository initialization are not provided by this form.


## Resize graph columns

Resize the existing Branch / Tag, Graph, Commit message, CI, Author and Date tracks directly from their header dividers. Column layout is a graph interaction, so no separate screen, toolbar button or configuration form is introduced. Drag a divider, use Left/Right (Shift for larger steps), or double-click/Home to reset that column. Header and all virtualized commit/stash rows use one grid definition and synchronized horizontal scrolling, including keyboard focus scrolling. Selection and vertical graph context are preserved.

Widths are device-local display preferences shared across repositories, persisted after a completed drag or keyboard adjustment. Defaults remain automatic until changed; each column can be reset independently. Malformed/unavailable storage does not prevent using the graph. Pointer movement is coalesced to animation frames; no Git read/write or network operation is requested by resizing. A compact initial graph width and adaptable lane spacing avoid reserving excessive blank graph space. Minimum graph width grows with the loaded topology so every lane and stash connector remains visible and node spacing stays readable. Narrow windows retain horizontal table scrolling instead of dropping columns.
