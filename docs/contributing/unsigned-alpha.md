# Unsigned alpha candidate workflow

The owner has chosen unsigned alpha preparation while platform signing accounts
are unavailable. This is not a public-release approval or a promise of native
feature parity. Do not disable branch protection or omit failing tests to obtain
an installer. The [distribution checklist](binary-distribution.md) still applies.

## Candidate pipeline

`.github/workflows/ci.yml` retains full core tests on all three platforms, all
graph workflows against the real Rust/Git bridge, native IPC/PTY tests and
isolated Windows/Linux credential-store checks. Each job gets a disposable Git
home with `core.autocrlf=false` and **no identity**. Existing toolchain caches
retain their absolute paths. This avoids runner-global line-ending policy
changing fixture bytes; production Git settings are never overridden.

After tests pass, each native job collects target-specific dependency evidence
and creates its alpha candidate:

- macOS: ad-hoc-signed `.app` in a ZIP. Ordinary checks use the local debug
  profile; the manual release-candidate workflow requires the release profile.
  Neither profile is Developer ID signed or notarized.
- Windows x64: unsigned per-user NSIS installer, release build.
- Linux x64: release Debian package installed with apt on a disposable runner,
  plus AppImage extracted in a temporary folder and started through AppRun.

Each candidate's actual installed/extracted executable is launched with an empty
private home and private temporary real Git repository. Native WebView startup,
catalog and graph IPC, duplicate-launch reuse and unchanged fixture HEAD/working
tree must pass. This smoke does not replace all native manual GUI workflows.
The diagnostic requires explicit `--native-acceptance`, its environment opt-in,
a bounded fixture catalog under the OS temporary directory, and every recorded
repository/worktree path resolving inside that fixture. Invalid diagnostics exit;
they never fall back to the user's catalog. Soak injection remains debug-only.

The runner emits `*-acceptance.json`, binding those checks to the installer's
SHA-256 and installed executable's SHA-256. `alpha-artifact.mjs` rejects a changed
installer, wrong OS/architecture/version/target, missing checks, stale lockfile
inventory or mismatched attribution resources. `*-artifact.json` records exact
hashes, source SHA when supplied by CI, target, size, lockfile hashes and limits.
Neither file is a cryptographic publisher signature. Evidence cannot be reused
after modifying, signing or repackaging an installer.

Artifact hashing reads in 1 MiB chunks rather than loading an entire installer
into RAM. Cleanup must stop both owned native processes before acceptance can
remain successful. The macOS ZIP is extracted before native acceptance too.

Artifacts are uploaded as CI candidates only after that job passes. No workflow
publishes a public GitHub Release. The optional release-candidate handoff can
attach verified packages to a draft prerelease. `publicationGatePassed` remains false while manual
native acceptance, dependency/asset review and trusted signing are outstanding.
An alpha build can succeed with known inventory gaps; do not describe that as a
completed license audit or a public-release gate pass.

Linux additionally reinstalls the same Debian candidate, checks the executable
hash and attribution resources, then purges it with dpkg and checks removal.
Windows repeats the NSIS installation, checks the executable/resources, then
uses the installed silent uninstaller and checks removal. Successful runs emit
`*-lifecycle.json` tied to the installer SHA-256. These checks do not verify
upgrades from older versions, migration of real user data or manual GUI flows.
The Debian script refuses to run outside a disposable GitHub-hosted Linux runner
or replace an already installed GitNova package. Do not run it on a work machine.

## Signing later

Do not paste certificate files, tokens or passwords into issues, source files or
build arguments. The unsigned pipeline currently receives no signing secrets.

For macOS, use a separate reviewed release pipeline and Developer ID identity;
the local `build:local` intentionally uses ad-hoc signing and strips account
signing/notarization credentials. Follow the official
[Tauri macOS signing/notarization guide](https://v2.tauri.app/distribute/sign/macos/),
verify the resulting signature and notarization on a clean Mac, then regenerate
all artifact hashes/acceptance evidence for the actual distributed bytes.

For Windows, configure an appropriate signing identity/service in the separate
reviewed release pipeline using the official
[Tauri Windows signing guide](https://v2.tauri.app/distribute/sign/windows/).
Verify the installer signature/timestamp on Windows and rerun installed-app
acceptance after signing. Do not claim SmartScreen reputation or eliminate user
warnings merely because a signing command succeeded.

Linux candidate hashes are currently integrity evidence only. No signed package
repository or update channel is configured. A future signing/publication choice
must record its verification procedure and publisher key separately.

## Native manual acceptance record

Record the artifact SHA, OS/version, architecture, Git version, installation
method, tester and result. Keep credentials/private repository data out of the
record. Use disposable repositories and test accounts chosen by the owner.

- Install, start without Cargo/Node, reopen, upgrade/uninstall; check shortcuts,
  icons, embedded UI and accessible license texts.
- Exercise the [whole feature map](../platform-parity.md): graph/workspaces,
  Init/Clone, stage/hunk/commit/amend, branch/remote/tag operations, merge/replay/
  conflict/recovery, stash, submodule/worktree lifecycle and archaeology.
- Check masked input/cancel, locked/unavailable OS store, exact host/account
  isolation, HTTPS and SSH, clipboard, browser links and real CI/PR/MR access.
- Check PowerShell/ConPTY or the installed POSIX shell, cwd with spaces/Unicode,
  resizing, exit, hide/reopen and cleanup. Check native fonts/EN/TH and dialogs.
- Include restart/interruption and long-session RAM/response measurements. A
  passing startup smoke or one hosted runner does not certify every OS version.

Mark failures and untested items explicitly. Never fill this record with inferred
passes from macOS tests or successful packaging on another platform.

See [release-profile candidates and Linux signing](release-candidates.md) for the
manual candidate pipeline, owner key setup and separate website handoff. This
preparation does not close the existing native acceptance or license audit gaps.
