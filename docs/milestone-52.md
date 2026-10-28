# M52 — Repository remote cards, connection selection and footer context

Persistent remote configuration remains in Settings. Repository remotes now uses the same card and Add-form structure as global Connections: configured remote cards show the exact name and safe configured fetch URLs; Add repository remote opens the scoped form. The local repository and linked-worktree scope stay explicit, separate from shared company/host accounts.

For first publication, select a saved global Git-host connection or enter a custom HTTPS/SSH/local URL. A connection supplies only the validated HTTPS host/base path; the user must enter the owner/group/repository path. The UI does not guess the hosted project from the local directory, copy account tokens into URLs, create a hosted repository, contact a remote or push while saving. Native Git credentials/SSH remain separate from optional hosting/CI accounts. Existing preview, native configuration lock, recovery copy and explicit save remain unchanged.

The footer credits the approved copyright holder and Apache-2.0 license, matching NOTICE. It shows configured remote names, the current branch's actual upstream, and a safe configured fetch URL when unambiguous. No upstream means no inferred active remote; multiple remotes are listed. A different configured push remote is identified separately. Long URLs are bounded visually and remain available in the tooltip; narrower layouts hide the long URL before the remote/upstream identity. Footer reads are local, bounded and refreshed on repository/context changes, focus, manual refresh or a confirmed configuration change. They never fetch or poll the network.

Remote URL display metadata comes from one bounded config snapshot, not one subprocess per remote. Credential-bearing/custom/unsupported URLs are omitted from display; remote names remain available. The display explicitly describes configured fetch URLs rather than claiming to show every effective rewrite or push endpoint. Native Push still independently reviews its actual destination.

## Verification

All writes use disposable repositories. Existing first-publication integration checks and the 383-file browser publication flow passed after the card change. A new native check verifies multiple configured URLs, omission of an embedded synthetic credential, unchanged config/index and no FETCH_HEAD. A real-core browser flow selects a self-hosted connection with a base path, reviews and saves an explicit nested project, verifies cards, then checks footer state with no remote, one remote without upstream, multiple remotes, and an actual configured upstream. No hosted network operation is needed.

Frontend/catalog checks and an explicit-project URL unit test cover valid nested paths and rejection of traversal, injected URLs, query strings and fragments. TypeScript, strict Git/core/desktop Clippy and formatting are also checked. Final packaging details are recorded after the local build.

Recorded checks: 9 existing initial-publication integration cases plus the new remote-display case passed; 38 frontend unit/catalog tests passed; TypeScript and formatting passed; strict Git/core/desktop Clippy passed. Two single-worker real-core browser flows passed (383-file first publication and remote cards/footer), with the cards/footer flow rerun after quiet-refresh coalescing and human-readable upstream labels. Live Git-host authentication and publication were not part of these local tests.

Local macOS debug bundle rebuilt in place: `target/debug/bundle/macos/GitNova.app` (80.30 MiB). Ad-hoc signing and embedded-UI verification passed. The user closed the running app before packaging; no alternate bundle was created and no native GUI or developer repository write was launched during verification.
