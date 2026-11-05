# Pull/Push comparison counts — 2026-11-05

## Resulting behavior

Keep the existing toolbar, graph and Pull/Push review flows. Pull shows the behind
count with a downward SVG arrow; Push shows the ahead count with an upward arrow.
Known zero remains visible as `0`. Branch labels retain their compact indicators.
No upstream-setup form, destination prompt or checkbox default was added to these
flows in the final implementation.

A configured upstream remains the comparison baseline. For the **current branch
without an upstream**, compare its commit with the locally fetched
`refs/remotes/origin/main`, per the requested behavior. The tooltip names that ref
and states that the comparison is for display only. Counts are commit divergence,
not uploaded objects or approval to integrate another branch. A separately chosen
Push destination is still validated by the existing preview.

## Boundaries and performance

- `branch_tracking` and all write-preview metadata retain the real upstream and
  original counts; display-only fallback is a separate optional `comparison` DTO.
- Never write branch tracking configuration, switch branches, merge or change files
  to calculate these indicators. No implicit network fetch is introduced.
- No fallback for a missing configured upstream, detached HEAD, missing exact
  `origin/main` ref or shallow history. Unknown comparisons are not fabricated zeros.
- Only the active untracked branch needs the extra divergence calculation. Reuse
  the existing bounded display cache, keyed by repository/worktree, config and ref
  fingerprint. Its weight includes the new comparison strings. Ref changes invalidate
  the comparison, so fetched remote advances update the toolbar.
- Revision matching prevents old-HEAD counts being rendered as the current result.
  Invalid, negative and unsafe JavaScript counts are rejected.
- EN/TH tooltips explain the comparison scope. Names truncate before indicators,
  and toolbar controls wrap on narrow windows.

## Verification

- TypeScript check and frontend suite: 67 tests passed in 16 files.
- Rust tracking integration: four tests passed, including real local/remote
  divergence, explicit upstreams, unknown/shallow states and the new fallback.
- The fallback regression proves 1 ahead / 2 behind, invalidates to 3 behind after
  a new fetch, reuses the cache on unchanged reads, refuses a ref-prefix match,
  preserves HEAD and dirty files, leaves upstream unset and keeps fallback out of
  write metadata.
- Targeted Playwright suite: four tests passed in 58.3 seconds, covering Push
  synchronization and narrow layout, differently named upstreams, Fetch/Pull, and
  the untracked-branch fallback. The fallback shows Pull 2 / Push 1, then Pull 3
  after fetched refs advance; Push keeps the original destination and unchecked
  upstream checkbox. HEAD, dirty content and unset upstream are preserved.
- `cargo fmt --all`, targeted Clippy with `-D warnings`, and `git diff --check`
  passed. No broad platform or long-running memory suite was repeated.
- `npm run build:local` passed. This is the local macOS debug/ad-hoc-signed alpha,
  without notarization. Vite reports the existing large-chunk warning. Bundled
  attribution evidence still reports an incomplete audit and 50 unresolved items;
  this change does not certify license compliance.
- Replaced `/Users/if668061/Desktop/GitNova.app` while closed. Installed signature
  and `--verify-bundle` passed without opening a workspace. Built and installed
  executable SHA-256 match:
  `8d174eac12788c27fe281cf9ca997f8987feca5651da6eacf0652fab975646fb`.

All Git writes for tests use disposable repositories and local bare remotes.
Browser tests use the real Rust core behind an emulated transport; they do not
certify native WebView behavior on every platform. No actual user repo settings,
credentials or network Git destination were changed.

## Follow-up: actionable Pull and dirty local checkout

The first indicator fix left Pull's upstream-only resolver unchanged. As a result,
Pull could display an incoming count against origin/main and then fail with an
upstream configuration message. The follow-up reuses the existing merge review
for that exact fallback, fetches origin/main, pins the current branch/tip, defaults
to fast-forward only and leaves upstream unset. Diverged histories require the
explicit merge-commit choice; dirty merge inputs still lead to Working changes.

Local branch switching previously rejected all dirty work, even unrelated files.
Only this local-checkout entry point now permits disjoint changed paths; remote
integration and other preview callers retain their existing protection. Preview
and execution both validate state. Overlap, renames, unsupported/hidden entries,
submodule constraints, occupied worktrees and native overwrite protections remain.
The test includes staged content plus subsequent edits and untracked files, then
proves that a newly overlapping edit invalidates the prior switch confirmation.

Follow-up verification:

- TypeScript check and all 67 frontend unit tests passed.
- All 9 branch-switch integration tests and all 13 remote integration tests passed.
- Targeted browser suite passed all 5 cases in 57.5 seconds: safe local checkout,
  configured-upstream Fetch/Pull, stale/diverged/dirty merge protection, fallback
  dirty-work guidance and complete fallback fast-forward with upstream still unset.
- Targeted Rust Clippy with `-D warnings` passed. Whitespace was checked with
  `git diff --check`.
- These are targeted regressions with disposable local repositories, not a
  certification of every app workflow or Windows/Linux native runtime.

- Follow-up local build passed; installed over the closed Desktop GitNova.app.
  Signature, embedded bundle check and executable hash match passed. Current
  executable SHA-256: `d870e6d3c862abe2d96f4fa897285f49243805fa9e3848fc61e37fbda6bede1f`. No workspace was opened by verification.
