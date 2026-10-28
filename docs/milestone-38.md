# M38 — Create and open GitHub PR / GitLab MR from a graph branch

Right click the exact local branch → Create / open PR or MR. Choose its remote;
known hosts are detected, with explicit provider/HTTPS origin settings for custom
hosts, SSH aliases and GitLab subgroups/base paths. This is the same graph review
surface, without a permanent provider dashboard.

First use connects a provider access token through a native macOS hidden-entry
prompt identifying the provider/host. The prompt labels come from the EN/TH
frontend catalog. Verify `/user` before storing the token with Security Framework
in Keychain. IPC returns account identity only. Reconnect and disconnect remain
available in the review. Tokens are not in argv, catalog JSON, receipts, drafts or
API responses. Native TLS uses OS trust and system proxy support; redirects are
not followed and response bodies have a 2 MiB bound.

Choose target project/branch, title, description and Draft. A blank target uses
the provider's actual default branch, confirmed in the next review. Resolve stable
source/target project IDs, including fork targets. A missing/different published
source revision offers the existing reviewed branch Push, seeded to the selected
remote/branch. Back preserves title, description and Draft. Publication and
request creation remain separate confirmations and outcomes.

Review binds selected source branch/OID, provider, project IDs, account, target
OID, request fields and Git configuration. Look up open requests by exact
source/target identities before creating. Open existing reuses its real title,
body, state and URL. Creation calls the provider's API, then displays returned
request details in the graph review. This first slice uses that review rather
than a new inspector layout; graph badges, provider diffs and CI/reviewer details
remain follow-ups. The actual returned source SHA is shown, with a changed-scope
notice when it differs. Source/target branches remain live; no atomic freeze is
claimed. A journal records the operation before POST and retains the returned
request in the receipt afterward. An unknown POST result is not automatically
replayed. Refresh lookups precede another explicit creation attempt.

The Rust `gitnova-hosting` crate is Tauri-independent. Provider HTTP, account
storage and the frontend/native transport are separate. Reads are on demand with
bounded pagination (five 100-item duplicate-lookup pages) and HTTP timeouts; no
workspace-wide background scan or new cache service is added. No auto-merge,
source-branch deletion, squash/rebase or protected-branch bypass is implemented.

Verification boundaries:

- Real temporary Git repositories test source publication/tag behavior and the
  existing branch Push used for handoff.
- Provider adapters have local HTTP contract tests for GitHub draft creation,
  GitLab fork creation using the source project endpoint, default target branch,
  existing-request reuse without POST, unpublished-source refusal, URL/credential
  handling and failed-POST non-retry. These do not contact live accounts.
- Browser tests combine real local Git/core transport with a clearly test-only
  provider transport fixture. They exercise both providers, title/body/Draft,
  source/target review, duplicate opening and handoff draft preservation.
- Tauri command origin checks deny external calls, including credential entry and
  creation. Native secure-entry and live Keychain/provider account setup require
  the user's explicit first-use connection; tests do not open credential dialogs
  or create PRs/MRs in work repositories.

Live GitHub/GitLab sandbox acceptance and enterprise-version compatibility are
not established by contract fixtures. One-click OAuth needs app registration;
this slice provides access-token connection. Hosted merge, CI/checks, approvals,
branch badges and provider diffs remain planned in the design's next slice.

Published source branch names may differ from local names. The review seeds the
published name from existing tracking/push configuration, exposes it explicitly
and uses it for API lookup, duplicate reconciliation and branch Push handoff.
The local source commit remains pinned separately. One saved account/token per
provider host is supported initially; reconnect replaces that saved connection.
The secure AppleScript prompt was syntax-checked with localized confirm/cancel
labels without displaying a dialog or reading credentials.

Provider-transport fixture UI captures (not live-account verification):
[GitHub](screenshots/hosting-github-contract-review.png),
[GitLab](screenshots/hosting-gitlab-contract-review.png).

Final local verification (2026-10-27): 6 provider adapter tests, 26 frontend
tests, 3 focused browser flows and the native Tauri real-Git/origin test passed.
The two Core tag integration tests also passed. The final macOS build passed
TypeScript compilation, native compilation, ad-hoc signing and embedded-asset
verification at `target/debug/bundle/macos/GitNova.app`. No live account request
was created by these checks.

Review scope is revalidated before returning an existing request as well as
before creation. A changed account, remote, source or target requires a fresh
review; account connection uses the displayed provider and HTTPS origin.

Direction clarity follow-up: Source → Target is visible before connection, in
review and in the result. Source/target fields are available before account
connection; an empty target explicitly names the unresolved provider default.
The graph header labels the checkout separately. Both provider UI fixtures
passed with an explicit release target, including connection, Push handoff and
existing-request reopening; 26 frontend checks passed. The updated local macOS
bundle passed TypeScript/native build and embedded-asset verification.

M40 CI follow-up preserves the hosted plan's JSON shape with boxed internal
storage and validates its persisted round trip. GitLab existing-MR reconciliation
accepts validated project IDs returned as numbers or numeric strings; both forms
are tested to reuse the request without a POST. Seven provider contract tests
pass. Native live-account acceptance remains outstanding.
