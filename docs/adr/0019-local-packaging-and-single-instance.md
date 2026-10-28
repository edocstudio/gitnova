# ADR 0019 — Standalone local app and one native instance

Daily use should not require a Vite process or recompilation. A separate local macOS config builds a custom-protocol .app with embedded UI, ad-hoc signing and one compiler job. No new screen, navigation or graph control is introduced. Bundle self-verification checks embedded assets before loading user state.

Use the official Tauri single-instance plugin first in the native builder. A duplicate launch focuses/shows the existing graph window and exits before opening a second catalog owner. Incoming command-line arguments are ignored rather than becoming implicit checkout/clone/write requests. Close older builds that predate this guard before upgrading.

A debug-only, opt-in native smoke profile must resolve state.json under a private gitnova-native-smoke-* temporary fixture. Invalid smoke state fails closed and never falls back to user data. The smoke window remains hidden and duplicate-launch focus is suppressed in that mode. Existing catalog and graph command completions provide diagnostic markers; there is no test-only mutation command or mocked Git. Release builds ignore smoke environment variables. The local smoke script verifies the packaged debug capability before launching, uses a real temporary repository and Finder-like PATH, checks WebKit→native→Git completion plus duplicate-instance exit, then stops only its own app instances.

This local artifact is not a signed/notarized public release or proof of all workflow readiness. Native signing/account publication and broader acceptance remain separate. No security agent or OS policy is modified.

References: https://v2.tauri.app/distribute/macos-application-bundle/ and https://v2.tauri.app/plugin/single-instance/.
