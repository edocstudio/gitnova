# Native acceptance and unsigned alpha preparation

Scope: Windows/Linux current-feature parity follow-up and unsigned alpha
packaging. No commit, push, release publication, signing credentials or GitHub
settings changes are performed by this work. The owner has no platform signing
certificates yet and selected unsigned alpha preparation.

## Hosted baseline inspected

[Run 41203066879](https://github.com/edocstudio/gitnova/actions/runs/41203066879)
tested `98ab705d95917a1695501bc1cc386813f8862a40`, before these changes.
macOS desktop/core and Ubuntu core passed. Windows core, Windows desktop and
Ubuntu desktop failed. This is not a passing cross-platform release run.

- Windows core: fixture LF expectation encountered runner-global automatic
  CRLF conversion. New disposable CI Git homes fix the fixture policy without
  supplying a user identity or changing production Git configuration.
- Windows desktop: canonical/verbatim cwd spelling differed in the PTY test;
  compare canonical directory identity while still requiring actual PowerShell
  output. Clone also needed local source paths normalized for Git's parser,
  alongside the previously normalized destination/template paths.
- Ubuntu desktop: 116 of 119 graph workflows passed. Remaining failures covered
  raw non-UTF-8 staged paths, secondary-pointer context-menu lifetime and Thai
  discard-progress geometry. Git readers now quote arbitrary bytes losslessly;
  menus close on primary outside clicks; sticky progress leaves bottom space.
  Assertions remain in place, with an additional secondary-pointer regression.

## Candidate evidence

The [candidate procedure](../../contributing/unsigned-alpha.md) launches the
actual installed/extracted native executable against private temporary Git
fixtures. It verifies WebView/native catalog/graph IPC, single-instance reuse
and preserved fixture state. Reports bind acceptance to exact installer and
binary hashes, native target/version and bundled attribution resources.
Release builds require an explicit diagnostic flag and bounded private fixture;
they do not enable debug soak injection or fall back to a user's catalog.

The macOS inventory contains 392 dependency candidates, 247 unique license texts,
50 unresolved entries and 27 component-specific term reviews. One missing crate
text is supplied from its pinned published VCS revision with a checked hash.
The inventory still includes build/development/optional candidates, not a
certified shipped-component list. SDK terms and asset provenance remain review
items. `auditComplete` and `publicationGatePassed` remain false.

## Remaining gates

The new hosted Windows/Linux jobs, installed OS-store/dialog workflows and full
native manual feature map still require successful results for the new commit.
Unsigned/ad-hoc artifacts have no trusted publisher signature or notarization.
No public distribution or full parity certification follows from a local build
or startup smoke. Long-session WebKit memory follow-up remains separate work.

## Local checks

Run serially on macOS with one Cargo compile job and one Rust test thread:

- Workspace/all-target Clippy with `-D warnings`: passed.
- Core workspace suite: 424 passed, 0 failed, 1 opt-in OS credential-store test
  ignored by the normal suite; no real account credentials were used.
- Native desktop tests: 7 passed, including fixture containment and native IPC.
- TypeScript/Vite build and frontend unit suite: passed, 55 unit tests. Vite
  still reports the existing large main-chunk warning; no performance claim is
  inferred from successful compilation.
- Targeted real-Git browser regressions: 3 passed (raw paths, pending-merge
  discard progress and submodule-pointer context menu). Native Linux behavior
  still requires the new hosted run; this is a local macOS regression pass.
- Packaging/prerequisite Node tests: 11 passed. CI-home and pinned-license
  provenance Python tests: 2 passed. Workflow YAML parsed; Rust formatting passed.
- Built the macOS local alpha app, verified its ad-hoc signature and all five
  attribution resources, zipped it, extracted the ZIP and successfully ran
  native startup/IPC/single-instance acceptance from the extracted executable.
  The [artifact manifest](macos-alpha-artifact.json) records exact hashes and
  outstanding gates. `sourceCommit` is null because this local candidate includes
  uncommitted changes over the inspected baseline, not a clean release commit.
- Replaced the closed Desktop `GitNova.app` with the tested extracted bundle and
  reverified its signature and bundled attribution. No additional visible app
  copy or public release was created.
