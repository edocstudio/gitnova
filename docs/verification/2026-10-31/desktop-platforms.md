# Windows/Linux desktop preparation — 2026-10-31

Status: implementation and macOS regression checks completed; Windows/Linux hosted CI and manual native acceptance are pending. No Windows/Linux binary was built or executed on this Mac. No account credentials, user project repository writes, commits, pushes, PRs, releases or GitHub settings changes were used.

## Changes

- Shared OS credential-store adapter: preserve macOS service/account identity, add Windows Credential Manager and Linux Secret Service through target-specific keyring backends. No plaintext fallback. GitHub/GitLab and Jenkins account flows use the same adapter, keeping provider/origin and controller/username isolation.
- Existing native account actions use masked Windows/Zenity input on those platforms. Tokens are returned over a pipe, not argv/environment or a frontend draft. Native prompt cancellation and invalid prompt data remain errors.
- Enable process-local HTTPS credential helper on all three desktop platforms. Normalize extended drive/UNC executable paths for Git for Windows' shell; preserve Unix paths verbatim.
- Linux background credential reads search only the exact, unique, already-unlocked item with a zero prompt timeout. They never call unlock/create. Locked/ambiguous/unavailable stores fail closed. Real Linux behavior still requires its native credential-store CI check.
- Open validated HTTPS links through ShellExecuteW on Windows or bounded xdg-open on Linux; retain the macOS opener. No generic shell command is exposed through IPC.
- Extend the existing Unix PID/inode/native-state stale-lock checks to Linux. Windows adds process-handle/file-ID proof and handle-based archival with competing writes/deletes denied. Native Windows execution is pending; no PID-text-only deletion path was added.
- Native Windows NSIS and Linux Debian/AppImage configuration; host-specific build command, prerequisites and target-specific license inventory checks. Existing macOS local build path remains separate.
- Native CI jobs compile/lint, run IPC/PTY and isolated synthetic credential-store tests, build installers and check packaged attribution. CI artifacts are alpha, unsigned and not public releases. Existing macOS desktop job is retained. Core Linux CI installs the new D-Bus development prerequisite.
- EN/TH visible account text explains OS-managed storage. Platform guide, README, architecture and contribution instructions agree. Incidental search changes are rustfmt-only, required by the repository formatter check; no search behavior changed.

## Checks executed locally

One runner at a time, Cargo build jobs set to 1; Rust tests used one test thread.

| Check | Result |
| --- | --- |
| Scoped Clippy for gitnova-hosting, gitnova-git and gitnova-desktop, all targets, `-D warnings` | Passed on macOS; existing deployment-target linker message remains a warning |
| `cargo test -p gitnova-hosting -p gitnova-git --locked -- --test-threads=1` | 48 tests passed; 1 OS credential-store acceptance test intentionally ignored |
| `cargo test -p gitnova-core --test stale_lock --locked -- --test-threads=1` | 5 passed, temporary repositories only |
| `cargo test -p gitnova-desktop --locked -- --test-threads=1` | 5 passed, including real PTY and native IPC fixtures |
| Frontend Vitest, one worker | 13 files / 55 tests passed |
| Prerequisite checker Node tests | 9 passed, including rejection of Git older than 2.38 |
| `npm run build` | Passed; existing large-chunk advisory remains |
| `cargo fmt --all -- --check` | Passed |
| Windows/Linux overrides merged with base config, validated against installed Tauri JSON schema | Passed (AJV configured with non-Unicode regex parsing for the schema's escaped-character pattern) |
| Workflow YAML parsed with Ruby YAML | Passed |
| macOS license inventory regeneration | 392 components, 247 collected texts, 51 unresolved items, auditComplete=false |
| Authored diff whitespace | Passed; generated verbatim third-party evidence retains one upstream trailing space, preserving its original bytes and hash |

The last Linux-only noninteractive Secret Service path was inspected against the installed `dbus-secret-service` and `keyring` APIs; it cannot be type-checked or exercised as Linux code on the current Mac target. The hosted Linux job must validate it. The Windows Credential Manager, native dialog, shell opener, executable-path helper test and installers likewise require native CI/manual acceptance.

## Remaining acceptance

Run the new native CI jobs for this exact commit, inspect failures rather than bypassing them, and manually test installed-app startup, account entry/cancel/locked store, HTTPS/SSH, clipboard, conflict handling, repository paths with spaces/non-ASCII characters, terminal cwd and long sessions on Windows/Linux. Retain the stated alpha/unsigned limitations and complete the distribution-license checklist before public binary distribution. Windows stale-lock, edited replay, descendant timeout cleanup and directory-ID checks are implemented but require the native acceptance jobs.

## Follow-up: complete feature scope

The owner clarified that every current macOS feature must work, not merely produce installers. The follow-up removes the Windows edited-replay rejection, enables shared ownership/recovery checks with Windows-native file/process proof, replaces timestamp-only Windows directory tokens, enables long Git paths process-locally, and owns Windows Git subprocess trees through Job Objects. Existing edited-replay and stale-lock graph cases no longer skip Windows/Linux. The full graph suite is configured on both native CI runners, alongside all core suites. See [current-feature acceptance map](../../platform-parity.md).

The first targeted Mac run passed 9 hunk tests, 25 rebase tests (including edited replay and restart/abort) and 5 stale-lock tests. The full workspace regression run excluding the desktop completed on macOS: 423 passed, 0 failed, 1 isolated OS-store acceptance test ignored (60 suite/doc-test summaries). After routing the isolated merge preview through the same child wrapper, scoped Clippy for git/core/hosting/desktop with all targets and `-D warnings` also passed; the existing macOS deployment-target linker warning remains. Native platform results remain pending.

Final UI follow-up: both selected graph tests passed against the real Rust bridge (stale-lock recovery and edited interactive replay). Native desktop tests passed again (5/5). The Linux installer CI baseline is Ubuntu 24.04; the doctor now rejects Git older than 2.38 because isolated merge previews use modern `merge-tree --write-tree` (documented in [Git 2.38](https://git-scm.com/docs/git-merge-tree/2.38.0)). This avoids treating an older distribution Git as sufficient for full feature parity.

Final lightweight checks: Vitest 13 files / 55 tests passed again; `cargo fmt --all -- --check` passed; workflow YAML parsed; local Markdown links checked (410, zero missing); authored diff whitespace passed. Regenerated macOS license inventory still reports 392 components, 247 collected texts and 51 unresolved items (`auditComplete=false`); current lockfile hashes verified. No Windows/Linux native execution or app installation was performed.
