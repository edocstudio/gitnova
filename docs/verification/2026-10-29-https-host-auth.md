# M58 — HTTPS host-account authentication verification

## Result

The missing connection between saved provider accounts and native Git transport
is implemented. The single canonical local `GitNova.app` was rebuilt in place,
embedded assets and ad-hoc signature verified, 70.98 MiB on disk. The normal app
was closed by the user before replacement; no alternate app was created.

## Checks

- Full workspace regression: **337 tests passed**, no failed suites, one build
  worker and one test thread. This included the existing branch, staging,
  discard/recovery, merge/cherry-pick/revert, rebase, submodule/worktree, refs,
  remote publication, search and IPC checks.
- Final authentication-focused run: **42 checks passed**, including final command
  target parsing, protocol/account matching and the real HTTPS transport case.
  This run overlaps the workspace tests; these counts must not be added as
  distinct coverage.
- A disposable TLS server wrapping `git-http-backend` required Basic authentication.
  Real Git destination inspection, confirmed Push and Fetch succeeded with a
  fixture helper. The remote branch matched the reviewed source OID. A preexisting
  Git username was replaced by the selected fixture account.
- A URL-scoped plaintext credential helper did not receive/store the fixture
  token. Repository config was unchanged by publication/fetch. An authenticated
  same-host redirect was refused despite a less-specific redirect-enabled Git
  setting; the outside destination received no authenticated follow-up.
- Protocol checks cover exact authority/port/repository path, longer hosting base
  path, duplicate/malformed/oversized fields, HTTP refusal, ambiguous saved
  providers and denied account access. The helper path includes spaces and an
  apostrophe, exercised with actual Git shell invocation.
- Frontend: **45 tests / 11 files passed**, TypeScript check passed. New account,
  authentication and Push wording is present in both English and Thai. App and
  website FAQ snapshots were updated from the matching bilingual source guide.
- Strict workspace/all-target Clippy passed with warnings denied; formatting and
  whitespace checks passed. Some linked dev-test binaries emitted an SDK/deployment
  target warning (Rust libraries target macOS 13 while that test link used 11);
  this does not establish older macOS compatibility.
- Packaged executable helper probes passed: `get` without a validated transport
  scope, `store`, `erase` and invalid operation exit silently without starting a
  Tauri window or requesting Keychain access. No native probe remained running.

## Scope and limits

Tests use synthetic temporary repositories, credentials and certificates only.
No actual account token was retrieved by automation; the user's repository was
not staged, committed or pushed. A real GitHub/GitLab token still needs the server's
repository and read/write permissions. The first saved-account read by the rebuilt
helper may require the user's macOS Keychain permission.

Native account storage/bridge acceptance here is macOS-specific. Existing SSH
agents and local/file transports remain independent. Submodule initialization
paths that do not resolve a selected HTTPS transport retain ordinary Git credential
handling rather than gaining a broad parent/account scope. Live server publication
and platform acceptance beyond this fixture remain user-driven follow-up checks.
