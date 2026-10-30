# Connection commit identities — 2026-10-30

## Diagnosis and authorized scope

The reported commit used the Git name/email from the user's existing global
configuration. Selecting a different hosting connection authenticates Git/PR/CI;
it does not itself determine commit authorship. The requested fix adds explicit
commit identities to GitNova. No user repository config, global Git config,
existing commit, published history or provider setting was changed by this task.
No real credential was used for testing.

## Implementation

See [milestone 74](../milestone-74.md). Identity rules use normalized provider/host
keys, bounded local JSON storage, validated names/emails and native Git argument
arrays. Selection follows origin/sole-fetch-remote host boundaries, including
supported SSH URLs, without an API lookup. Nonmatching repositories use native
Git configuration. Native author/committer-specific config keys are overridden
for selected commands too, so an old author.email cannot override the chosen
connection identity. Removing an override restores native Git defaults.

The existing Settings cards host explicit Save/Use Git configuration controls;
there is no new screen or navigation. Composer/merge contexts show the committer.
Identity/config changes invalidate commit previews. Amend/replay preserve original
authors. The core scope covers corresponding previews, writes and recovery/read
paths; it does not mutate process-wide environment or user Git config files.

## Verification performed locally

All Git-writing tests use newly created temporary repositories. One runner at a
time; Cargo uses one build job and integration suites one test thread.

- New identity integration suite: **8 passed**. Includes independent hosts,
  unmatched/lookalike-host fallback, author/committer-specific config, concurrent
  connections, persistence after restart, validation/clear, stale identity/remote,
  original amend/replay authors, merge/rebase/cherry-pick committers, consistent
  snapshot tokens, linked-worktree isolation and bounded metadata reads/writes.
  Oversized saves preserve the previous valid settings. Config bytes remain unchanged.
- Existing integration regressions: `initial_publication` **10 passed**, `amend`
  **13 passed**, `merges` **24 passed**, `rebase` **12 passed**.
- Native Tauri IPC boundary test: **1 passed**, including new identity commands,
  persisted metadata and external-origin refusal. No provider/Keychain calls are
  needed for identity operations.
- Browser Settings/FAQ/staging regression group: **4 passed**; the new two-host
  identity case initially navigated before post-commit refresh completed. The
  test now uses the actual enabled picker button instead of an early shortcut;
  no timeout increase or sleep was added. The new case then **passed**.
- Final browser group: **5 passed** (1.5 minutes), covering two-host real commits,
  unchanged Git config, global connections, merge conflict editing, stash and
  rebase conflict flows. Provider account reads are emulated in this identity
  scenario to avoid real accounts; all author/settings/commit calls use Rust.
- Frontend TypeScript/production build and **53 unit tests** passed. Existing
  chunk-size advisory remains unsuppressed.
- Workspace/all-target Clippy with warnings denied, formatting and whitespace
  checks passed.

The complete hosted CI matrix, real provider authentication and full browser
suite were not rerun. This is local macOS verification, not Linux/Windows
certification. Existing published commits and their deployment attribution remain
unchanged; configuring an identity affects future operations.

## Packaging

The final local debug macOS bundle was built and installed over the existing
`~/Desktop/GitNova.app`. Ad-hoc signature verification, matching executable
SHA-256 and embedded asset verification passed. The app was not opened into a
workspace. This is not a notarized release. No commit, push, PR, merge or release
is performed by this task.
