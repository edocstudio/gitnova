# M65 — Source-build prerequisite guidance

A teammate followed the quick-start npm commands without Cargo installed. The README listed Rust/Cargo as requirements, but the executable quick-start did not explain how to obtain them or distinguish nvm/npm dependencies from native build tools.

README and CONTRIBUTING now give an ordered setup: install stable Rust/Cargo and platform build dependencies, activate a supported Node version, verify the tools in the same terminal, install JavaScript dependencies, run doctor and start Tauri. They distinguish a prebuilt standalone app (no Rust/Cargo/Node runtime requirement, Git still needed) from source development. Public installers remain unavailable.

`npm run doctor` uses bounded read-only version probes for Node, rustc, cargo and Git, plus the macOS SDK when applicable. Minimum Rust and accepted Node bounds are read from the manifests. Missing/old tools and timeouts print installation/PATH guidance and fail clearly. No tool installation, compilation, credential access or Git repository command occurs. Rustup auto-install is disabled for probes. Linux/Windows system dependencies remain an explicit manual prerequisite; the doctor is not a full system-library audit.

The npm Tauri wrapper runs the same check for `dev`/`build`, stopping before the native CLI when tools are missing. Informational commands such as `--version` retain direct forwarding. Frontend-only Vite/check/test commands are unaffected. No app screen or setting is introduced: this belongs to source setup and terminal diagnostics.

## Verification

- `node --test apps/desktop/scripts/check-prerequisites.test.mjs`: seven tests passed. Includes valid tools, absent Cargo, older toolchain, SDK failure/timeout, Node engine boundaries, non-macOS SDK omission, nightly rejection and a real child invocation of `tauri dev` with an empty PATH that exits before starting a dev server or app.
- `npm run doctor`: passed against installed tools on macOS.
- `npm run tauri -- --version`: forwarded successfully to Tauri CLI 2.12.4.
- `git diff --check`: passed.

Logs: `/tmp/gitnova-m65-prerequisites.log`, `/tmp/gitnova-m65-doctor.log`, `/tmp/gitnova-m65-cli.log`. No native app build/launch or full app tests were needed/run for these tooling/docs changes. Linux/Windows machines were not used; their dependency requirements link to the official Tauri instructions. No dependencies/lockfiles, CI settings, user tools or credentials changed. No commit or push was performed.
