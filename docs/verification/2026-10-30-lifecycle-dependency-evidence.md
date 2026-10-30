# Lifecycle and dependency evidence — 2026-10-30

Scope: [M78](../milestone-78.md). Writes below use disposable repositories;
no project checkout, remote, credentials or hosted settings were used to test
Git operations. Suites ran serially, Cargo with one build job/one test thread,
Vitest and Playwright with one worker.

## Implemented and exercised

- Root replay distinguishes unrelated complete histories from shallow history.
  Rebase recreates bounded ordinary two-parent merges, preserves other refs,
  validates ordered parents/authorship/messages, and binds native rewritten labels.
- Registration Stage/Unstage reviews the manifest and selected gitlink together,
  keeps child files and unrelated staging, and rejects stale/ambiguous inputs.
- Missing-worktree prune retains the branch tip, backs up metadata and atomically
  archives only the selected registration directory. It never runs global prune.
  Matching catalog memberships are removed separately from native metadata.
- Follow-file search carries each historical filename through graph/diff
  navigation. Cross-repository comparisons inspect explicitly registered child
  identities and report committed gitlink ancestry with unknown/partial evidence.

## Results run locally

| Check | Result |
| --- | --- |
| `cargo test -p gitnova-core --test rebase -- --test-threads=1` | 17 passed |
| Recreated merge Continue/foreign-label regression after symbolic-label hardening | 1 passed |
| `cargo test -p gitnova-core --test related_maintenance -- --test-threads=1` | 13 passed |
| Prune subset after switching to selected-directory archival | 2 passed |
| `cargo test -p gitnova-core --test submodule_lifecycle -- --test-threads=1` | 6 passed |
| `cargo test -p gitnova-core --test investigation -- --test-threads=1` | 6 passed |
| `npm run check` | Passed |
| `npm test -- --maxWorkers=1` | 53 passed / 13 files |
| Selected real-core browser flows | 7 passed / 1 worker, 1.1 minutes |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `git diff --check` | Passed |
| Changed Markdown local link/image targets | 159 checked, no missing files |
| EN/TH locale key parity | Passed |

The seven browser cases cover existing conflict Rebase Continue, existing
multi-repository investigation, historical rename navigation, paired registration
staging, missing-worktree archival, merge-preserving Rebase and dependency-child
navigation. Only temporary Git repositories are used. These are browser transport
tests, not native WebKit interaction acceptance or hosted CI passes.

An earlier browser run passed five cases and failed the prune fixture because it
used a nonexistent picker selector. The test was corrected to the existing picker;
the final seven-case run passed without increased timeouts or weaker write checks.
The symbolic-label regression initially exposed an overly narrow error assertion
and a ref-format edit in the wrong reader. The ordinary ref format was restored,
the native-label reader hardened, and that regression passed after correction.

Clippy reports a cached build-script linker advisory about a macOS 13 Rust object
and an 11.0 link target. It completed successfully; this is not evidence that
macOS 11 is supported. Native Linux/Windows execution, live providers and the
entire browser suite were not run in this task.

## Boundaries

Rebase remains bounded to 100 reviewed commits; octopus/cousin ranges, custom
merge drivers, renormalization and submodule-changing replay are refused. Original
manual merge resolutions may need reapplication. General interactive history
editing is not included.

Prune cannot restore working files on a missing disk. It refuses ambiguous,
locked or other missing/broken registrations. Metadata archival and catalog
persistence are separate effects; partial failures require inspection.

Rename history is bounded to 100 results/2 MiB and Git history simplification;
it is not full blame/line archaeology. Dependency evidence admits at most 32
gitlinks per parent and requires explicit checkout identity before ancestry reads.
Cached remote containment is not live publication proof or semantic/package/CI
release certification. No new performance or long-session RAM claim is made.

## Local bundle

`npm run build:local` passed, including embedded UI verification and strict deep
codesign verification. After a second running-process check, the bundle replaced
the existing `~/Desktop/GitNova.app`; no alternate app was created. Installed-copy
codesign, executable SHA-256 equality and `--verify-bundle` from `/tmp` with a
minimal system PATH passed. No native workspace/window was opened.

Executable SHA-256:
`2e7248fff9c3b5eb6a60652d676a559f342923ee17f892a5e5ff0acfe549d32e`.

Vite retains its existing large-chunk advisory; no chunk-warning suppression or
new performance claim was introduced.
The artifact remains a local ad-hoc-signed macOS debug build, not a notarized
public distribution. No commit, push, PR, merge or release was performed.
