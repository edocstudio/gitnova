# GitNova — FAQ & guide

[English](faq.en.md) · [ไทย](faq.th.md)

Open **Settings → FAQ** to read and search this guide inside GitNova.

## Start here

### How do I turn a project folder into a repository?

Open the workspace picker → Initialize repository…. Enter the full path to a new or existing folder and the initial branch (default `main`), review, then Initialize & open graph. The parent folder must exist. Existing files stay unchanged and unstaged; no first commit or remote is created automatically. Use Open for an existing Git repository. See the [setup guide](../../README.md#start-a-local-repository) for this alpha workflow.

### How do I make my first commit?

Open Working changes, select a file to inspect its diff, then Stage file or Stage all changes. Enter a summary, review the staged files and confirm the commit. Only staged content enters the commit; committing does not publish it. Push publishes to a remote.

### What are a workspace, repository and worktree?

A workspace groups projects for quick switching; it is not a Git submodule. A repository stores Git history. A worktree is a checkout with its own files, HEAD and staging area; linked worktrees share repository history. Switch repositories from the workspace picker; search and pagination help with long lists.

## Everyday Git

### What does the graph show?

Each node is a commit; lines connect it to its parents. Branch and tag labels point to commits. HEAD marks your current checkout. A merge commit has multiple parents; a fast-forward moves a branch label without adding a merge node. Select a row to inspect its files and diff. Right-click a commit for available actions.

### Can I resize the graph columns?

Drag the separators in the history header; widths are saved on this device. Double-click a separator or press Home while it is focused to reset that column. Arrow keys resize it, with Shift for larger steps. The Graph column keeps a minimum width for its lanes; wide histories can scroll horizontally.

### How do I switch branches or open a remote branch?

Use the branch picker, or double-click a graph node or branch label to enter checkout review. Inspect the destination before confirming. Opening a remote branch can create a local tracking branch. Switching the parent repository does not silently update submodule checkouts.

### How are Stage, Unstage and Discard different?

Stage copies the selected changes into the next commit’s staging area. Unstage removes them from that area while keeping working files. Discard restores the reviewed scope and can remove new files. Review its exact scope before confirming; recovery information is shown after supported operations.

### Can I stage just part of a file?

Select a regular tracked text file in Working changes. Stage hunk or Unstage hunk on its diff header changes only that block in the index; working files stay intact. Full-file actions remain available. New/deleted files, mode changes, binary files, non-UTF-8 content and truncated patches require file staging. Stale previews are refused.

### What do Fetch, Pull, Push and Refresh do?

Fetch downloads remote history without merging into your local branch. Pull fetches and integrates into the current branch using the reviewed method. Push publishes selected local history. Refresh rereads local Git state; it does not replace Fetch. On macOS, HTTPS actions use a matching saved GitNova host account when available, otherwise configured Git credentials. SSH uses your existing agent.

### What do the up and down counts mean?

↑ counts local commits missing from the tracked remote branch; ↓ counts remote commits missing locally. Both can be nonzero when histories diverge. These counts describe the last fetched remote state, not live server state. Fetch before deciding how to integrate.

### How do I investigate a file or code change?

Open Search commits and choose File / code history. Enter a literal repository-relative path, a literal code phrase, or both; choose the repository scope and Search. Code search finds changes in the phrase occurrence count, not every edit to lines containing it. It considers at most 2,000 recent local commits per location and shows up to 100 results; merge changes compare with the first parent and renames are not followed. Select a result to open its graph and historical diff without checkout. Shallow or bounded coverage is labelled.

## History and recovery

### How do Stash, Apply and Pop work?

Stash saves unfinished work. Apply restores it onto the currently checked-out branch and keeps the stash. Pop restores it there and removes the stash only after success. You can stash on main, switch to develop, then pop onto develop. Conflicts keep the stash. Finish an active Git operation before restoring another stash.

### How do Merge and Fast-forward differ?

Merge combines histories into the current branch. Create merge commit retains both tips as parents, making the junction visible. Fast-forward is available when the current tip is an ancestor of the incoming tip; it only advances the branch. Optional source deletion removes only the eligible local source branch after a verified successful merge.

### What is Cherry-pick? Why choose a parent?

Cherry-pick applies one commit’s changes onto your current branch as a new commit. A merge commit has multiple parents, so choose the baseline used to calculate those changes. Parent 1 is usually the destination branch before the merge; compare the preview before selecting it. Parent 2 produces a different delta.

### What is Rebase?

Rebase replays commits onto a new base and changes their IDs. Right-click the target commit, choose Rebase, inspect current branch → target and the replay range, then acknowledge rewriting. GitNova supports up to 100 linear commits; merge/root ranges and submodule pointer changes are refused. Conflicts use Working changes, followed by reviewed Continue, Skip empty commit or Abort. Original tips remain recoverable; no force push occurs.

### What are Amend and Revert?

Amend replaces the last commit using the reviewed message and staged content; its ID changes. It does not combine arbitrary older commits. Revert creates a new commit undoing a selected commit’s changes. Prefer a new revert when preserving shared history matters.

### How do I resolve a conflict?

Open Working changes and select a conflict. Compare Current and Incoming, choose individual text blocks or edit Result, then review and stage the resolved result. Continue or commit the pending operation only after every conflict is resolved. Current/Incoming refer to the shown operation, especially during rebase. Binary and submodule conflicts need their dedicated choices.

## Multiple repositories

### What is a submodule?

A submodule is a separate Git repository referenced by a recorded commit in its parent. Each parent branch can record a different commit. Recorded, staged and checked-out commits can differ. Dirty means files inside changed; Commit changed means the checkout differs from the recorded pointer. Detached HEAD is normal when checking out a recorded submodule commit.

### Should I restore a submodule or stage its pointer?

Use the Submodules row for the selected branch and Manage to inspect it. Open the child graph to handle its files. Restore recorded returns its checkout to the parent’s recorded commit; stage its pointer to record the checked-out commit. Review dirty files and unpushed commits first. Sync URLs copies the reviewed .gitmodules URL to the selected initialized child’s local configuration; it does not fetch, change files or update its commit. Relative URLs are resolved against the parent branch’s configured remote for supported standard URL shapes; review the resolved destination before confirming.

### When should I use a linked worktree?

Use a linked worktree to keep another checkout of the same repository, for example to review a release while keeping feature work open. Open or create one from the graph context. In Related repositories, Worktree metadata offers reviewed Lock, Unlock and Repair for an existing linked checkout. Lock protects against pruning; Unlock removes that protection. Repair fixes metadata without deleting files or moving branches. Dirty worktrees are not removed automatically.

### Can I compare release branches across repositories?

In Search commits, choose Compare branches across repositories and select the current, selected or entire-workspace scope. Enter Source and Target branch names, then Search. Evidence lists source-only and target-only commits, exact tip IDs and changed-file counts for each location. Missing refs, shallow history and limits remain explicit. Cached remote refs are not fetched automatically by this comparison. This is committed Git evidence; it does not certify CI, dependency compatibility or release readiness.

## Sharing and connections

### How do I clone or publish a new repository?

Clone from the workspace picker into a new folder. To publish local work, commit first and use Push. If no remote exists, Add remote lets you use an existing repository URL or create an empty GitHub/GitLab repository using a global Connection. Repository creation, local remote setup and Push are separate reviewed steps. Choose visibility carefully.

### How do I create and push a version tag?

Right-click the desired commit and choose Create tag. Review the name and exact commit. A local tag is not published by a normal branch push; use the tag’s Push action and review the destination. Existing remote tags are not silently overwritten.

### How do I open a pull request or merge request?

Use the branch’s request action and verify the source → target direction. Publish the source branch first. GitHub calls it a pull request; GitLab calls it a merge request. The host connection needs the relevant API permissions. Creating a request does not itself merge the branches.

### What does Copy commit link copy?

It copies the selected commit’s full remote web URL. GitNova uses the current branch’s fetch upstream, then origin, then a sole configured remote. Global Connections identify private Git hosts. No token is needed just to form a link. A local-only commit may not open for teammates until pushed; a link does not grant access.

### Where do I configure accounts and CI?

Open Settings → Connections. Host accounts apply globally to matching Git hosts, not one repository. CI status appears between Commit message and Author when configured and available. GitLab pipelines and configured Jenkins evidence are read from their services, not from local Git alone. On macOS, the saved GitHub/GitLab host account can also authenticate HTTPS Git operations; tokens stay in Keychain and pass to Git through a process-local helper. Existing Git credentials are a fallback when no app account exists. SSH uses your agent. Git permissions still depend on the token.

## History and recovery

### What if an operation fails or stays pending?

Open Working changes to inspect a pending merge, cherry-pick, revert or GitNova-started rebase. Resolve conflicts, then review commit/Continue or Abort. An uncertain interrupted journal needs inspection; the app never retries automatically. Handle an externally started rebase with Git outside the app. Zero changed files does not mean no operation is pending. Recent activity and recovery show retained references or backup locations; review them before recovery.

### Why can the commit author differ from my connected account?

A connection authenticates Git, PR/MR and CI requests. Commit authorship normally
comes from Git configuration. In **Settings → Connections → Commit identity**,
you can explicitly save a name and email for that host. Use a verified email or
your GitHub noreply address for GitHub attribution; GitNova does not infer an
email from a provider login.

GitNova matches the repository's `origin`, or its only remote when `origin` is
absent. Multiple possible remotes or ambiguous host profiles require inspection;
they do not choose an arbitrary account. With no matching override, Git settings
remain the fallback. The override is local to GitNova and does not edit Git config
files. Review the **Committer** line before confirming. Amend, cherry-pick and
rebase preserve original authors. Existing commits and published history are
never changed by saving an identity. Identity or remote changes invalidate a
pending commit review; refresh and review again. Finish pending merge/rebase
operations before changing their identity settings.

## How do I open a terminal in the current repository?

Select Terminal beside Pull/Push. The on-demand dock starts your shell in that
repository or linked worktree folder. Each membership keeps its own session;
switching repositories never sends input to the previous repository's shell.
Hide preserves the shell; End session asks before closing it. Up to four sessions
can stay open, and app/window closure or reload closes them. Detached/background
programs may have their own lifetime. The starting folder remains shown even if
you later run `cd`.

Commands run directly with your normal shell/Git configuration, without GitNova
write previews or connection commit identities. Refresh the graph to inspect
external changes. Auto fetch pauses while any terminal session remains open.
Scrollback/output retention is bounded; older output can be trimmed. Terminal
transcripts are not stored by GitNova; your shell may keep its usual history.
