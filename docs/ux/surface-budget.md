# UI surface budget

Every new surface must state why an existing graph context cannot deliver it naturally.

| Surface | Why it is necessary | Reused context |
| --- | --- | --- |
| Top-bar workspace/repository selector | Selecting another repository changes the graph's data source; no commit selection can express that | One menu contains workspace switch/create, repository list, branch/dirty status, pin and add-existing path |
| Empty graph path entry | There is no graph context before the first repository is selected | Same add-existing control as selector; no dashboard |
| Graph rows and ref labels | Primary task is reading history | Native button semantics, arrow/Home/End navigation and visible focus |
| Commit details and diff | Selected commit has more data than fits in its row | One collapsible contextual area, no separate page |
| Load-more row and refresh | Pagination and external Git changes need explicit controls | Graph footer and top bar; no navigation item |

Do not add permanent workspace sidebars, status dashboards, feature cards or modal stacks. Search is deferred until real search exists; no fake search button. Keyboard: Cmd/Ctrl+O focuses repository selection; Escape closes the selector; arrow keys navigate rendered graph rows. Graph selection and scroll restore per membership.

## Everyday Git and visual refinement

Use blue-black surfaces, legible 12–14 px interface text, blue selection accents, clear pane hierarchy and original SVG icons. Avoid tiny all-caps labels and decorative status noise. Group patches by file with old/new line numbers; preserve patch truncation warnings.

| Surface | Existing graph context cannot express | Reused context |
| --- | --- | --- |
| Working changes row above history | Uncommitted files are not commits and have no OID | A single graph row opens the existing detail area |
| Stage/unstage file action | Reviewing a file alone cannot express an index write | One contextual action beside the selected file, acting only on that path |
| Commit message and staged review action | A new commit needs user-authored intent and explicit staged-content approval | Existing working-changes detail, followed by its staged patch preview and explicit Commit action; no modal |
| Return from commit review | Editing the message or selected staged files changes the review context | Inline Back action in the same area |

Check cached history in the background on focus and periodically while visible. Keep selection when only worktree state changes. Revalidate file/index/HEAD snapshots before writes; show operation outcomes and recovery inline. This initial slice did not expose discard/reset; the later reviewed-discard scope is described below.

## Reference-led graph and diff layout

Use the supplied draft as a visual reference: blue-black surfaces, a clear blue active row, stronger colored graph lanes and a legible description/author/date hierarchy. Reuse the existing repository selector; no second navigation sidebar, inactive search box or unsupported intelligent-action cards.

| Surface adjustment | Why the previous context was insufficient | Existing capability reused |
| --- | --- | --- |
| Bottom diff dock | The narrow metadata inspector cannot provide readable old/new code comparison while keeping graph and commit context visible | Move the existing commit patch out of the right inspector; do not duplicate it |
| Changed-file list inside the dock | Stacked long patches require repeated scrolling to locate another file | Select one file from the same commit's existing patch records |
| Side-by-side / unified control | Old and new line alignment is a rendering choice within the selected patch | Reuse parsed Git patch lines; no extra backend or navigation |
| Collapse / restore controls and resize separator | Users need to allocate graph/code space without losing selection | Existing graph context toolbar and pane headers; keyboard-accessible resize |

Metadata remains on the right. The dock exists only for an actual selected commit and can collapse to its header. Working changes keeps its existing reviewed-write workflow. No new Git operation is introduced by this layout change.

The inspector's existing commit identifier gains Copy SHA in that same context. This is a local clipboard action on the exact selected OID, with visible success/fallback feedback. The changed-file count itself restores/focuses the existing diff dock; it does not open a new page.

The diff header keeps its own collapse/expand control. The graph toolbar shows a metadata restore action only while the inspector is hidden, avoiding duplicate permanent hide/show buttons.

## Branch switching and fixed comparison panes

The existing current-branch display becomes the branch selector; there is no new navigation entry. Its local-branch filter and inline affected-path preview share that popover. Confirm explicitly in the same context. Dirty, occupied, in-progress or unsupported submodule cases explain why switching is blocked; no force/discard/automatic stash action is exposed.

