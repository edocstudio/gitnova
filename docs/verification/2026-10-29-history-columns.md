# Resizable history column verification — 2026-10-29

Verification ran sequentially with a single browser worker. No user/company repository was modified. Browser fixtures use disposable local repositories and a real Rust core with emulated IPC.

- `npm run check`: passed before browser testing; local packaging also runs TypeScript validation against the final source.
- `npm test -- --maxWorkers=1`: 53 tests passed across 13 files. New cases cover malformed/unknown persisted values, dimension bounds, compact legible graph sizing, independent tracks and minimum table width.
- Focused browser selection (`graph columns resize|parallel branch graph|stash.*graph|graph.*stash`): six passed; the remaining stash geometry assertion initially failed because it classified CSS-variable-colored stash connectors as ordinary branch lines. Updated that color filter without removing the separation check. Follow-up `direct main stash pops|graph columns resize`: both passed. Seven distinct browser cases passed across the two runs.
- The column browser case drags all six dividers, checks header/row positions and widths, preserves selection, reloads stored sizes, adjusts by keyboard, resets CI, and checks horizontal scroll alignment at a narrow viewport. Stash regression also verifies branch separation after graph contraction; merge topology, stash apply/drop/pop/conflicts and packed-stash behavior passed.
- Screenshot inspected: `apps/desktop/test-results/graph-graph-columns-resize-e3ec0-ection-and-scroll-alignment/resizable-graph-columns.png`.
- `git diff --check`: passed.

Logs: `/tmp/gitnova-m69-ui.log`, `/tmp/gitnova-m69-browser.log`, `/tmp/gitnova-m69-browser-followup.log`. Browser tests do not certify native WebKit pointer behavior. No new native code, dependency, Git write behavior or hosted CI setting was introduced, and no full native suite or performance benchmark was run for this display change.

`npm run build:local` passed with final TypeScript/UI assets. The existing `target/debug/bundle/macos/GitNova.app` was updated (71.57 MiB), and embedded UI/local bundle verification passed. This is a local debug, ad-hoc signed build, not a notarized distribution. Log: `/tmp/gitnova-m69-build.log`. No commit, push, PR or release is performed.
