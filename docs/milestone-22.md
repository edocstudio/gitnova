# Milestone 22 — Local standalone macOS application

For the user's everyday commit/pull/push/merge and multi-repository/submodule workflows, first remove the dev-server requirement. A separate local bundle configuration embeds the compiled frontend through Tauri's custom protocol and creates an ad-hoc-signed .app. The existing graph and IPC boundary remain the interface; no UI surface is added.

build:local uses debug Rust artifacts to reuse the current cache and one compiler job. It produces a local application bundle, verifies its signature and runs --verify-bundle from /tmp with a Finder-like PATH. This flag checks the embedded HTML root and every referenced assets/ script/stylesheet before returning, without starting the GUI, loading workspace state or writing Git. Apple account-signing/notarization credentials are not passed to this local build.

This is a local validation artifact, not an optimized/notarized public release or a 95% readiness claim. Production signing, broader platform validation and the remaining roadmap are still separate. Git must be installed independently; GUI Git credential/network behavior needs practical validation on this machine. The official single-instance plugin is registered first: duplicate launches focus the existing window and exit before opening another catalog. Close older builds that predate this guard before upgrading. Incoming arguments never become implicit Git actions.

Reference: official Tauri CLI and macOS application-bundle documentation.


Validation on this Apple Silicon Mac: local bundle built with one compiler job, codesign verification and embedded-asset self-check passed. The isolated debug native smoke launched a hidden actual WebKit window against a temporary real Git repository; frontend catalog and graph requests reached native commands and Git. A second launch exited and notified the first instance. Fixture HEAD/status stayed unchanged; both owned processes stopped. This verifies native startup/IPC and duplicate-instance handling, not native automation of every write flow or corporate authentication. Commands: npm run build:local, then npm run test:native:local, sequentially from apps/desktop.
