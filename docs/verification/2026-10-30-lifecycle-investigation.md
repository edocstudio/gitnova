# Lifecycle and investigation verification — 2026-10-30

All mutating fixtures were freshly created temporary repositories. No provider
credentials, company projects, remote publication or GitHub settings were used.
This report concerns local alpha code, not hosted Windows/Linux acceptance.

## Checks run

- Focused core regressions: hunk (6), selected submodule/worktree maintenance
  (9), investigation (4), existing workspace search (13): 32 distinct cases.
  The earlier combined run covered 30 cases; the final maintenance/investigation
  boundary run adds local relative URLs and shared/shallow worktree coverage.
- Existing Rebase integration target: 12 passed, including authorship, separate
  conflict stops, explicit Skip, Abort/recovery, stale/foreign journal refusal,
  later gitlink detection and linked-checkout isolation.
- Real Tauri command-dispatch test: one broad fixture passed, including the new
  investigation DTO and external-origin rejection.
- TypeScript check and frontend unit suites: 53 passed across 13 files.
- Workspace all-target local Clippy with `-D warnings`, offline/locked and one
  Cargo job: passed. Rust formatting and whitespace checks are recorded with the
  final change review.
- Browser focused regression selection: five existing scenarios passed
  (workspace search, worktree removal, Rebase, hunk and selected URL sync).
  The new investigation scenario passed in the follow-up run. It covers two
  repositories, matching-file diff navigation, source/target evidence, missing
  branch coverage and preservation of HEAD/refs/index/draft files.

## Failures found during development

- SSH URL redaction was incorrectly used as the post-sync expected raw value;
  the new raw-value postcondition and regression fixture address it.
- Literal `diff --git ` inside code was counted as another patch header; only
  actual header lines are now counted, with stage/unstage byte checks.
- The first investigation parser included Git's merge diff output in its
  metadata framing. Explicit no-patch formatting fixed this; the real-Git
  investigation tests then passed.
- The first new browser fixture sent the search shortcut before repository
  opening completed. It now waits for the search action to become enabled;
  the follow-up passed without increasing a timeout or removing assertions.

The focused browser run is not a claim that the entire browser suite was run.
The 53 unit tests and local macOS Clippy are not native acceptance on other OSes.
Source-profile performance evidence is documented separately; this test report
contains no claim of a full-workday memory test or bug-free release.

## Final checks and packaging boundary

Final frontend production build, all 53 unit tests, Rust formatting and all-target
Clippy passed. The frontend build still reports its large-chunk advisory; no
warning threshold was raised or suppressed. The final focused browser run passed
both the new investigation flow and searchable bilingual FAQ.

Checked 340 local documentation/image targets and their referenced Markdown
anchors: none missing/unresolved. The supplemental benchmark JSON/CSV sizes and
SHA-256 values match their integrity manifest. Publication-text pattern checks
found no known restricted identifiers or common credential signatures; this is
not a complete secrets or legal audit.

The normal native app remained open. The source/frontend outputs and tests are
updated, but this increment has not overwritten GitNova.app. The existing local
builder requires the app to be closed to protect the open session. No app was
terminated, and no branch, commit, push, PR, merge or release was created here.

The benchmark runner's Git-environment isolation smoke also passed after
sanitizing its provenance reads; it is separate from the 30-minute measurements.
