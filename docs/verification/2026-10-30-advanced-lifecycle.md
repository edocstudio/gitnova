# Advanced lifecycle verification — 2026-10-30

Scope: [M80](../milestone-80.md). Existing M78/M79 work was committed by the owner
before this task; this increment preserves that baseline. Suites are serial,
Cargo one job/one test thread, Vitest and Playwright one worker.

- Full Rebase integration suite: **24 passed**, 136.27 seconds. Includes prior
  recovery/tampering/shallow/author protections, clean cousin and three-parent
  replay, conflictful octopus refusal, reordered Pick/Drop/Fixup, same-base fixup
  in a quoted path and custom conflict Abort after application restart.
- Worktree maintenance suite: 15 passed plus one fixture failure. The synthetic
  gitlink fixture was missing its registration/folder and triggered dirty-state
  refusal before the intended submodule guard. Adding a valid uninitialized
  manifest/folder corrected the fixture; that guard regression then passed.
- Clean move/dirty-locked-destination refusal subset: **2 passed**, preserving
  existing workspace identity, selected file/staging state, normal refs and a
  dirty parent checkout. The initial fixture's all-ref assertion was corrected
  to distinguish explicitly retained recovery refs from normal branch/tag refs.
- Workspace/all-targets Clippy with warnings denied: passed. Existing cached
  build-script macOS deployment linker advisory remains, not an older-OS claim.

Frontend type checking and 53 unit tests passed. Four focused browser regressions passed (owned rebase conflict continuation, missing-worktree prune, two-parent topology and clean worktree movement). The new interactive replay regression initially caught a response comparison that depended on JSON key order; comparing OID/action values fixes it, and that browser test then passed (17.1 seconds). The regression also checks invalid-plan correction and preserved target branch.
No hosted CI or Windows/native desktop acceptance is inferred from macOS tests.

The broader follow-up subsequently passed all **25 Rebase tests** (including batched publication evidence) and all **16 maintenance tests**. It also found an unrelated existing gitlink deletion admission issue in Commit review; the correction and serial reruns are recorded in [the performance/regression follow-up](2026-10-30-performance-followup.md).
