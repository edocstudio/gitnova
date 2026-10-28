# M44 — GitLab status in the graph

The requested CI column belongs between Commit message and Author in the existing graph. It adds no screen, navigation or permanent pane. Selecting a commit retains the existing inspector for detailed jobs and provider configuration.

Read GitLab's own commit-level status with GET repository/commits/:sha?stats=false. This mirrors the provider's status rather than deriving an overall result from a subset of jobs. Jenkins appears here when it publishes status to GitLab; a standalone Jenkins server remains a separate optional inspector integration. No build is triggered. GitLab connection remains explicit and credentials stay in Keychain.

Visible commits only, at most 40, after a 350 ms scroll debounce. Shared single-request scheduler, no jobs/logs/artifacts, 128-entry frontend/native memory caches. Active status refreshes after 15 seconds and polls every 20 seconds; empty results can be checked again for newly started builds. Terminal results are reused for 60 seconds and then quietly rechecked while visible, so a new run on the same commit can be detected. Hidden windows stop starting requests; obsolete responses cannot paint another graph. Three consecutive unavailable commits stop a scan to avoid authentication failure fanout. No background scan of entire history. Keep cached values on failure and mark stale in the tooltip. Unknown, unavailable, unconnected and no pipeline are distinct. Percentages are not fabricated.

The commit response ID must equal the requested full SHA; the existing remote/repository validation remains mandatory. Commit links are reconstructed from the connected project, never arbitrary response URLs. This column displays the provider's commit status, not every pipeline/ref on the SHA; job and direct Jenkins details remain in the inspector. GitLab docs: https://docs.gitlab.com/api/commits/#retrieve-a-commit

Validation and limitations are recorded after focused checks. No live provider account/network test is performed without the user's connection; provider transport tests use bounded local fixtures, while UI tests use real disposable Git repositories.

## Verification

- TypeScript and all 29 frontend unit checks passed; EN/TH catalog parity retained.
- Two focused hosting tests cover server aggregate states, empty/unknown status, exact SHA rejection and inaccessible commits. The local HTTP test listener explicitly uses blocking accepted streams to avoid inherited nonblocking socket races on macOS.
- Five targeted browser flows passed across focused runs: visible unselected graph status and polling/hidden pause, retained GitLab results after failure, discarded late selection responses, mixed GitLab/Jenkins evidence and Jenkins hidden/late-response isolation. Disposable real Git repositories retained HEAD and working state. CI transport responses are fixtures, not live-account certification.
- Native command test passed for read-only scope validation and foreign-origin rejection, including ci_summary.
- Strict Clippy for hosting/core/desktop all targets and formatting checks passed.
- Local debug app built and signature/embedded assets verified: `target/debug/bundle/macos/GitNova Updated.app` (77.57 MiB). No GUI or developer workspace was opened by verification.