Before/After headings remain fixed above two equal-width panes. Long code scrolls inside its own pane while vertical scrolling stays synchronized. The layout does not widen the entire comparison or move its center divider.

## Parallel graph refinement

Before adding a screen or control, the existing graph was assessed: branch labels mixed into messages and changing lane positions caused the readability problem. Moving those labels into a Branch / Tag column inside the same graph solves it without a sidebar, dashboard, dialog or additional navigation. Compact rows and stable colored lanes require no extra interactions. Reference tooltips and the existing selected-commit inspector expose names that do not fit in a row.

## Remote branches and submodule-aware parent switches

The existing branch picker naturally owns local/remote branch navigation. Local / Remotes scopes, grouped remote refs, fetch progress, integration preview and submodule pointer details all fit there. A separate sidebar, dashboard, dialog or navigation route is unnecessary. A per-remote Fetch control is justified only to discover heads that have not yet been fetched and therefore cannot appear in the current graph or cached branch list. Selecting a known remote branch performs that scoped fetch directly before its existing confirmation area shows the local effect. Local creation and fast-forward pull share one confirmation; no separate tracking-configuration screen is needed.

## Workflow sequence and working changes

The existing working inspector contained an introduction, conditional file groups, composer and a second inline diff. This put evidence below the action and hid empty stages. Reuse the right inspector for two persistent file groups and the composer; reuse the bottom graph diff dock for the selected working/staged/review patch. The right inspector spans the existing graph/diff height while the code dock occupies the graph column, so resizing code does not move the composer. No new screen, sidebar, panel or dialog is necessary.

Move stage/unstage actions from the old inline preview header to their file rows in the existing inspector. The row gives exact scope and enables one action after inspection without searching below the composer. Do not duplicate these controls in the dock. Separate Summary and optional Description within the existing composer, with one primary review/commit action at its stable lower edge. A noninteractive Changes → Staged → Commit indicator conveys state without creating a wizard or navigation surface. Both empty groups remain visible and explain the next step.

The broader interaction contract is maintained in `workflows.md` and referenced by the repository's AGENTS.md. Current graph, workspace, local branch, remote and related-repository flows were checked against its context/scope/action/result sequence; their existing surfaces already host these steps and do not need new permanent navigation.

## Recorded submodule operations and durable feedback

The related-repository item already identifies the affected child and its parent context. It naturally hosts Initialize or Restore recorded preview and confirmation, so no standalone submodule screen, sidebar, modal or dashboard is needed. Missing modules cannot be opened in the graph until initialized; their contextual initialize action fills that existing gap. The item shows recorded-versus-checkout state; mutation review adds source, retaining branches and the exact confirmed target. Nested items reuse this list with explicit containing-path context.

Operation failures reuse the existing repository error surface after navigation, rather than adding an activity panel. Successful context feedback identifies the affected path. Graph refresh re-anchors in place and uses the existing notice style when the old position cannot be recovered inside the loaded range. These changes preserve the user's graph context while keeping engine-level guards out of the normal flow.

## Graph context actions and double click

The selected commit and its refs already identify the action scope. Reuse that graph position for a compact contextual action menu and inline preview; no toolbar, sidebar or permanent action panel is necessary. Right click, the keyboard Context Menu key and Shift+F10 open the same menu. Double click on a graph node selects its unique local branch, otherwise its unique remote branch, otherwise detached commit checkout; ambiguous refs open the choice menu. Double clicking a ref targets that exact ref. Never guess between multiple branches or reset a branch to the selected commit. Checkout and ref creation still require reviewed confirmation.

The menu exposes implemented actions with scope-specific names: inspect, local checkout, remote fetch/open/pull, detached checkout, create-and-open branch, lightweight tag and copy identities. Avoid unavailable placeholder actions. The later ref-lifecycle increment below adds validated branch/tag management, upstream and single-branch push. Rebase, reset/revert/cherry-pick, history rewriting, patch export and hosted-provider actions still need their own validated backends and recovery flows before appearing. Track their coverage explicitly in the graph-actions roadmap.

## Ref lifecycle, upstream and publication

