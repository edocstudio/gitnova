# Workspace repository management

Repository entries can be managed through the existing workspace picker: use
the row's actions button or right-click the row. The inline form changes a
workspace-local display name, reconnects an entry to an existing repository
root, or confirms removal from that workspace. It does not move/delete files,
edit Git configuration, change refs, or remove other workspace registrations.
Unavailable checkouts can still be renamed or removed.

Path changes validate discovery before saving, reject duplicate checkouts and
folders inside a repository, preserve pin/order, and assign fresh membership
identity to avoid reusing another checkout's graph/selection context. Linked
worktrees keep shared repository identity while reconnecting only the requested
membership. Missing paths and invalid names leave the catalog unchanged.

The UI blocks reconnect/removal with a Terminal session or unsaved commit/file
draft for that membership. Label changes remain available and update Terminal
titles. Removal requires confirmation and selects another entry when available.
The existing five-entry pagination, search and bounded visible status reads stay
in place. UI copy is available in English and Thai.

Validation on macOS:

- Rust `membership_management` integration suite: 4 passed. Disposable real Git
  repositories cover workspace isolation, restart persistence, missing folders,
  relocation, invalid roots/names, duplicate destinations, linked worktree reuse,
  adding a relocated linked checkout to a new workspace, bare repositories, and
  preservation of HEAD, index bytes and working edits.
- Existing worktree creation/removal regression suites: 11 and 13 passed.
- TypeScript check passed.
- Frontend unit suite: 21 files, 79 tests passed.
- Clippy for core and native desktop, all targets, with warnings denied passed
  with `MACOSX_DEPLOYMENT_TARGET=13.0`, also used for local packaging.
- Targeted Playwright suite: 4 passed. Covers the two existing picker pagination/
  search/language regressions and the new rename → invalid path → reconnect →
  remove/reload journey, plus protection of a commit draft. Transport uses the
  real Rust core bridge and temporary repositories; the native Tauri transport
  is not exercised by these browser tests.
- Removal confirmation screenshot inspected; no new sidebar or navigation page.
- React review covered cleanup, async unmount handling, input labels, busy/error
  state and bounded status work. No runtime dependency added.

Windows/Linux GUI behavior and the full application regression suite were not
run for this change. Tests do not modify the developer's project repositories.

Delivery: version script and its test synchronized 1.0.5 metadata, including
README status/badge. Attribution inventory was regenerated (413 components,
250 license texts, 67 unresolved entries; audit remains incomplete). The local
macOS debug/ad-hoc bundle built successfully and replaced Desktop/GitNova.app.
Installed version 1.0.5, matching executable SHA-256, code signature and
`--verify-bundle` checks passed. This is not a notarized release build. No commit,
push or release publication was performed. `git diff --check` passed.
