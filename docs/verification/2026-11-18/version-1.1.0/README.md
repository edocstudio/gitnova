# CI notifications release version correction — 1.1.0

Date: 2026-11-18 (Asia/Bangkok). Branch: `feat/ci-action-notifications`.
Feature implementation: `ff98420`; this follow-up corrects its release number
from 1.0.21 to **1.1.0**, because CI notifications are a backward-compatible new
feature. `AGENTS.md` now records semantic version selection instead of patch-only
delivery. No product logic or third-party dependency change was made.

The six synchronized metadata files and regenerated attribution inventory were
checked. Version-script tests and the native release build passed, including the
TypeScript check and production UI build. Existing implementation tests remain
historical evidence in the [CI report](../ci-notifications/README.md); they were
not rerun for a version-only change.

[Installed bundle](installed-bundle.json): Desktop/GitNova.app upgraded from
1.0.21 to 1.1.0, source/built/installed versions agree, executable hashes match,
strict deep ad-hoc signature and embedded UI verification passed, and all five
attribution resources match. The previous app is backed up in a temporary folder.
The existing inventory audit remains incomplete; this is a local app update.

[Installed native smoke](installed-native-smoke.json) passed catalog IPC, real
Git graph IPC, startup, single-instance window reuse and unchanged disposable
fixture HEAD/worktree. Visible native CI acceptance remains open as recorded in
the earlier report; this startup smoke does not certify that flow. No live company
provider, owner catalog or real repository was opened. No public release or push.

Local archive: `target/alpha-evidence/GitNova-1.1.0-macos-arm64.zip` (ignored output).
