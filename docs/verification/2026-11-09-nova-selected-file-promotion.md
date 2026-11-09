# Selected-file promotion CLI verification

Date: 2026-11-09. Version: 1.0.1. Local verification: macOS Apple Silicon.

## Behavior

The existing embedded Terminal supplies `nova promote`. Exact selected whole
file snapshots from a remote source branch are applied to the remote target tree
in an isolated bare repository. Publication creates a new review branch; provider
integration can then create a same-project GitHub PR or GitLab MR. Preview is the
default. Publication and request creation require separate typed confirmations.

The active checkout, HEAD, working files and staging are preserved. No original
repository hooks or checkout filters execute in this isolated workflow. Selected
destination file contents can be replaced by the source snapshot: this is not a
merge or cherry-pick. Review the resulting request before merging.

## Checks actually run

- Cargo formatting check: passed.
- Clippy for gitnova-git, gitnova-core and gitnova-desktop, all targets, locked,
  warnings denied: passed.
- Git promotion integration: 6 passed. Includes selected snapshots, excluded
  target files, dirty checkout/index preservation, explicit deletion, stale tips,
  unsafe paths, concurrent publication and retained recovery on failure.
- Core library and CLI integration: 5 unit tests and 1 integration test passed.
  Includes preview, cancelled confirmation, publication and exact host identity.
- Native Terminal suite: 4 passed; includes session command availability, exact
  folder, output lifecycle and session cleanup.
- Hosting adapter suite: 41 passed, 1 ignored, using local mock servers.
- Terminal welcome frontend suite: 2 passed.
- `npm run build:local`: passed, including TypeScript/Vite and native bundle.
- Desktop/GitNova.app overwritten in place; ad-hoc signature and installed
  `--nova --help` verified.
- Installed native executable promotion smoke: passed 5 checks for selected
  source content, excluded target content, unchanged HEAD, unchanged index bytes
  and preserved dirty file. Used only disposable repositories/local bare remotes;
  removed the task-owned recovery directory after inspection.
- Diff whitespace check and the added CLI documentation links: passed.

## Limits

No real provider credentials were used to create a request, and no developer
project branch was published. Live provider permissions/rules remain unverified
by this run. Windows/Linux execution of the new CLI is not verified here.
Existing hosting adapters have mock coverage; this is not live end-to-end PR/MR
verification.

File selection uses literal paths after `--`, not an interactive picker. The
initial scope is 1–200 regular files, same-project requests, and identical fetch
and push destinations. Submodule pointers, directories, symlinks and .gitmodules
are excluded. Recovery repositories reside in OS temporary storage and may be
cleaned by the OS; they are not durable backups. Publication and request creation
are separate effects with explicit partial-failure reporting and no automatic
retry/rollback.

The installed bundle is a local debug/ad-hoc build, not notarized. Existing
third-party license review still reports 67 unresolved entries; no complete
license-compliance claim is made. No source commit or push was performed.
