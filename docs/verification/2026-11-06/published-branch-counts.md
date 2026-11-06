# Published branch comparison

The reported branch had no configured upstream, so display-only counts continued
to compare its published feature commits with origin/main. This incorrectly
suggested those commits still needed publication, despite a successful push.

For the current branch without an upstream, compare the exact fetched
origin/<local branch> first, and fall back to origin/main only if that branch is
absent. Explicit upstreams retain priority. No configuration is silently changed.
The existing fingerprint/config display-cache key invalidates when fetched refs
change; no additional cache or periodic request is needed. Ref candidates are
queried together and matched exactly, not by prefix.

Pull now resolves the comparison reference through the remote listing and reviews
that exact source, preserving current-branch/OID validation. It does not silently
pull main after showing same-name branch counts. Push retains its reviewed target.

Validation (2026-11-06):

- Tracking integration suite: 5 passed, including publication without upstream,
  cache invalidation, equality after push, new unpublished commits and explicit
  upstream precedence.
- TypeScript check and 69 frontend unit tests passed.
- Targeted browser suite: 3 passed. The new case pushes through the UI without
  setting upstream, checks zero Pull/Push and graph equality, fetches incoming work
  on that feature branch, and explicitly pulls it through the correct review.
  Existing origin/main fallback and fallback-Pull cases also passed.
- Tests used temporary repositories/local remotes. No developer repository was
  pushed, pulled or reconfigured. Browser transport is emulated with real Rust Git
  results; this is not a full cross-platform native acceptance run.
- Targeted Git/core Clippy with all targets and `-D warnings` passed.
- `cargo fmt --all` and `git diff --check` passed. The browser screenshot was
  inspected: toolbar counts are zero and the current graph branch shows equality.

`npm run build:local` passed, with the existing debug/ad-hoc macOS signing and
bundle verification. Notarization and the existing incomplete attribution audit
remain outside this fix. Current main did not contain the previously installed clickable-CI commit
`af86e892`. The user explicitly requested retaining it in this change; its source
patch was reapplied without switching branches or changing the index/stash. After
combining, TypeScript and 69 frontend tests passed again, along with 2 browser
cases covering CI links and the publication/Pull journey together. No commit or
push was performed.

The combined build passed and was installed over the closed canonical Desktop
GitNova.app. Strict/deep signature and installed `--verify-bundle` checks passed;
the executable matches the built artifact (SHA-256
`26b93332ab9b0d91e5ff4ba5349ab0927a79eb79feb4a4135b4b6424ce7dc37a`).
