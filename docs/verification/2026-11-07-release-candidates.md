# Release candidate preparation — 2026-11-07

Scope: prepare release-profile packaging, an optional Linux detached-signature
workflow and a separate website download handoff. No commit, push, GitHub
Release, deployment, signing-account configuration or owner key creation was
performed. The Desktop application was not replaced.

## Local checks actually run

- `npm run build:release` on macOS ARM64 with Node 20.20.2, Rust/Cargo 1.98.0,
  `MACOSX_DEPLOYMENT_TARGET=13.0` and one Cargo compilation job: passed.
- The first build exposed the missing explicit `custom-protocol` feature in
  the new script. After correction, the second build passed embedded UI
  verification, strict/deep ad-hoc signature verification and bundled
  attribution checks automatically. Final bundle size reported by Tauri:
  27.43 MiB. No Developer ID signature or notarization was performed.
- Bundled attribution: five resources checked, 50 unresolved entries,
  `auditComplete: false`. This is not redistribution approval.
- Created a ZIP in ignored `target/release-candidate-local`, extracted it and
  ran `smoke-local.mjs` against that extracted executable with a temporary
  synthetic Git repository and isolated catalog: passed native WebView,
  catalog/real Git graph IPC, duplicate-instance reuse and fixture preservation.
  This was startup acceptance, not full manual workflow or installation testing.
- `node --test scripts/prepare-release.test.mjs scripts/alpha-artifact.test.mjs
  scripts/check-prerequisites.test.mjs`: 14 passed, one skipped. The skipped
  test needs GnuPG, which is not installed locally. Linux CI installs GnuPG
  and can exercise the disposable test signing-subkey path.
- Both workflow files parsed as YAML. This does not validate or execute hosted
  GitHub Actions.
- Separate website: typecheck, 45 tests and production build passed. Its
  download feed remains unpublished and contains no installer links.

Existing frontend build warnings remain: a large main chunk and a dynamic
import also imported statically. They did not fail the build and were not
represented as performance fixes in this change.

## Outstanding owner and hosted checks

- Run the manual release-candidate workflow on the reviewed source ref; verify
  Windows and Linux native evidence. No new hosted workflow was run here.
- Complete target-specific direct/transitive dependency and asset license
  review, including the unresolved entries and actual shipped obligations.
- Create and back up the owner GPG key/subkey, configure the documented
  environment secrets, then run and verify real Linux signatures. No Linux
  publisher signature exists from this local check.
- Complete clean-machine install/upgrade/uninstall and manual workflow checks.
  Trusted macOS/Windows signing remains unavailable.
- Review exact artifacts before owner publication; only afterward update the
  website download feed. Candidate generation never sets publication approval.

See [release preparation](../contributing/release-candidates.md).
