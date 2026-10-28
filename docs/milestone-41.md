# M41 — GitLab CI status and shareable commit links

## Delivered scope

Read-only GitLab pipeline status for the selected exact commit, using the existing
commit inspector and bounded observed-status badges in the graph. Initial explicit
remote/HTTPS-host connection reuses the native GitLab account in Keychain; no
credentials enter browser storage. Connection settings contain only remote and
web origin, scoped by workspace membership. Ordinary Git never needs CI.

The adapter requests exact SHA, at most 5 latest pipelines and 50 jobs per run,
with explicit partial coverage. Each run retains ref/source/status and each job
stage/name/status/runner/duration. Unknown states stay unknown; mixed runs stay
separate. Job lists exclude child pipelines/bridges and retried attempts; the UI
says so and supplies the provider link. No invented completion percentage,
background Git fetch or build write is performed.

Core memory cache holds at most 16 snapshots, keyed by provider origin/project,
account and full SHA. Active snapshots expire after 15 seconds, settled after 60;
manual refresh can bypass TTL with a 2-second dedup floor. Network reads serialize
under a separate try-lock so another CI invocation cannot block a worker waiting
on that cache. HTTP has existing connect/read timeouts and a 2 MiB response limit,
no redirects, no automatic HTTP retry, and bounded identifiers/URLs/fields.
Frontend has at most 16 cached snapshots, 32 graph badges and deduplicated in-flight
reads. Poll only the selected inspector, every 20 seconds for active builds;
settled/empty results stop polling. Errors retry after 60 seconds and retain stale
results. Hidden windows cancel polling; late repository/commit responses are
ignored. This first slice does not persist CI payloads in SQLite or add Redis.

Copy commit link reuses the existing commit actions. A single recognized remote
copies immediately; multiple remotes or self-hosted/SSH aliases use inline explicit
remote/provider/HTTPS-origin settings. URLs use full SHA and the selected fetch
project, not a possibly different push destination. No auth/network request is
needed. Reject credential-bearing/local-path remotes and non-commit objects. A
clipboard failure leaves a selectable URL. Remote accessibility/publication is not
asserted from a locally constructed link.

## Verification

See the final delivery notes below for completed checks. Git/clipboard browser
flows use actual Rust core and temporary repositories; provider HTTP/Keychain
transport is substituted in those browser tests. Adapter contracts exercise real
HTTP against an isolated local server. This does not certify a live GitLab account.

## Remaining scope

Jenkins controller/job/SCM-revision mapping and secure credential integration,
child-pipeline drill-down, logs, trigger/retry/cancel/deploy, hosted merge checks,
persistent CI cache and large-workspace/long-session measurements are not shipped
here. Graph badges cover commits inspected in this session, not every commit in
the repository, and identify last-observed status. Terminal pipelines must be
refreshed/revisited to detect a later retry. CI success is not merge eligibility.

## Final delivery checks — 2026-10-28

- Hosting adapter suite: 11 passed, including exact-SHA GitLab requests, mixed
  job/runner states, bounded run/job coverage, unknown statuses, wrong-revision,
  cross-host, 404/rate-limit failures and GitHub/self-hosted GitLab commit URLs.
- Real-Git core scope test: passed for historical/detached commits, distinct
  fetch/push projects, missing/non-commit objects, and preserved index/files/HEAD.
- Targeted native Tauri command test: passed for the real commit-link result,
  external-origin rejection, and invalid CI scopes rejected before credentials,
  prompt or network access. No user account/catalog was used.
- Three browser workflows passed using temporary real Git: quiet CI polling and
  retained failure state; late-response isolation/hidden pause; clipboard links
  for historical commits, multiple remotes and persistent self-hosted settings.
  The last sharing flow was rechecked after preference reuse was added.
- 26 frontend checks and TypeScript validation passed. Core/hosting all-target
  strict Clippy and rustfmt checks passed.
- Serialized one-job local build completed in 53.95 seconds. Ad-hoc signature and
  embedded UI verification passed for `target/debug/bundle/macos/GitNova Updated.app`
  (76.75 MiB), without opening a GUI or user workspace. The currently running older
  app was not overwritten or terminated.

No live GitLab account or Jenkins controller was contacted during verification.
CI cache remains in memory. Successful status display is not evidence of long-session
RAM behavior or full production readiness. GitLab reconnect in the existing CI
settings explicitly updates the host account shared with merge-request workflows.
Copy-link host preferences contain no credentials and reuse matching CI host
settings where available.

Jenkins follow-up: the Git-plugin build-status slice is now delivered in [M42](milestone-42.md); stage/agent/log drill-down remains external.
