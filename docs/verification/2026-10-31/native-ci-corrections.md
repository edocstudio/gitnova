# Native CI corrections — 2026-10-31

Baseline: `fa5c2676be2c780d5730d5d80070da5688a39445` (merge of the platform increment). Read-only inspection of [run 41156924550](https://github.com/edocstudio/gitnova/actions/runs/41156924550) found the macOS desktop job passed and five other jobs failed. No workflow rerun, repository settings change, commit, push, merge or release was performed for these corrections.

## Observed failures and changes

| Native job | Evidence | Correction / remaining gate |
| --- | --- | --- |
| Linux core | Clippy `items_after_test_module` sees the Linux-only `read_background` after the test module | Move platform implementation before tests; keep `-D warnings` |
| Windows desktop | `unused_mut` in the default PowerShell command builder | Scope mutable setup to the Unix login-shell argument |
| Linux desktop | Confirmed amendment in a cloned IPC fixture has no committer identity | Set synthetic name/email and signing configuration in that temporary clone; source configuration is not inherited by Git clone |
| macOS core | Git 2.56 removes fetch tracking when deleting through a different configured push URL | Isolate native Push through an ephemeral command-local remote with no fetch refspecs; retain exact reviewed destination, leases, hooks, explicit tracking reconciliation and unchanged on-disk config |
| Windows core | Discard activity fixture returns `unknown`; the original assertion omitted its receipt summary | Normalize extended Windows index paths at the Git environment boundary, pin fixture line-ending policy locally, exercise spaces/apostrophe/Thai in its path and include the complete synthetic receipt on assertion failure. This candidate still requires a native Windows rerun; the original log alone does not prove which boundary caused the failure |

The push correction applies to branch Push, selected tag Push and remote branch deletion through the common writer. Credential selection still uses the original reviewed URL before the execution-only alias is generated. Pre-push hooks run and receive the temporary `gitnova-reviewed-…` remote name as their first argument and the exact resolved destination URL as their second. No configured remote is added, removed or overwritten. Tests check unchanged config, retained fetch-server refs, exact destination deletion and hook arguments.

Git 2.56's behavior was checked against upstream [remote tracking selection](https://github.com/git/git/blob/v2.56.0/remote.c) and [transport tracking updates](https://github.com/git/git/blob/v2.56.0/transport.c), and reproduced with temporary repositories using a locally built upstream Git 2.56.0. Native anonymous URL deletion removed `refs/remotes/origin/feature` even though the fetch server still had the branch. No user repository was used.

## Verification

Run one build/test runner at a time, compilation jobs set to 1 and Rust tests serialized. Local host is macOS; Linux/Windows execution is not simulated or claimed by local checks.

- Apple Git 2.39.2: activity 4, branch/ref/push 22 and remote deletion 6 tests passed. The expanded activity/remote-deletion regressions passed again (10 tests).
- Scoped git/hosting/desktop Clippy with all targets and `-D warnings` passed on macOS. The existing deployment-target linker warning remains.
- Complete upstream Git 2.56.0 toolchain: activity 4, ref/push 22, remote deletion 6 and tag publication 2 tests passed (34 total). The native desktop IPC/PTY suite passed with that Git too (5 tests), including the cloned fixture's amendment.
- Real local HTTPS Push/Fetch credential-helper test passed with Apple Git 2.39.2 (1 test), verifying authentication and unchanged persistent config after execution-only push aliasing. Only synthetic credentials and loopback traffic were used. Its initial sandbox attempt failed before the fixture server returned a listening port; the approved run outside the sandbox passed.
- Formatter and authored whitespace checks passed. No full browser suite or full workspace regression was repeated for this follow-up; the previous platform report records the broader baseline checks.

The initial Git 2.56 probe deliberately built only the executable. That incomplete toolchain lacked default hook templates and the external submodule script, causing five setup failures in the broader ref/push suite. A complete private-prefix build is used for subsequent regression results; those setup failures are not app regressions or passing evidence. This test Git is isolated under a temporary directory and does not replace the installed Git or ship with GitNova.

## Acceptance still open

Run native CI again with the corrections. In particular, Windows discard must report verified success and preserve the saved original bytes/index across restart; its cause is not closed solely by a passing Mac test. Later Windows/Linux graph, OS-store and installer steps did not run in the baseline failed jobs, so they remain unverified. Installed-app interactive acceptance and foreground/full-workday performance/RAM work also remain open. No new performance measurement is inferred from build/test time.
