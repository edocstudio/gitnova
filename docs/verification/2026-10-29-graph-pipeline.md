# Graph pipeline verification — 2026-10-29

## Scope

GitLab graph CI now displays the exact commit’s latest pipeline state/ID, ref tooltip and an existing-context-menu link. These checks do not certify access to a private company instance or benchmark real network latency. No developer repository files, refs or configuration were changed by Git operations.

## Executed checks

Runners were sequential with one Cargo job/test thread or one browser worker.

- `cargo test -p gitnova-hosting --lib -j1 --offline -- --test-threads=1`: 34 passed. Local HTTP fixtures cover exact commit/pipeline SHA, nested namespaces, host base paths, missing web URL, running/pending/passed/failed/canceled/skipped/manual/unknown states, aggregate-status fallback, and refusal of mismatched SHA, malformed ID, cross-project/host/ID, credential-bearing or query-bearing URL. Existing GitHub/provider tests also pass. Fixture server expects only the lightweight commit request, without jobs/logs.
- `npm run check`: passed.
- `npm test -- --maxWorkers=1`: 46 tests across 11 files passed. Includes identity/stale-cache preservation, aggregate fallback without fabricated pipeline ID, unknown/failed labels and rejection of wrong-SHA evidence.
- `npm run test:e2e -- --workers=1 --grep 'global CI follows real fetch hosts'`: 1 passed. Uses real Rust core and temporary Git repositories for host routing; the provider boundary is simulated, not a live GitLab/GitHub service. Checks running → passed pipeline badge, ref tooltip, keyboard context menu and exact link dispatch, hidden-window polling pause, late-response isolation and GitHub failed-state display. External-browser opening is intercepted; Git HEAD/status remain unchanged.
- `cargo clippy -p gitnova-hosting --all-targets -j1 --offline -- -D warnings`, `cargo fmt --all --check` and `git diff --check`: passed.

Logs: `/tmp/gitnova-m62-hosting.log`, `/tmp/gitnova-m62-ui.log`, `/tmp/gitnova-m62-browser.log`, `/tmp/gitnova-m62-clippy.log`. The GitLab screenshot in ignored Playwright test results was visually inspected for the pipeline badge and aligned CI/Author columns.

## Packaging

`npm run build:local` completed after the owner closed the app. It updated only `target/debug/bundle/macos/GitNova.app`; ad-hoc signature verification and embedded UI `--verify-bundle` passed. Build log: `/tmp/gitnova-m62-build.log`. This remains a local debug/ad-hoc bundle, not a notarized release. The bundled app was not launched for a live private-host check. No commit, push, PR, release or account-setting change is part of this task.