The existing graph context menu owns branch/tag identity and is the natural place for rename, delete, tracking and push. Extend that menu with a per-ref "Manage" choice, reusing its inline form/review surface; do not add a remote dashboard, modal or permanent toolbar. The selected local branch remains explicit even when another branch is checked out. Destination selection and outgoing commits stay in this same context. Deletion previews disclose a durable recovery ref; push review distinguishes first publication, fast-forward, cached-tracking updates and optional upstream configuration. Network inspection starts only after an explicit "Check destination & review" action, and never runs pre-push hooks.

After a successful local deletion, the existing repository status receipt carries a transient Copy recovery ref control. This exposes the already-created recovery pointer without adding an activity/recovery panel or permanent navigation. It is scoped to that repository's operation feedback.

## Merge and conflict resolution

Merge starts from a selected commit/ref in the existing graph action popup. Source, current destination, predicted paths/conflicts and fast-forward versus merge-commit behavior fit its existing review area. Once started, use the existing Working changes inspector for conflict/ready file groups and finish/abort actions, and the existing diff dock for comparisons and an optional editable result. Do not add a merge dashboard, wizard, second diff panel or conflict sidebar. Abort and per-file replacement previews disclose filesystem recovery snapshots before replacing merge edits. Expected pending/conflicted state is a workflow state, not a generic error banner.

Merge result feedback reuses the existing recovery-copy control for the recovery ref or snapshot directory; no recovery browser or extra navigation is introduced. Conflict editing occupies the existing bottom diff dock with fixed equal columns, and resolution review replaces the editor in that same space.

## Cherry-pick and revert

The graph commit already supplies source identity and the current branch supplies destination. Extend its existing context menu/review with Cherry-pick and Revert; no additional screen, sidebar or permanent navigation is needed. Mainline selection for merge commits belongs in that same preview and explicitly identifies the selected parent. Revert explains inverse changes and merge-history consequences. Reuse the owned-operation Working changes inspector, conflict editor and staged review. Labels identify the actual operation and single-parent commit; cherry-pick retains original authorship. Abort previews saved edits and restoration of the reviewed original checkout. No multi-commit range, automatic commit, implicit stash or history rewrite is added.

## Workspace commit search and exact graph navigation

The current graph already hosts commit inspection and all relevant actions, so search results must lead back to that graph rather than create a search screen or dashboard. A transient search field/popover beside the repository context is necessary to enter a query and choose Current repository, Selected repositories or Entire workspace: scrolling the currently loaded graph cannot naturally query other repositories or unloaded history. Reuse the existing topbar and graph/detail/diff surfaces; Cmd/Ctrl+K opens the same search. Results identify repository/worktree locations and commit identity; selecting a result switches the existing repository context and opens the exact graph range without checkout. Scope selection uses registered memberships only. Show incomplete indexing, traversal budgets, unavailable repositories and stale pagination explicitly; do not turn a partial scan into a definitive No results claim. Commit message/author/email/SHA search is the first slice; file/content/blame search stays on the roadmap until real backends exist.

## Locale and blocked-operation recovery

Language does not belong to any commit/ref action. Reuse the existing workspace picker for one labelled English/Thai preference, persisted locally; no settings screen or permanent toolbar control. Language changes update the current UI without remounting repository sessions or changing drafts, Git names, messages, paths or code.

A blocked operation cannot predict its result yet. Replace its existing Back action with Review working changes when uncommitted work is the blocker; navigate to the existing inspector/diff instead of adding a warning dialog. Historical file lists must use complete bounded metadata independently of patch text; selecting a file reuses the existing diff dock and loads only that immutable comparison.

## Stash within Working changes

Stash captures uncommitted index/working content, so Working changes already provides its natural context. Add one Stashes mode control in that inspector; reuse its file area for the list/review and its existing diff dock for saved working/index comparisons. No top-level Stashes navigation, new sidebar, screen or modal. The same control remains available when the working tree is clean so saved work can be restored. Push, Apply, Pop and Drop identify the exact shared-repository stash and require an explicit preview. Pop retains the entry on conflict or incomplete application; Drop discloses its recovery ref/location. Language and draft/context preservation follow the existing English/Thai contract.

