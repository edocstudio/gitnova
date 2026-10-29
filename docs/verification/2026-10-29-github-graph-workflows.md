# GitHub graph workflow verification — 2026-10-29

## Executed checks

Verification ran sequentially with one Cargo job/test thread or browser worker. No live provider token, company API or developer Git write was used.

- `cargo test -p gitnova-hosting --lib -j1 --offline -- --test-threads=1`: 36 passed. Local HTTP fixtures cover workflow display number versus API ID, name/ref, exact SHA and connected-project URL, multiple workflows, latest run per workflow, refusal of wrong SHA/project/query-bearing URL and Actions permission failure with incomplete aggregate evidence. Request assertions require `head_sha` and `per_page=20`. Existing GitLab and GitHub check/status tests pass.
- `npm run check`: passed.
- `npm test -- --maxWorkers=1`: 47 passed across 11 files. Covers GitHub display run number, aggregate/workflow status disagreement, multiple workflows, incomplete evidence and prior GitLab identity/caching checks.
- `cargo clippy -p gitnova-hosting --all-targets -j1 --offline -- -D warnings`: passed.
- `npm run test:e2e -- --workers=1 --grep 'global CI follows real fetch hosts'`: 1 passed. Real Rust core and temporary Git repositories resolve host scope. Provider responses are fixtures. Checks GitLab running/passed number, GitHub `Failed · #1`, workflow name/ref tooltip, keyboard context menu and exact workflow link dispatch; hidden polling pauses and late responses do not cross repository boundaries. The browser-open transport is intercepted rather than opening a real provider. Temporary Git HEAD/status remain unchanged.
- `cargo fmt --all --check` and `git diff --check`: passed.

The GitHub graph screenshot in ignored Playwright test results was visually inspected. This does not certify live account permissions or actual network latency. No job/log/artifact reads or writes are issued by the new graph workflow-list read.

Logs: `/tmp/gitnova-m63-hosting.log`, `/tmp/gitnova-m63-ui.log`, `/tmp/gitnova-m63-clippy.log`, `/tmp/gitnova-m63-browser.log`.

## API reference

[GitHub REST workflow runs](https://docs.github.com/en/rest/actions/workflow-runs) documents exact `head_sha` filtering and the separate run `id`, `run_number`, `workflow_id`, `head_branch`, name and result fields. GitNova validates and bounds responses instead of treating nearby workflow evidence as belonging to the selected commit.

## Packaging

`npm run build:local` completed after the owner closed the app. It updated only `target/debug/bundle/macos/GitNova.app` (71.16 MiB); signature and embedded UI verification passed. The same bundle includes the M64 CI-source/test corrections. Log: `/tmp/gitnova-m63-build.log`. It remains a local debug/ad-hoc bundle, not a notarized release; the bundled UI was not launched against a live provider account. No commit, push, PR, release or account-settings change is part of this task.
