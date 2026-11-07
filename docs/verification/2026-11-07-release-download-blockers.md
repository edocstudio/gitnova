# Release download blockers — 2026-11-07

The owner requested downloadable 1.0.0 installers. The release-candidate run
42385943321 (source 711b987) cannot assemble all platforms: Windows shard 1
failed attribution collection, shard 2 failed a bounded Stash-readiness check,
and Linux failed its Debian installation entrypoint. This change prepares
focused corrections without changing the developer's ongoing toolchain work.

## Corrections

- Read source JSON as UTF-8 and write the inventory JSON as UTF-8 explicitly.
  The Windows Python 3.12 log shows cp1252 failing on an npm package manifest
  containing byte 0x81. A regression fixture emulates that legacy default with
  non-ASCII metadata and license bytes. Original license bytes/hashes and the
  incomplete-audit flag remain unchanged.
- Remove only generated `target/release/bundle` outputs before CI packaging.
  The cached target can contain both 0.1.0 and 1.0.0 packages; the Debian glob
  then passes more than the required three arguments. Clearing bundle output
  also prevents stale AppImage/NSIS files entering later verification/uploads.
  Preserve the compiled Cargo cache and isolated acceptance bridge.
- Give the observed direct-main Stash readiness assertion the existing
  45-second write/readiness budget. The failing log waited only 15 seconds for
  the disabled control. No global timeout, runtime guard, Git-content assertion
  or workflow coverage is removed. Hosted Windows verification remains needed.

## Local validation

- Three inventory tests pass. The new non-ASCII/legacy-codepage regression fails
  with the original collector's UnicodeDecodeError and passes with the fix.
- Workflow YAML and Debian shell syntax parse.
- A disposable bundle fixture verifies old artifacts are removed, compiled
  bridge bytes survive, and a fresh build exposes only its current .deb.
- Desktop TypeScript checking passes with the existing installed dependency tree.
- Playwright lists exactly the intended direct-main Stash regression. No native
  app launch, full desktop build or hosted Windows/Linux acceptance is claimed.

No installer was published and no public download URL was invented. A passing
exact-source full release-candidate run and actual asset publication remain
necessary before the website download feed can be switched to published.

Later-completing Windows shards reported additional failures in submodule
registration, operation preparation and conflict/rebase completion. This focused
patch is not a claim that all Windows UI failures are fixed. Retained traces
and a new exact-source hosted run are needed to distinguish slow in-flight
writes from application-state defects; no failed test is skipped or converted
to success. The full candidate must remain blocked until those checks pass.
