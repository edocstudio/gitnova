# Pipeline/build status in the existing Git graph

Status: GitLab selected-commit reads implemented in M41; Jenkins Git-plugin
build evidence implemented in M42. Full visible-graph coverage remains proposed. User request:
show running Jenkins builds and GitLab CI jobs/pipelines in GitNova. This extends
the provider check-status slice without a new dashboard or permanent navigation.

## Existing surfaces and exact scope

Add a compact status badge beside the commit subject, populated only for loaded
visible commits after an explicit provider connection. Selecting the badge/commit
uses the existing commit inspector for provider, project/job, branch/ref, full
commit SHA, run number/source/attempt, status, stages/jobs and elapsed time, plus
the provider's build link. A branch summary may describe only its exact current
commit, never a different "latest build" on a moving branch. One SHA can have
several pipelines/providers/reruns; keep each result accessible and show mixed
results rather than overwriting one run with another. Distinguish failed from
unstable, skipped, canceled, manual, blocked and unknown states.

GitLab integration reads project pipelines filtered by SHA and their jobs/bridges.
A Runner is an executor attached to a job, not the pipeline itself. Expose the
job's runner identity where available, without equating runner health with build
success. MR/merge-train/child pipelines may build synthetic or different SHAs;
label their source explicitly and never attach their result to a different local
commit by branch-name inference.

Jenkins uses explicit controller + job/folder/multibranch mappings per repository.
Read bounded build records and their SCM revision metadata. Different plugins and
multi-SCM pipelines expose revision metadata differently. Match the selected
repository's full revision; a branch name, lastBuild, changelog fragment or merely
a configured parameter is not sufficient evidence. If mapping cannot be proved,
show the run as unlinked/unknown rather than an incorrect graph badge. Validate
the behavior on the user's actual job/plugin configuration before claiming it
works with every Jenkins installation.

## Quiet refresh, bounded resources and local-first behavior

Use provider adapters independent of Tauri, with normalized run/job identities.
Reuse existing GitLab host/account storage after verifying read permissions;
Jenkins has its own explicit controller/account connection stored in Keychain.
No credentials are read from the chat or logged. No CI connection is required
for ordinary local Git usage.

Use bounded in-memory cache + existing SQLite metadata keyed by provider,
controller/project/job, exact SHA and run/attempt identity. Coalesce requests
across worktrees sharing repository identity; do not merge independent build
contexts for separate CI projects. Show cached data immediately with last-updated
and stale/offline states. Poll only visible/selected scope while builds are active,
with bounded concurrency, cancellation, response/page limits and backoff for
rate limits. Stop on hidden windows and stop tight polling for terminal results.
Background refresh never clears rows, inserts loading banners, flashes the graph
or resets selection. One initial/manual loading indicator is sufficient.

Do not invent a completion percentage from elapsed time or completed job counts.
Stages/jobs can be shown as counts; any estimate is explicitly an estimate. Use
running state + elapsed time when the provider has no reliable progress value.
Build logs, if added later, are fetched on demand with bounded bytes, rendered as
text, and not permanently indexed/cached by default.

## Delivery and acceptance

Start with read-only GitLab pipeline/job status because GitLab context/auth already
exists, then Jenkins controller/job mapping and status. Trigger/retry/cancel or
manual deployment are separate future write workflows, each with explicit scope,
permissions, review/confirmation and unknown-result reconciliation. Do not silently
add pipeline actions to Push/Pull or create a remote build as a status refresh.

Tests must prove exact-SHA mapping, multiple runs/providers, stale response
isolation on repo/commit switches, mixed statuses, hidden-window pause, quiet
refresh, bounded cache/request behavior, auth/rate-limit/offline errors and
unknown Jenkins SCM mappings. Real GitLab/Jenkins sandbox acceptance remains
necessary beyond HTTP contract fixtures. Pipeline success alone does not establish
merge eligibility or replace approval/protected-branch checks.

Primary API references:

- https://docs.gitlab.com/api/pipelines/
- https://docs.gitlab.com/api/jobs/
- https://www.jenkins.io/doc/book/using/remote-access-api/

## Graph commit status (M44)

The graph now places CI between Commit message and Author. After connecting GitLab in the existing inspector, visible commits read GitLab's commit-level status rather than downloading all jobs. This includes external CI that actually reports status to GitLab. The server's aggregate is authoritative for this column; it is not computed from a partial job list. Direct Jenkins configuration remains optional for jobs not reported to GitLab. See [M44](../milestone-44.md) for cache bounds, polling and unavailable-state behavior.
