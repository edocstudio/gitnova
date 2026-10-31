# Desktop platforms (alpha)

Windows and Linux desktop support is being added alongside macOS. Native CI builds, IPC/PTY tests, synthetic credential-store checks and packaging checks are configured in `.github/workflows/ci.yml`. Configuration is not evidence of a passing hosted run: verify the checks for the exact commit before distributing its artifacts. The initial native CI run failed before completing acceptance; see the [findings and corrections](verification/2026-10-31/native-ci-corrections.md). No passing Windows/Linux installed-app acceptance is claimed.

| Platform | Build target | Secret storage | Terminal |
| --- | --- | --- | --- |
| macOS 13+, Apple Silicon | Existing local debug `.app`, ad-hoc signing | macOS Keychain, existing service/account keys retained | Installed login shell |
| Windows 10/11 x64 | Native MSVC build, unsigned per-user NSIS `.exe` | Windows Credential Manager | Windows PowerShell through ConPTY |
| Linux x64, Ubuntu 24.04 baseline | Native `.deb` and AppImage | Desktop Secret Service (for example GNOME Keyring), no plaintext fallback | Installed login shell through a PTY |

Windows ARM, older Windows, headless Linux, every Linux distribution, Flatpak/Snap and notarized/signed public releases are not certified by this setup. AppImage still depends on compatible host system libraries; it is not a promise to run on every distribution. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) and [AppImage compatibility guidance](https://v2.tauri.app/distribute/appimage/).

## Run from source

All platforms need Git 2.38+ on PATH, stable Rust 1.98+ including Cargo, and Node matching `apps/desktop/package.json` (`^20.19.0 || >=22.12.0`). An installed app does not need Rust, Cargo or Node. Git and platform runtime components are still required.

### Windows

Install Git for Windows, the Rust MSVC toolchain, Microsoft C++ Build Tools with the **Desktop development with C++** workload and Windows SDK, and Microsoft Edge WebView2 Runtime. Reopen the terminal after installing tools. PowerShell is used for the embedded terminal and masked native token dialog. Keep Git for Windows' bundled shell available: Git credential helpers use it internally.

```powershell
cd apps/desktop
npm ci
npm run doctor
npm run tauri dev
```

The doctor checks basic executables; it does not certify the MSVC workload or WebView2 installation. A compiler/runtime error still needs inspection. [Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows).

### Linux (Ubuntu 24.04 example)

Install Rust using rustup and activate its environment as described in the root README. Install the desktop development packages:

```bash
sudo apt-get update
sudo apt-get install -y git build-essential pkg-config curl wget file \
  libwebkit2gtk-4.1-dev libssl-dev libxdo-dev \
  libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev \
  zenity xdg-utils gnome-keyring fonts-noto-core
cd apps/desktop
npm ci
npm run doctor
npm run tauri dev
```

Use a normal graphical desktop session with a session D-Bus and an unlocked Secret Service. Zenity provides the masked token dialog; xdg-utils opens validated HTTPS links in the default browser. Missing/locked secret storage produces an error; GitNova never substitutes a plaintext credential file. Other distributions need equivalent packages from their package manager, including Git 2.38+ for isolated merge previews. Older distribution defaults may need a Git upgrade.

## Native installer builds

Build on the target OS. Windows installers are not produced on macOS; Linux artifacts are not produced on macOS. Cross-compiling the core alone does not validate a desktop application.

From the repository root, collect target-specific attribution evidence after dependency installation:

```bash
cargo fetch --locked
# Set TARGET to the exact `host:` value printed by `rustc -vV`.
python apps/desktop/scripts/inventory-licenses.py --target TARGET
cd apps/desktop
npm run build:desktop
```

On systems where Python is named `python3`, use that command instead. `build:desktop` verifies inventory lockfile hashes and the native target. Windows/Linux invoke a release Tauri build; macOS delegates to the existing bounded local debug build and refuses to overwrite a running app. Output is under Cargo's target directory, normally `target/release/bundle/nsis`, `deb`, or `appimage`. This script creates artifacts without publishing or installing them.

The installers are **alpha and unsigned**. Windows may display SmartScreen warnings. There is no signing/notarization or automatic update promise. The dependency inventory is evidence with unresolved items, not a completed license audit; follow the [binary distribution checklist](contributing/binary-distribution.md) before redistribution.

## Current platform-specific limits

- Linux automatic background HTTPS fetch reads only an already-unlocked, uniquely matching Secret Service item. It never unlocks or creates a collection in the background; a locked keyring requires a foreground operation. Foreground Fetch/Pull/Push can request keyring access. SSH continues to use the configured agent.
- Stale app-lock recovery uses checked Unix PID/inode ownership on macOS and Linux. Windows uses process-handle ownership checks, volume/file IDs and handle-based archival while denying competing writes/deletes. Live or unverifiable owners and native Git locks block recovery on every platform. Native Windows acceptance is pending.
- Existing macOS native smoke automation is not a Windows/Linux end-to-end acceptance suite. Native CI runs the full existing browser graph suite against the real Rust/Git fixture bridge, plus IPC/PTY, credential-store and installer-content checks; manually exercise graphical startup, repository switching, HTTPS/SSH, conflicts, clipboard, links and long sessions on the target OS before treating it as ready for daily work.
- No account or private project data is needed for native credential-store tests. CI uses isolated synthetic entries and removes them afterward. Native account dialogs require manual acceptance; provider HTTP contracts use local fixtures.

The full current-feature acceptance map is tracked in [platform parity](platform-parity.md). A passing installer build alone is insufficient.
