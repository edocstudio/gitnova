# Local distribution evidence — 2026-10-30

Scope: local macOS arm64 alpha/debug artifact, without publishing, changing
accounts/security settings or using signing credentials. Runtime Git features
were verified separately in M80–M82.

## Inventory and limitations

The offline inventory contains 390 dependency candidates (295 external Rust
packages and 95 npm packages), 245 unique collected license/notice texts and
98 asset candidates. Actual binary/JS contribution is not fully classified.
The report records **51 unresolved components** and **27 component-specific terms
reviews**. These counts describe collection gaps/review candidates, not confirmed
license violations or 27 mutually exclusive components.

Unresolved entries include uncached optional packages for other platforms,
native build-tool packages and Rust packages with a license declaration but no
license file in the cached published source (including Objective-C bindings).
Do not invent replacement notices or treat a manifest declaration as a complete
attribution audit. Upstream version-bound provenance, shipped-component analysis,
license-choice/source obligations and asset permissions remain public release
gates. `auditComplete` stays false.

## Checks performed

- Temporary packaging fixture: complete five-resource inclusion succeeds;
  tampered NOTICE and stale Cargo.lock are refused (3 cases).
- Temporary collector fixture: a license symlink outside the package is excluded;
  a text above 1 MiB is refused (2 cases).
- Read-only local signing identity discovery reports **0 valid identities**.
  No Developer ID signing/notarization was attempted, and no certificate was
  selected or installed.

## Actual local artifact

`npm --prefix apps/desktop run build:local` passed on macOS arm64. Cargo used
one job; the native debug build took 1m49s and Tauri reported a 77.04 MiB app.
The existing frontend chunk-size advisory remains; it is not a failed build.
Notarization was explicitly skipped because no signing credentials were supplied.

The five bundled attribution resources matched their source bytes. Deep/strict
ad-hoc signature verification and `--verify-bundle` from `/tmp` with only system
PATH passed. The app was closed before copying into the existing canonical
Desktop GitNova.app; executable/resource hashes matched afterward. No alternate
installed app was created and no user repository/window was opened for this check.

A local debug ZIP under `target/distribution/m83-local-debug/` was created with
`ditto`; each of its five license resources was read back and matched. This ZIP
is ignored build output, not an uploaded installer or a notarized release.
[Machine-readable receipt](../licenses/local-artifact-m83.json) records the
binary/archive SHA-256, source base commit, dirty-source status and acceptance
limits without absolute machine/user paths.

Source/documentation text scanning found no competitor identifiers, known private
host identifiers, developer absolute paths or token-shaped matches in tracked
and nonignored new files. This bounded pattern scan is not a complete secret or
image-content audit. No source file exceeded 10 MiB. All 195 relative file links
in changed/new Markdown resolved locally; external URLs were not revalidated by
this local check.

This document is not a completed legal audit or public release approval.
