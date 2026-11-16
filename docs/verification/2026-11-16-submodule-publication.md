# Submodule publication guidance — 1.0.16

Baseline: `6c9691a184a3fa4748dd03e511e8398c68e3a857` (1.0.15).
Verification used disposable repositories on macOS 26.7.1 arm64 with Rust
1.99.1, Node 26.13.0 and Apple Git 2.39.2; no project repository was
pushed, reset or reconfigured.

## Findings and changes

- A parent push correctly refuses a gitlink whose child commit is unpublished.
  Keep `--recurse-submodules=check`; explain the prerequisite and offer to open
  the child from the existing Push review instead of implicitly publishing it.
- A child checkout can legitimately have detached HEAD. Push now opens the
  existing create-branch preview at that exact commit. After confirmation the
  user can publish the new branch without moving an existing child branch.
- HTTP redirects are distinct from authentication failures. Report an exact
  clone-URL check, including `.git` when the server requires it. Preserve
  `http.followRedirects=false` and repository-scoped credentials. Do not append
  suffixes, change user remotes or retry publication automatically.

## Checks run

- `cargo test -p gitnova-git transport_error_tests -- --test-threads=2`: 2 passed.
- `cargo test -p gitnova-git --test transport_credentials -- --test-threads=2`:
  1 passed, including a loopback HTTPS 301 response and credential isolation.
- `cargo test -p gitnova-core --test ref_and_push -- --test-threads=2`: 22 passed.
- `npm run check`: passed.
- `npm test`: 89 tests across 23 files passed.
- `npm run test:e2e -- --grep "parent push guides publication"`: 1 passed.
  Chromium used the real Rust bridge and temporary local remotes. The test
  covers blocked parent publication, opening a detached child, creating and
  publishing its branch, then publishing the parent. It checks that the
  original child branch and the parent remote remain unchanged until their
  respective explicit publication steps.
- `node --test scripts/set-version.test.mjs`: 1 passed.
- `npm run build:release`: optimized macOS build passed. Bundle checks verified
  embedded UI and five attribution resources. Installed over the existing
  Desktop `GitNova.app`; its reported version is 1.0.16 and strict/deep
  ad-hoc signature verification passed. No notarization was performed.

The browser result does not certify native WebView behavior or Windows/Linux.
The reported company-server 301 was not reproduced against that live server;
its exact clone URL still needs verification by its owner. This change does
not claim to repair that remote URL automatically.

Attribution inventory was regenerated for `aarch64-apple-darwin`: 428
components, 259 collected license texts, 67 unresolved entries and 27 manual
terms reviews. This is evidence collection, not a completed compliance audit.
