# M42 — Jenkins build status in commit context

## Delivered behavior

The existing CI group in Selected commit now supports a separate Jenkins
connection alongside GitLab. Choose the Git remote, HTTPS controller URL, full
folder/branch job URL and Jenkins username. Enter the API token only in the native
secure prompt. The account is verified through whoAmI before storage under a
separate Jenkins Keychain service, keyed by controller and username; connecting
Jenkins cannot replace the GitHub/GitLab account. Browser preferences contain only
connection addresses, remote and username.

One explicit job is configured per workspace repository entry; other branch jobs
are not discovered automatically. Read up to 20 recent builds. The Jenkins Git plugin's BuildData must identify the
selected repository and exact last-built SHA. Standard credential-free SSH/HTTPS
remote URLs compare by host/path, preserving path case and nonstandard ports;
SSH aliases and different mirror hosts are not guessed. Missing or ambiguous
SCM metadata, multiple revisions for the same repository, parameter-only commit
values and mixed-repository URL sets stay unlinked. Never search old
buildsByBranchName/changelog data as proof of the current build's revision.

Display running, success, failure, unstable, canceled, not-built and unknown
states distinctly. Pipeline build links are constructed from the validated job
URL and numeric build identity, ignoring remote-returned URLs. Unverified coverage
is labelled No verified build rather than success; mixed GitLab/Jenkins results
remain mixed. Stopping one provider preserves the other's results.

Stage/agent/log detail is available through the Jenkins build link, not read or
persisted in this slice. No trigger, retry, cancel, deploy, Groovy, custom hook or
workspace scan is invoked. The adapter is independent of Tauri.

## Bounded read lifecycle

- Jenkins HTTP: one GET of the selected job's filtered JSON per status read;
  at most 20 builds, 128 inspected actions per build, 32 remote URLs per action,
  2 MiB response, 20-second request / 8-second connect timeout. Over-limit action
  metadata stays unverified. HTTPS only; redirects and automatic retries disabled.
- Native and frontend Jenkins caches: at most 8 snapshots each. Native keys include
  controller, job, exact SHA, repository remote, username and a credential-bound
  digest. Account/job changes cannot reuse a different credential's snapshot.
- Shared GitLab/Jenkins provider scheduler permits one read at a time, deduplicates
  identical scopes, and drops obsolete graph selections rather than queueing them.
  Each inspector read also guards against overlapping visibility/poll callbacks.
- Active matching or active-unlinked Jenkins runs poll every 20 seconds. Terminal
  results stop polling; empty/unverified history with no active run needs manual
  refresh/revisit. Errors back off 60 seconds and retain labelled stale evidence.
  Hidden/unmounted scopes stop polling and discard late updates.
- Graph retains at most 32 observed provider badge entries; no complete-repository
  or background-workspace CI scan, logs cache, SQLite CI payloads or Redis process.

These are implementation limits, not a measurement of long-session RAM use.

## References and verification scope

The adapter follows the Jenkins Remote Access API, the Git plugin's exported
BuildData.remoteUrls / lastBuiltRevision and Revision.SHA1:

- https://www.jenkins.io/doc/book/using/remote-access-api/
- https://raw.githubusercontent.com/jenkinsci/git-plugin/master/src/main/java/hudson/plugins/git/util/BuildData.java
- https://raw.githubusercontent.com/jenkinsci/git-client-plugin/master/src/main/java/hudson/plugins/git/Revision.java

Contract tests use an isolated HTTP server and dummy Basic credentials. Browser
flows use real Rust/Git in temporary repositories, substituting provider/Keychain
transport only. Native tests reject foreign WebView origins and invalid scopes
before Keychain or network access. No user Jenkins controller, token or live
account was accessed. Actual controller/plugin/account acceptance is still needed;
unsupported SCM metadata remains explicitly unverified.

Final verification and the delivered bundle are recorded below.

## Native build links

GitLab and Jenkins now share a CI link component. Native clicks call a validated
HTTPS-only browser command instead of relying on WKWebView target-blank behavior.
Credential-bearing/local/script URLs are refused; the external WebView origin is
rejected before launching anything. The macOS browser launcher waits at most five
seconds, and failure retains a selectable URL. No API token is added to a browser
link. Tests substitute the OS-open transport; no real browser was launched during
verification.

## Final verification — 2026-10-28

- 17 hosting adapter checks passed, including six Jenkins/browser-link tests and
  existing GitLab/GitHub request/CI/link regressions. Dummy credentials only.
- Real-Git scope test passed, including invalid Jenkins scopes rejected before
  credentials, with original files/index/HEAD preserved.
- Targeted native Tauri command test passed for Jenkins/CI/link command registration,
  foreign-origin rejection and invalid inputs rejected before Keychain/browser/network.
- 29 frontend tests and TypeScript validation passed.
- Five real-Git browser flows passed in 49.2 seconds: GitLab quiet/stale behavior;
  GitLab late/hidden handling; commit clipboard/multiple/self-hosted remotes;
  mixed Jenkins/GitLab results, unknown coverage and both native browser-link
  handoffs; Jenkins late/hidden behavior. The selection race found during this
  work is protected immediately at graph click, not only after inspector cleanup.
- Strict Clippy passed for core, hosting and desktop, all targets; rustfmt passed.
- One-job local macOS debug bundle built in 45.91 seconds, ad-hoc signature and
  embedded UI verification passed: `target/debug/bundle/macos/GitNova.app`
  (77.40 MiB). No GUI, real browser, user workspace or live account was opened.
  The running older `GitNova Updated.app` was preserved.

Public signing/notarization, live controller/plugin acceptance, multiple job
mappings/automatic branch-job discovery, stage/log drill-down in GitNova and
long-session performance/RAM measurements remain separate work.
