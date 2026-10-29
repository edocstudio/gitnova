# Milestone 70 — Hosted check failures and UI state regressions

The supplied Windows Clippy output identified a test-only wildcard import whose only consumer was Unix-specific. The entire `wait` test module is now gated by both `test` and `unix`. Production waiting behavior is unchanged; warnings remain errors and no lint suppression or workflow bypass is added.

The supplied desktop log listed 11 failing scenarios. Two expectations referred to obsolete UI text: the unstaged heading now exposes Stage all, and first publication describes complete branch history rather than claiming every reachable commit is a newly uploaded commit. Language tests now operate through Settings → General rather than a removed workspace-picker language field. The reserved stash lane test recognizes theme-variable stroke colors while retaining geometric separation assertions.

Actual UI state fixes preserve a stale-review warning while per-file diff loading continues, retain bounded per-workspace repository search/pagination state while opening Settings, and avoid feeding the dialog's queued native close event back into an already reopened Settings dialog. Escape, backdrop and Close settings still use the controlled close handler. The FAQ keyboard test verifies focus restoration before reopening.

No Git write semantics, credentials, dependencies, lockfiles, repository permissions or workflow rules were changed for these fixes. Push/search/delete/publication scenarios are checked against temporary repositories; local success does not replace hosted Windows/Linux acceptance. Historical workflow runs still refer to their original commit; publish the updated branch to trigger checks of the fix.
