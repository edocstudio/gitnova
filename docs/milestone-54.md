# Milestone 54 — Direct commit sharing and the in-app guide

The footer now reads “GitNova · by edocstudio”. Legal attribution stays in NOTICE and the README; the Apache-2.0 license is unchanged.

Copy commit link no longer displays remote, provider or HTTPS configuration fields. It resolves the current branch's fetch upstream, then origin, then a sole remote, and uses the selected commit's full object ID. Private hosts use the matching global Connection, even if CI display is disabled. Unknown or ambiguous hosts report a Settings/upstream action rather than guessing. Old repository-specific link overrides are ignored. Successful copies show confirmation only; a URL fallback appears if clipboard access fails. Copying is read-only and does not publish commits or grant repository access.

The existing Settings dialog contains a searchable, expandable FAQ with 23 English/Thai explanations. Matching standalone guides are in `docs/guide/faq.en.md` and `docs/guide/faq.th.md`. They cover graph basics, staging, remote synchronization, stash/pop, merge/cherry-pick, conflicts, submodules, worktrees, tags, requests, connections and recovery. Rebase and per-hunk staging are explicitly marked as planned rather than advertised as available.

Validation: 41 frontend unit tests and TypeScript checking passed. Focused browser tests exercised exact historical SHA copying, upstream selection over origin, nested remote projects, private host Connections, and FAQ searching/language switching. Browser transport is emulated; all Git operations in tests use the actual Rust core and temporary repositories. The Thai FAQ screenshot was visually inspected. This slice does not certify other platforms, native clipboard behavior or every Git workflow.

The single canonical `target/debug/bundle/macos/GitNova.app` was rebuilt after the user closed the app. Ad-hoc code-signature verification and embedded-asset verification passed; the app was not launched and no alternate bundle was created.

Follow-up: Settings now separates General (default on every open), Connections and FAQ into accessible tabs. Switching tabs preserves mounted drafts and FAQ search/answer state. Arrow keys and Home/End move between tabs; translated tab labels and compact layouts remain available. The outer FAQ disclosure was removed because its tab already provides that scope. Three focused browser scenarios passed covering FAQ search/language changes, Settings context preservation and global host/Jenkins connections; the compact Thai tab layout was visually inspected.
