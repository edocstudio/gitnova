# CI regression verification — 2026-10-29

The supplied log listed 11 desktop failures plus a Windows Clippy unused-import error. The initial-branch `hint:` text is advisory Git output, not a job failure.

## Changes confirmed from source and reproduction

- Gate the Unix-only wait test module, including its imports, with `cfg(all(test, unix))`. No `allow(unused_imports)`, disabled warning policy or workflow bypass was added. Windows production code is unchanged.
- Update assertions for the existing Stage all heading and complete-history wording on first publication. Git object/ref/config/file checks remain in those tests.
- Exercise language changes through Settings → General; removed `#language` in the workspace picker is not a valid user flow anymore.
- Preserve the stale staged-review warning while the selected file diff reloads. Reproduced the old disappearing alert locally; the working-changes test passes after the fix.
- Retain bounded per-workspace repository search/page state when Settings unmounts the picker.
- Remove the redundant native dialog-close feedback callback; controlled Close/Escape/backdrop handlers remain. Verify focus restoration before issuing the next keyboard shortcut, rather than adding a delay or retry.
- The stash geometry assertion already recognizes themed stash connector strokes and retains ordinary-lane separation checks, including contracted Graph width.

Some scenarios (workspace search, branch deletion, large first publication and long-history Push) were listed without their full hosted error details. Their original hosted failure cannot be assigned a proven cause from that summary alone. They succeeded against the current source in local real-Git browser fixtures; no speculative Git-write change was made to force them through.

## Actual checks

All build/test runners were sequential. Cargo used one compilation job and the focused wait test used one test thread. Playwright used one worker. Git writes were confined to temporary repositories; no real credentials, company repository, remote permission or GitHub setting was used or modified.

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --locked -j1 --offline -- -D warnings`: passed on macOS.
- `cargo test -p gitnova-git wait::tests -j1 --offline -- --test-threads=1`: the one matching wait unit test passed; other tests were filtered out.
- `npm run check`: passed.
- `npm test -- --maxWorkers=1`: 53 passed across 13 files.
- Focused browser follow-ups verified all 11 scenarios from the supplied failure list. The final `CI=true` 11-case selection passed 10; the remaining picker case exposed a test keyboard/focus timing race. After explicitly waiting for restored Settings-trigger focus, `CI=true ... --grep 'repository picker opens the selected page|FAQ is searchable' --repeat-each=3` passed all 6 executions. No fixed sleeps, removed scenarios or timeout increases were used.
- Final local packaging validates TypeScript and embedded assets separately. `git diff --check` passed.

Logs: `/tmp/gitnova-m70-clippy.log`, `/tmp/gitnova-m70-wait.log`, `/tmp/gitnova-m70-ui.log`, `/tmp/gitnova-m70-browser.log`, `/tmp/gitnova-m70-browser-followup.log`, `/tmp/gitnova-m70-browser-final.log`, `/tmp/gitnova-m70-browser-ci.log`, `/tmp/gitnova-m70-browser-repeat.log`.

`CI=true` is a local browser-runner mode; it is not a hosted Windows/Linux run. No complete browser suite or full native suite was run for this change. Hosted runners must validate the new commit after publication; rerunning an older commit still executes the older source.

`npm run build:local` passed. The existing `target/debug/bundle/macos/GitNova.app` was updated (71.57 MiB), with embedded UI and local bundle verification passed. This remains a local debug, ad-hoc signed build, not a notarized distribution. Packaging log: `/tmp/gitnova-m70-build.log`. No commit, push, PR, merge or release is performed by this task. Dependencies, lockfiles and `.github/workflows/ci.yml` remain unchanged by these fixes.