## Linked worktree creation

A selected graph commit supplies the exact source revision and local branches. One Create worktree here action extends that same contextual menu and review, with explicit branch mode and new destination; it cannot be replaced by checkout because checkout would move the current working context. Open the result through the existing Related repositories list, preserving the source and composer until the user explicitly switches. No worktree screen, automatic switch or parallel navigation. See ADR 0014.

## Saved work in graph and session closure

Saved work belongs beside its base revision as a distinct row in the existing graph. Selection goes directly to the existing stash inspector, rather than a second stash navigation surface. An interrupted session reuses the existing abort-review control as a reviewed metadata-only closure, with the same inspector/dock and preserved working state. Blocked starts reuse Back as Review current operation. See ADR 0015; no new sidebar, screen or permanent toolbar control is introduced.


## Direct current-work actions

Replace the nested Stashes entry in the working inspector with Stash and Pop beside the existing graph's Working changes row. Current working scope is independent of commit selection, so these two contextual actions are needed to save current work or restore the latest entry without hunting for a saved row. Older entries already live in the graph. Reuse the existing inspector/diff for one compact review; remove mandatory mode selection and put uncommon options/recovery in a collapsed section. No separate toolbar, stash screen, sidebar or wizard. A Pop preview error that specifically concerns removable metadata can offer the supported Apply-and-retain review in that same error context. See ADR 0016.


## Linked worktree removal

Reuse the existing Related repositories worktree row for Remove worktree… and an inline folder/branch/commit/membership review. A commit alone cannot identify which checkout folder to remove. Keep Open as the row's normal action; use explicit Remove checkout confirmation and a nearby reason when unsafe. Recovery metadata stays collapsed. Source graph selection and commit drafts remain unchanged. No screen, permanent panel, dashboard, automatic prune or force option. See ADR 0017.


## Reviewed discard and working submodule context

One trash action in the existing Working changes heading reviews the explicit changed-file set; a file context menu exposes Discard file… without permanently multiplying row buttons. The existing inspector replaces the composer temporarily with branch, scope, checkboxes and confirmation, while the bottom dock continues inspecting actual files. Content selection and discard remain distinct. Changing the selected set/scope disables confirmation immediately until the matching preview completes; an older preview never authorizes a new visible scope. No new screen/sidebar/dialog is needed. The existing SubmoduleEntry recorded-restore review is reused beside a changed gitlink, so a directory failure becomes pointer evidence and a relevant action. See ADR 0018.

## Initial project publication

The existing per-file + control requires hundreds of interactions for a new project. Add one Stage all action in the Unstaged group heading, with the selected file list and exclusions reviewed in the existing Working changes composer. No additional screen/dialog/navigation is justified. Submodule pointers remain separately reviewed.

Large commits need a complete staged manifest plus bounded, token-bound per-file inspection in the existing diff dock, rather than loading an unbounded combined patch. Explicit commit confirmation still selects the full reviewed staged index.

Remote URLs are persistent repository configuration and belong in the existing Settings dialog, scoped to the active repository and shared linked worktrees. The empty Push state may link to that existing Settings section; it must not grow its own settings form. Adding a remote configures a URL only, without creating a hosted repository, contacting its server, changing credentials or publishing commits.

## Conflict resolution clarity (M51)

The existing merge/cherry-pick/revert inspector already offers whole-file choices and Edit result, but its Current-to-working diff hides the incoming conflict blob. Reuse the bottom dock for Current | Result | Incoming from actual Git stages 2 and 3. The initial result is read-only; Edit result enables the middle draft. Retain existing whole-file version review and apply actions. No new screen, sidebar, dialog or navigation item is needed.

Within the editor, Previous/Next conflict and Accept current/incoming/both block controls are necessary to resolve individual conflicts without replacing the whole file. They alter only the in-memory draft. Each accepted block preserves surrounding content; diff3 base text is excluded. Explicit Review edited result and Apply resolution remain the only save/stage path. Binary, symlink, gitlink and oversized content keep their supported exact-version or external inspection flow.

