# Native CI follow-up — 2026-10-31

Baseline: `bf716df47e454748ae8a4ecd15dc2d763b8c4fe1`.
[Hosted run 41166932243](https://github.com/edocstudio/gitnova/actions/runs/41166932243)
passed all lint steps and the macOS desktop job, including its graph suite.
The Linux desktop IPC/PTY and frontend unit suites also passed. Five jobs still
failed; native OS-store and installer steps did not run. This report does not
certify Windows/Linux feature parity.

## Findings and corrections

| Observed evidence | Correction |
| --- | --- |
| Windows core reached Amend; seven tests reported `could not read log file '//?/C:/…'` | Normalize absolute message/config/clone path arguments at Git's parser boundary. Reuse that conversion for alternate-index environment paths, preserve native Unix bytes and reject unrepresentable Windows paths. Amend fixtures include spaces, apostrophe and Thai. |
| Linux core merge/submodule fixture returned an unknown receipt | Persist synthetic identity/signing policy in that temporary parent repository. Apply the same correction to patch-submodule and cloned pointer-publication fixtures, and retain receipt diagnostics. Command-local fixture `-c user.name=…` does not configure the application's later Git processes. |
| Linux graph job failed 22 cases, including explicit `Committer identity unknown` and unresolved follow-on operation assertions | Initialize only new temporary checkout fixtures with local synthetic identity and fixed line-ending policy. Launch the real core bridge with an empty private home to prevent a developer's global configuration from hiding missing fixture setup. No real user's Git configuration is changed. |
| Windows IPC returned `add_repository not allowed. Plugin not found`, then a poisoned test lock | Use Tauri's documented platform-local test URL: `http://tauri.localhost` on Windows and `tauri://localhost` elsewhere. External-origin negative tests remain intact. No production ACL is relaxed. |
| Windows PTY fixture timed out waiting for PowerShell exit | After the child exits, release the pseudo-console while its output reader drains; exit is reported only after child termination, reader EOF and buffered output delivery. The native fixture answers cursor-position requests, as the real xterm UI does, instead of leaving PowerShell's line-editor handshake blocked. A native Windows rerun remains required. |
| macOS core's real HTTPS test failed with curl 55/broken pipe | Make the bounded loopback fixture support HTTP/1.1 and chunked request bodies, drain request data before authentication/rejection responses, and bound request size/time. Keep the actual authenticated Git Push/Fetch, redirect isolation and explicit buffered-upload assertions. No production retry or credential policy is changed. |

The previous Windows activity/restart test passed in this run; later core suites
were blocked by Amend. That is evidence for this one test, not a complete native
discard acceptance pass. Earlier fixes and historical checks are retained in
[the preceding report](native-ci-corrections.md).

## Foreground measurement protocol

The debug-only isolated native soak accepts `--foreground`. Only its private
catalog/profile can request a visible focused window. The driver waits for real
`document.visibilityState === 'visible'` and refuses foreground acceptance when
it observes a hidden WebView. Normal application startup is unchanged. Reports
record requested mode and observed visibility separately; neither mode measures
JS heap or GPU allocations or proves full-day/leak-free stability.

No GitHub settings, branch protection, credentials, commit history or user
repositories were changed for this work. Hosted rerun and installed-app native
Windows/Linux acceptance remain separate gates.

## Local regression checks

All runners were serialized with one Cargo compilation job and one test thread
(or one Playwright worker). These checks execute on macOS; they do not execute
Windows-only branches.

- Git 2.56 with HTTP support, `cargo test --workspace --exclude gitnova-desktop
  --locked -- --test-threads=1`: **424 passed, zero failed, one ignored**. Includes
  real authenticated loopback HTTPS Push/Fetch. This broad run preceded the final
  Clone argument conversion and additional fixture-identity changes; targeted
  checks below cover those follow-ups.
- Empty private home, Git 2.56, core `amend`, `merge_submodule_preview`,
  `patch_submodule_preservation`, `submodule_updates`: **46 passed**.
- Git 2.56, core `clone` after the final path conversion: **5 passed**.
- Native desktop IPC/PTY tests after final terminal changes: **6 passed**. The
  reader-EOF regression injects that event separately from a live Unix child;
  native PowerShell/ConPTY acceptance must still run on Windows.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
  The existing macOS build-script linker deployment-target advisory remains;
  it is not a new Rust lint failure.
- Frontend `npm run build`: passed TypeScript and Vite. Vite still reports a
  large main-chunk advisory; this change does not claim to resolve it.
- Frontend `npm test -- --maxWorkers=1`: **55 passed in 13 files**.

- Browser graph/terminal suite using real Rust core, Git 2.56 and an empty
  private home: **119 passed (16.8 minutes)** with one Playwright worker. This
  includes submodule pointer/lifecycle, worktree, rebase/merge topology, hunk,
  conflict, first publication, large Push and terminal-session UI workflows.

## Foreground visibility gate

Two foreground attempts were deliberately rejected by the visibility guard:

- [Initial attempt](../../benchmarks/native-foreground-interrupted-2026-10-31.json):
  53.4 seconds sampled, four completed cycles, then the WebView became hidden.
  The initial failure event did not include visibility, so its observed-visibility
  array contains only `visible`; the failure message still prevents acceptance.
- [Occluded attempt](../../benchmarks/native-foreground-occluded-2026-10-31.json):
  6.12 seconds sampled, zero complete cycles, both visible and hidden phase
  events. A temporary display-idle assertion did not eliminate occlusion. A
  read-only console query reported an unlocked session; it does not identify
  which window or system event caused the occlusion.

These are failed measurement attempts, not evidence of an application memory
failure or a completed foreground soak. The runner now records visibility on
failure events too. Foreground mode uses a temporary PID-bound display-idle
assertion, never changes display/lock preferences and never unlocks a session.
A quiet interval with the test window continuously visible remains necessary.

A background attempt was also [interrupted](../../benchmarks/native-background-interrupted-2026-10-31.json)
after 22.42 seconds when its native child exited with status zero during the
interactive handoff. It is retained as failed evidence, not a completed soak.

## Completed foreground native measurement

[Machine-readable report](../../benchmarks/native-foreground-2026-10-31.json):
**passed**, 906.54 sampled seconds, **78 complete cycles**, observed WebView
visibility exclusively `visible`. Environment: Apple M1 Pro, 16 GiB RAM,
macOS 26.7.1 (25G241), Git 2.39.2. This is the packaged debug build with optimized
first-party crates, not a release-profile or cross-product comparison.

Fixtures: 50,000 initial commits and 10,000 initial files in the large repository,
plus synthetic merge/bulk-change commits, 200 tracking branches, two submodules,
two linked worktrees and ten workspace locations. Search retains its 20,000-entry
scan bound; this workload does not establish complete 50,000-commit search coverage.
The report records the base revision, uncommitted-source flag and exact binary
SHA-256. Fixture creation/building were outside sampling; no other build/test
runner ran during measurement.

| Interaction | Samples | Median | p95 |
| --- | ---: | ---: | ---: |
| Switch repository through UI | 78 | 363.5 ms | 823 ms |
| Open commit detail through UI | 78 | 101 ms | 203 ms |
| Manual refresh through UI | 78 | 1,624.5 ms | 1,940 ms |
| Load older graph page | 26 | 716.5 ms | 813 ms |
| Current-repository search through UI | 13 | 850 ms | 1,132 ms |
| Workspace search through IPC | 13 | 2,436 ms | 12,614 ms |
| Two reviewed branch switches through IPC | 7 | 1,497 ms | 2,658 ms |

Workspace search includes its first indexing sample. IPC branch roundtrips are
two preview/apply operations on the clean fixture, not a single UI switch.
Provider/network latency and Push/Pull/Merge timing are outside this soak.

| Five-minute interval | Median native RSS | Median WebKit RSS |
| --- | ---: | ---: |
| 0–5 minutes | 221.48 MiB | 318.27 MiB |
| 5–10 minutes | 225.59 MiB | 396.41 MiB |
| 10–15 minutes | 223.36 MiB | 436.22 MiB |

Peak native RSS: **249.11 MiB**. Peak sampled WebKit RSS: **458.38 MiB**. Peak
sampled combined RSS: **701.16 MiB**. Peaks need not occur at the same instant.
After minute five, native slope was **−0.07 MiB/minute**, while WebKit slope
was **+8.34 MiB/minute**. Native memory stabilized in this interval; **whole-app
memory stability is not yet established**. Continued WebKit growth warrants a
heap/retention investigation and multi-hour rerun; RSS alone cannot identify a
JavaScript leak, browser cache retention or GC policy. Do not extrapolate a
full-day footprint or publish a leak-free claim from these samples.

All before/after fixture checks passed: HEAD, checked-out branch, staged entries,
refs and working changes remained identical. Index bytes were also identical
except on the one explicitly switched clean checkout, whose logical staging was
verified unchanged. Maximum observed DOM nodes: 2,184; rendered commit rows: 24.
These sampled counts support bounded graph rendering, not a bound on all JS heap.
WebKit attribution and shared-page/dynamic-child sampling limitations are in the
JSON. OS-store, installer and real-network acceptance remain separate.

## Bundle and final review

`npm run build:local` passed embedded-asset, ad-hoc signature and bundled-notice
presence checks. The same binary was copied to the canonical Desktop
`GitNova.app` only after confirming no app process was running; its signature,
embedded UI and binary hash were checked. No alternate Updated app was created.
This remains a local alpha/debug artifact, not a signed/notarized release.
Bundled notice presence does not establish complete dependency-license compliance:
the existing inventory still reports unresolved items.
