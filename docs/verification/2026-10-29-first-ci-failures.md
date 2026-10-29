# First hosted CI failures — 2026-10-29

## Evidence

Read-only GitHub job/step/log retrieval for run [40811611954](https://github.com/edocstudio/gitnova/actions/runs/40811611954):

- Ubuntu job `123243003716`: unused hosting `credential_account`, Jenkins `account_key` and `credential_account` fail `-D warnings`.
- Windows job `123242984999`: `stash.rs` directory builder is unnecessarily mutable on that target.
- Desktop job `123242985251`: native IPC queue saturation assertion fails with `NoPermits`; frontend Vitest 45 tests passed in the historical run.
- macOS core job `123242985216`: cancelled, not evidence of a separately diagnosed test failure.

No account secrets or user Git repositories were used for testing. Existing uncommitted source/docs work is preserved. No CI setting, workflow YAML, ruleset, dependency or lockfile changes are needed for these identified failures.

## Local verification

- `cargo test -p gitnova-desktop --locked -j1 --offline -- --test-threads=2`: both native IPC tests passed. This deliberately permits two test threads while the test-only mutex isolates the shared global queue.
- `cargo clippy --workspace --all-targets --locked -j1 --offline -- -D warnings`: passed on macOS, including desktop, hosting and test targets.
- `cargo fmt --all --check` and `git diff --check`: passed.

Logs: `/tmp/gitnova-m64-desktop.log`, `/tmp/gitnova-m64-clippy.log`. Tests ran in temporary Git repositories. The full workspace test suite was not re-run for this small correction; prior M63 provider/UI checks are recorded separately. Linux/Windows hosted checks have not been re-run; local macOS verification cannot certify those targets.

The canonical local GitNova.app was built and verified together with M63; see its packaging record. No commit, push or hosted workflow rerun was performed.