Stage all visibility refinement: keep the existing action at the right of the Unstaged heading, use a distinct staging color and disable it when empty. Remove the redundant unstaged-scope caption while the action is shown so the button fits narrow inspectors without another control or surface.

## Settings consistency and remote context

Reuse the existing Connections card/add-form pattern for repository remotes inside Settings; remove the always-visible remote form. The add form can select a saved global host connection, but requires the explicit owner/group/project path rather than guessing from the local directory. Connection credentials never become clone URLs. Push destination remains an operation input; persistent remote URLs stay in Settings.

The existing footer gains passive repository remote/upstream information, with bounded configuration-only reads on repository changes, focus or confirmed configuration changes. No network polling, dashboard or navigation item is introduced. Multiple configured remotes without an upstream are listed without guessing an active one. Copyright credits match NOTICE and the Apache-2.0 license.

## First Push destination setup (supersedes Settings remote cards)

User correction: remove Repository remotes from Settings. Keep global host/account Connections there. A Push with no configured destination now asks whether to add one in the existing graph action context; its Yes action reuses that same area for existing URL/account choice or new hosted repository inputs. No new screen, sidebar, navigation item or provider-logo asset is introduced. Account management links to the existing Settings dialog while preserving the action context.

New repository name, owner/namespace, description and access are operation inputs. Default access is Private; Public is explicit and repeated in confirmation. Provider creation creates an empty repository only; reviewed local remote configuration and reviewed code publication remain subsequent distinct operations. Failed or uncertain provider writes do not retry or delete remotely. A verified created URL remains visible even if later local configuration fails.

## Branding, direct commit links and FAQ (2026-10-28)

The footer uses “GitNova · by edocstudio”; legal attribution remains in NOTICE and LICENSE. Copy commit link stays in the existing commit inspector/context menu and resolves the current branch's fetch upstream, then origin, then a sole remote. It uses global host Connections; remove the remote/provider/web-address form and legacy per-repository URL overrides. Never silently select among ambiguous destinations or copy a local path. Unknown hosts point to Settings.

An expandable, searchable FAQ lives inside the existing Settings dialog. Explanations are reference material rather than Git actions, so the graph cannot naturally hold the full guide without crowding commit context. This adds no dashboard, sidebar or top-level navigation. Keep matching English/Thai standalone docs for readers outside the app.

### Settings tabs (2026-10-28)

General, Connections and FAQ are tabs within the existing Settings dialog. Reference content and global account preferences do not belong in commit graph rows; tabs separate these existing sections without another screen or navigation destination. General is selected each time Settings opens. Keep connection drafts, FAQ queries and expanded answers mounted while switching tabs; support arrow/Home/End navigation and translated labels.

### Appearance preference (2026-10-28)

Theme belongs in Settings → General alongside Language. It is a device-wide visual preference, not a graph operation, so add one labelled Dark/Light selector there rather than a new header toggle or screen. Dark remains the default. Switching theme changes presentation only and preserves drafts, selection and Git state. Shared surface, text, border and graph palettes must cover both themes, including diff/conflict and warning states.

## Partial index changes

Stage/Unstage hunk is attached to the corresponding hunk header in the existing split/unified diff dock. This is a change to an exact displayed patch block and cannot be expressed naturally as another whole-file action. Do not create a separate staging screen. Only the complete canonical native patch authorizes an action; unsupported shapes retain whole-file staging. A fresh snapshot token binds working file, HEAD, index, configuration and attributes before atomic index publication.

## Owned linear rebase and related metadata maintenance

Rebase is invoked from the target commit's existing graph menu. Its start preview identifies current source branch → selected target, the bounded linear replay range, cached publication evidence and explicit rewrite acknowledgment. Pending conflicts reuse Working changes and the existing three-way diff; Continue/Skip-empty/Abort require fresh previews. Do not auto-stash, flatten merge topology or force-push.

Worktree lock/unlock/repair lives in an expandable metadata group in the existing worktree row. Submodule Sync URLs lives in that selected submodule's detailed row. These modify repository relationships/metadata, so they need selected scope rather than global Settings. Previews list affected metadata or old/new destinations; no new screen or permanent panel is introduced. There is no blind prune or recursive URL-sync action.
