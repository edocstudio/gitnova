# Auto fetch verification — 2026-10-29

Runners were sequential with one Cargo job/test thread or browser worker. Git writes used disposable repositories only. No company remote, live Keychain token or account setting was used.

- `cargo test -p gitnova-core --test remotes -j1 --offline -- --test-threads=1`: 13 passed. Added cases confirm remote-tracking update while HEAD/index/staged and unstaged data/config/FETCH_HEAD are preserved, no tags or reference-transaction hooks run, no per-minute receipt directory appears, pending merge and app operation locks prevent fetch, upstream remote beats origin and no-remotes skips. Prior reviewed remote checkout/fetch failure/config race checks still pass.
- `npm run check`: passed.
- `npm test -- --maxWorkers=1`: 50 passed across 12 files, including allowed/default/disabled interval values, independent per-repository cooldown, exponential failure cap, bounded schedule eviction and prior UI-domain tests.
- `npm run test:e2e -- --workers=1 --grep 'auto fetch defaults|auto fetch failures'`: two passed. Virtual-clock tests and real Rust/local Git transport cover no early fetch before one minute, refreshed remote graph with original selected commit/HEAD/index/dirty files preserved, hidden-window pause/resume, Settings pause, persisted Off, 30-second interval and failed-local-remote exponential retry without dialogs. Browser/native IPC is emulated; this does not certify native WebKit hidden-window events or live SSH/HTTPS authentication. Unrelated browser tests explicitly disable timers to prevent fixture host network access.
- `cargo clippy --workspace --all-targets --locked -j1 --offline -- -D warnings`, `cargo fmt --all --check` and `git diff --check`: passed.

Logs: `/tmp/gitnova-m67-remotes.log`, `/tmp/gitnova-m67-ui.log`, `/tmp/gitnova-m67-browser.log`, `/tmp/gitnova-m67-clippy.log`. No full app/native test suite or live network benchmark was run. Static compilation validates the macOS helper API; quiet Keychain access and real private-provider authentication remain live acceptance checks. No universal performance or atomic-fetch claim is made.

`npm run build:local` passed after the owner closed the app. The existing `target/debug/bundle/macos/GitNova.app` was updated (71.28 MiB); local bundle and embedded UI asset verification passed. This is a local debug, ad-hoc signed build, not a notarized distribution. Packaging log: `/tmp/gitnova-m67-build.log`. The app was not launched for live private-provider acceptance testing.

No commit, push, PR, hosted CI rerun or release is performed by this task.
