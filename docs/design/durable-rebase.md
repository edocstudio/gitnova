# Next slice: owned, restartable linear rebase

Status: implemented bounded linear slice in [M56](../milestone-56.md).
Native Rebase uses the existing operation envelope with a dedicated rebase plan
and private journal, preserving conflict/diff surfaces. This document records the
design intent; M56 describes implemented bounds and verification. Arbitrary crash
boundaries and topology-preserving rebase remain additional hardening work.

## Existing surfaces

Graph-selected target → Rebase current branch onto this commit → exact range and
publication-risk preview → explicit Start. Keep the current branch and target in
that order. During conflicts, the existing Working changes inspector becomes the
owned Rebase context; use its file groups and diff dock. No dashboard or permanent
panel is justified. Finish/continue/abort actions belong to that same context.

## Native domain and journal

Keep this independent of Tauri. Define a versioned RebasePlan, RebaseState and
RebaseAction with stable repository/worktree identity, full local source ref,
original OID, target OID/ref, merge base, replayed commit list and original author
metadata, protected/cached remote refs, index/config snapshots, recovery refs and
expected native rebase metadata. Store the complete prepared journal before the
first native operation. Keep original and target tips reachable for recovery.

State progression: prepared → running → conflict/empty-stop → ready-to-continue
→ completed. Interruption or inconsistent native metadata produces needs-inspection,
not an inferred completed/aborted state. Each action has its own reviewed token
and append-only operation receipt. Native locks and repository operation guards
remain authoritative. Never replay Continue on startup, focus, or polling.

Preview operates in the existing isolated scratch object database, without native
hooks, checkout filters or custom merge drivers. Enumerate and bind a bounded,
complete linear range. Reject ranges containing merge commits until preserving
merge topology is separately implemented. Do not silently flatten them. Report
predicted conflicts and unsupported merge attributes instead of executing an
untrusted driver during preview. Refuse dirty/index-conflicted/pending or
ambiguous-worktree contexts before Start; no automatic stash/reset is permitted.

Execution must explicitly disable native automatic stash, unrelated ref updates
and implicit merge-topology changes. Use supported native Git rebase commands,
not manual commit substitution. Require explicit acknowledgment when cached remote
refs show rewritten commits; lack of a fetched ref is not proof of unpublished
history. Never force-push as a follow-on action.

## Conflict, Continue and Abort

Bind an owned conflict to its journal and matching native rebase directory, source
branch, stopped commit, rewritten HEAD and todo/done identity. Ordinary writes
remain blocked during the native rebase. Owned resolution stages only an exact
reviewed file/index result, with the existing raw-path and symlink protections.
Continue revalidates unresolved entries, native metadata and expected staged tree;
empty or skipped commits require an explicit policy and preview, not an implicit
success. A completed result must verify the final branch, unchanged unrelated
refs/worktrees, replayed authorship and target ancestry.

Abort reviews every affected path and captures working edits plus staging in the
existing bounded private recovery format before asking native Git to abort. Large
or unrepresentable recovery scopes block this action. Retain unpushed tips and
saved work; never hard-reset away conflict edits as app cleanup. A foreign or
externally edited native rebase cannot be adopted just because files exist.
Bookkeeping-only closure may be offered after an external finish/abort only when
its observed Git state can be verified, retaining the archived journal/recovery.

## Acceptance before shipping

Real temporary-repository tests must include clean replay, root/empty/skipped
commit policies, merge-range refusal, conflicts across several Continue steps,
rename/deletion/binary/raw-path cases, author/date preservation, branch/worktree
identity, dirty/untracked/symlink protection, custom-driver preview refusal,
published-risk acknowledgment, native lock/race scenarios, crash at each journal
boundary, external finish/abort and recovery of edited conflict results. UI tests
must prove source → target clarity, stable file groups/dock, action visibility at
minimum window size, localization/draft preservation and no mutation on cancel.
Only then add the graph action and call the feature implemented.
