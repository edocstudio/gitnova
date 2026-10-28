# Version tags and hosted pull/merge requests

Status: initial tag publication and create/open PR/MR slices implemented on
2026-10-27. See milestones 37 and 38 for tested behavior and integration limits.
The broader follow-up interactions below remain a design, not a completion claim.
User direction: create a version tag from a graph commit, publish it explicitly,
and create/open PRs and MRs inside GitNova with the same graph-first clarity.

## Baseline before implementation and surface decision

The graph already creates lightweight local tags. RefManager manages local tag
deletion and retains the original tag object for recovery. The existing Push
implementation intentionally publishes one branch, excluding tags. Hosted PR/MR
creation, provider authentication and request status are not implemented.

A commit context menu already identifies the exact tag target. A branch context
menu already identifies the source of a hosted request. Reuse those menus, the
existing review surface, selected-commit inspector and diff dock. No permanent
Tags, Releases or Pull Requests dashboard/sidebar is required for this slice.

## Version tag flow

1. Right click the chosen commit → **Create tag…**. Replace the existing
   lightweight-only menu item rather than adding another neighboring item.
2. Show repository, selected commit SHA/subject, tag name (example `v1.2.0`),
   type and optional annotation. Default a release/version tag to Annotated;
   offer Lightweight explicitly. Do not require Semantic Versioning syntax:
   validate actual Git ref rules and reject duplicate names or collisions.
3. Review → **Create tag**. Pin the exact selected commit, not the moving HEAD.
   Create the local ref without replacing an existing tag. Signing is a separate
   explicit later capability; do not imply an unsigned annotation is signed.
4. Keep the same graph selection. Show the tag badge with a contextual completion
   action **Push tag…**; right click that badge also offers Push tag later.
   The main Push control continues to mean the current branch, with no inferred
   tag publication.
5. The current review displays tag name, annotation/type, selected remote and
   actual push destination, tag object OID and peeled commit OID. Inspect the live
   destination. For a matching existing remote tag, show Already published;
   for a different existing tag, explain the conflict and refuse replacement.
6. **Push tag v1.2.0** publishes this exact ref only, then verifies the remote and
   returns a receipt. Preserve the local tag on failure. Treat an indeterminate
   network result as Needs inspection until remote state has been queried.

Example existing-context review:

    Create tag                      Commit a1b2c3d4
    Version/name     [v1.2.0                   ]
    Type             [Annotated                ]
    Message          [Release 1.2.0            ]
                         Back      Review tag

    Push tag v1.2.0                 project / repo
    Destination      origin · verified push endpoint
    Commit           a1b2c3d4 · exact selected commit
    Tag object       9e8f7a6b · annotation retained
    Remote state     Tag does not exist
                         Back      Push tag v1.2.0

Tags belong to repository commits and are shared by linked worktrees, rather
than belonging to one branch/worktree. Publishing a tag does not publish all
branch tips or create a hosted release. Nested submodules use their own repo's
context and remote; parent and child tags must never be conflated.

## PR / MR flow

Use provider terminology: **Create pull request…** on GitHub and **Create merge
request…** on GitLab. Both implement the same internal change-request model.

1. Right click a local/remote branch badge → provider-specific Create action.
   A bare commit is not a source branch. If it has no unambiguous branch, first
   use existing Create branch here; when several refs coincide, choose the exact
   source. Do not silently choose the currently checked-out branch instead.
2. Resolve the provider, host and real project identity from selected remote
   metadata. GitLab Self-Managed/Enterprise hosts need explicit host/provider
   mapping. Support SSH aliases, subgroup paths, custom ports/base paths and
   separate fetch/push URLs through validated resolution, rather than guessing
   project identity from a directory name or blindly replacing URL text.
3. Connect an account in the same review when needed. Git SSH/credential access
   does not by itself provide a GitHub/GitLab API account. Show host and account
   identity before use; keep access tokens in the OS credential store.
4. Display source project/branch → target project/branch. Suggest the provider's
   actual default branch, or an explicit saved per-project preference; do not
   assume `main`/`develop`. Show published commit state, comparison summary,
   title, description and Draft where the provider/account supports it.
5. If the source branch is unpublished or local commits are ahead, offer the
   existing reviewed branch Push first. On success refresh and revalidate source
   publication, then allow **Create PR/MR**. No hidden push or combined atomic
   promise. If creation fails after Push, report that publication succeeded and
   request creation remains unfinished. A remote-only source needs no local
   checkout or implicit local branch creation.
6. Before POST, validate the host/project/account, source/target branches and
   current remote comparison. Check for an existing open request with that exact
   source/target identity. Offer Open existing instead of creating duplicates.
7. Confirm **Create pull request** / **Create merge request** inside GitNova using
   the provider API. On success show the returned request number/title/status
   beside its source branch and open its detail in the existing inspector.
   Open in browser is an optional action using the returned validated web URL,
   rather than being the only way to complete creation.
8. Initial inspector: title/body, source/target, Draft/Open/Merged/Closed state,
   request link and available CI/review summary with loading/unknown states.
   Load details only when selected. Changed files reuse the diff dock with the
   comparison's explicit head/base revisions and bounded pagination.

    Create merge request            gitlab.example / team/project
    Account          signed-in account
    Source           feature/payments (published SHA)
    Target           develop
    Comparison       3 commits · provider comparison scope
    Title            [Handle payment retries              ]
    Description      [                                    ]
    Draft            [ ]
                         Back      Review → Create merge request

