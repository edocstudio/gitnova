# Large Push review verification — 2026-10-29

## Scope

Tests use disposable local Git repositories and a bare remote. No company remote, token, user Git configuration or project history is modified. This verifies exact normal branch publication, not remote object-transfer size or live service permissions.

## Executed checks

- `cargo test -p gitnova-core --test ref_and_push -j1 --offline -- --test-threads=1`: 21 passed. The added fixture creates 522 reachable commits with fast-import. New-branch review reports 522 and existing-branch review 521, each with exactly 500 newest records. Both publish the full tip/history. A destination created after review rejects the stale token before publishing. Local HEAD, index, config and file content remain unchanged. Existing hook-preview, destination-race, divergence, force/refspec/tag isolation, worktree, submodule and timeout tests also pass.

- `npm run check`: passed.
- `npm test -- --maxWorkers=1`: 47 tests in 11 files passed.
- `npm run test:e2e -- --workers=1 --grep 'new remote branch with long shared history'`: 1 passed. Real Rust bridge and disposable Git/bare repositories verify a 521-commit new branch, exactly 500 displayed subjects, full-history/truncation explanations, enabled explicit confirmation, complete remote history after push, unchanged remote main and clean local working state. Browser IPC is emulated, not a native WebView acceptance test. The review screenshot was visually inspected; confirmation remains visible.
- `cargo clippy -p gitnova-core -p gitnova-git --all-targets --locked -j1 --offline -- -D warnings`, `cargo fmt --all --check` and `git diff --check`: passed.

Logs: `/tmp/gitnova-m66-push.log`, `/tmp/gitnova-m66-ui.log`, `/tmp/gitnova-m66-browser.log`, `/tmp/gitnova-m66-clippy.log`. No full-workspace test suite or live SSH/HTTPS remote test was run.

`npm run build:local` completed after the owner closed GitNova. The canonical `target/debug/bundle/macos/GitNova.app` (71.16 MiB) was updated, with signature and embedded UI verification passing. Log: `/tmp/gitnova-m66-build.log`. This is a local debug/ad-hoc bundle, not a notarized release. The bundled UI was not launched against a company remote. Runners remain sequential. No commit, push to the project’s own remote, PR or release is performed.
