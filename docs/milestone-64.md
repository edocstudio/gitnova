# M64 — First hosted CI failure diagnosis

Read-only inspection of GitHub Actions run [40811611954](https://github.com/edocstudio/gitnova/actions/runs/40811611954), commit `b5ae7ef00a03e41af2009dd6bf4581952eb9992e`, identified source/test failures rather than missing repository secrets or permissions:

- Ubuntu strict Clippy rejected three private Keychain helper functions compiled but unused outside macOS.
- Windows strict Clippy rejected a mutable directory builder whose mutation exists only under Unix.
- Desktop Vitest/build/Clippy passed; its native IPC test failed with `NoPermits` when intentionally claiming the entire process-global command queue while another test used it.
- The macOS core job was cancelled by the matrix, so that run does not establish a macOS core test failure or success.

Platform helpers now compile only where used (or where directly tested). Directory permissions remain mode 0700 on Unix; only the mutable binding moves inside the Unix branch. Both native IPC tests share a test-only mutex because one deliberately exhausts the global queue. Production queue capacity, error behavior and scheduling are unchanged. CI warnings remain errors; tests are not skipped and workflow behavior/settings are unchanged.

The owner must publish a new reviewed commit to run these changes on hosted platforms. Re-running the old commit continues to use old source. No push, hosted rerun, repository setting or ruleset change is performed by this task. Required checks should be selected after a successful eligible run; see [GitHub troubleshooting](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks).

See [verification](verification/2026-10-29-first-ci-failures.md) for actual local checks and platform limits.