Source/target tips are live hosted branches, not frozen by the creation API.
Record reviewed SHAs, validate before creation and inspect the returned head
revision afterward. If they moved during the request, show the actual created
request and changed scope; do not promise atomic SHA-pinned creation. Never
blindly repeat POST after a timeout: query existing requests first and report
unknown state if it cannot be reconciled. Permission errors, API throttling,
expired sessions and unsupported Draft capabilities remain explicit.

## Hosted merge and release follow-ups

Creating/opening a PR/MR and merging it are separate actions. A later **Merge
request…** action inside the same inspector must read current approvals/checks,
permissions, conflict/mergeability state and available merge strategies from the
provider. Require an explicit final review tied to the current source SHA; use
provider SHA guards where supported. Do not default to squash, delete a source
branch, enable auto-merge, publish a release or bypass protected-branch rules.
Refresh local Git refs explicitly after hosted completion; an API merge does not
silently pull/reset a local working tree. Show newly merged state and the next
Fetch/Pull action in existing graph context.

This interpretation includes creating/viewing requests in the first provider
milestone; hosted merge and hosted releases are clearly separate later scope.

## Architecture and API boundaries

Keep domain/core code independent of Tauri:

- Extend existing Git/ref adapters with typed CreateTag and PublishTag previews,
  apply requests and durable operation receipts. A publish request identifies
  repository, exact tag ref/object and one selected configured remote.
- Introduce `gitnova-hosting` with a provider-neutral ChangeRequestService and
  separate GitHub/GitLab adapters. Provider identity is host + immutable project
  ID; retain source and target project IDs for forks and local membership IDs
  only for navigation. Repo folder names and URLs alone are not stable IDs.
- Use a native credential-store interface, with macOS Keychain initially and
  platform adapters later. Persist only account/credential references in app
  metadata. Keep raw credentials out of frontend state/IPC responses, operation
  receipts, catalog JSON, diagnostics and caches. Scope credentials to the
  validated host/account and do not forward them across API redirects.
- Initial account connection may use provider-scoped tokens through a native
  secure entry path; an OS-owned authentication sheet is justified if the
  current WebView form would retain a long-lived secret. For the streamlined
  login milestone, use provider-supported native-app OAuth: GitHub device flow
  with a registered application and GitLab public-client PKCE with per-instance
  registration. Do not embed a confidential client secret in the desktop app or
  promise OAuth support on arbitrary enterprise versions without discovery.
- Keep network requests in native adapters with timeouts/cancellation, bounded
  response sizes and explicit pagination. Limit concurrency; on-demand bounded
  metadata cache can reuse the existing Moka dependency, keyed by host/account,
  project and source/target revisions. Never cache write authorization. No
  Redis process or always-on workspace-wide PR/MR polling is required.

For annotated tags distinguish tag object identity from its peeled commit. An
explicit single-ref push preserves the original object/annotation and disables
implicit tag following. Preview does not run signing or push hooks; execution
reports configured hooks/signing and provider-side protection failures honestly.
Local creation, remote publication and provider API operations retain separate
results and recovery paths. Do not delete/recreate refs automatically as rollback.

## Incremental implementation and acceptance

| Slice | Reviewable result | Essential validation |
| --- | --- | --- |
| 37A | Unified Create tag form; annotated/lightweight local creation | Exact non-HEAD commit, Unicode/ref rules, duplicates, concurrent creation, annotation retention, unchanged checkout/index |
| 37B | Push selected tag from graph badge/result | Local bare remote; only named tag published; preserve annotation object; existing same/different remote ref; protected/rejected/stale/indeterminate pushes; hooks only on apply |
| 38A | Provider identity + native account connection + read-only request lookup | SSH aliases/subgroups/forks/enterprise hosts; account mismatch; auth failure; pagination/limits; zero secret leakage |
| 38B | Create/open GitHub PR and GitLab MR from branch context | Explicit source/target, draft/title/body, publish-first handoff, duplicate lookup, stale remote state, ambiguous POST reconciliation, preserved drafts and graph context |
| 38C | Provider detail/check status + hosted merge review | Current source SHA guard, required approvals/CI, unknown/pending state, strategy permissions, partial failures and local Fetch/Pull guidance |

Use temporary real Git repositories for tag/push and branch publication tests.
Provider transport contract tests may use recorded/synthetic API responses, but
release acceptance requires explicit opt-in sandbox-host integration against real
GitHub/GitLab test projects. Do not create requests or tags in the user's actual
work projects as an implicit test. Verify one build/test runner at a time and
EN/TH states, pending feedback, keyboard interaction and bounded resource use.

## Primary references checked for this design

- Git tag object/annotation semantics: https://git-scm.com/docs/git-tag
- Explicit refspec and tag push rules: https://git-scm.com/docs/git-push
- GitHub PR API (create/read/merge, permissions, source/base): https://docs.github.com/en/rest/pulls/pulls
- GitLab MR API (create/read/merge, source/target projects/branches): https://docs.gitlab.com/api/merge_requests/
- GitHub native-app/device authentication: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps
- GitLab OAuth/PKCE and instance capabilities: https://docs.gitlab.com/api/oauth2/
