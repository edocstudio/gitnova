# M47 — Measured everyday-operation latency

The user reports slow real-world branch switching, pull, push and merge. This slice measures the existing flows and removes shared launch/wait/refresh overhead without weakening write previews or recovery. It adds no UI controls, screens, dashboards or settings.

## Evidence

`cargo run -p gitnova-core --example perf_operations -j1` creates only disposable repositories: 500 tracked files, a feature branch, a temporary bare remote and a temporary writer clone. It records preview, apply and graph/snapshot/stash refresh separately for switch, push, pull and reviewed two-parent merge/finish. The opt-in `measure_git` API records main Git command names/counts and duration only; it never records arguments, paths, URLs, output or credentials. No telemetry/logging or retained measurements are enabled in the app. Isolated scratch commands are included in wall time but excluded from those main-command counters.

On this Mac, eight serial `git --version` probes gave median launch/exit time approximately 24 ms through `/usr/bin/git` and 12 ms through the Git binary selected by `xcrun --find git` (`/Library/Developer/CommandLineTools/usr/bin/git`). Both reported Apple Git 2.39.2 (Apple Git-143). Before the change, one push apply invoked 125 main Git commands; pull apply invoked 167. Small dispatch/wait costs compound throughout each reviewed operation.

The same debug/local-transport fixture gave this single before/after comparison (milliseconds). These are observed core-flow samples, not a statistical latency SLO or a cross-product comparison:

| Flow, including one graph/snapshot/stash refresh | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| Branch switch (average feature/main) | 5117 | 1123 | 78% |
| Push, including preview | 8549 | 2617 | 69% |
| Pull, including fetch and preview | 11771 | 2589 | 78% |
| Merge, including begin/finish previews and finish | 20551 | 6804 | 67% |

Full evidence: [operation-performance.json](verification/2026-10-28-operation-performance.json). These numbers exclude company-network latency, human review time, WebView rendering and RAM soak; the frontend duplicate-refresh savings below are not counted in the table. The branch/merge fixture has no submodules; separate safety regression suites exercise those workflows.

## Changes

- On macOS, respect the first executable in PATH. Only when it is exactly Apple's `/usr/bin/git` dispatcher, resolve the active developer Git once using a bounded two-second `xcrun --find git`. Use that executable for inspection, writes and isolated merge prediction. Fallback keeps normal PATH behavior. Custom Git overrides remain honored; changed PATH/developer/toolchain environment falls back to PATH lookup. No software installation, Git version change or repository configuration change occurs. Toolchain selection is cached for the app lifetime; changing xcode-select externally may require reopening the app.
- Pipe-drain completion wakes the child waiter immediately, replacing a mandatory 10–15 ms polling granularity for short commands. Child ownership, deadlines, process-group termination, output bounds and operation-unknown behavior remain in their original callers. If a child closes pipes early and continues running, bounded polling/backoff prevents a disconnected-channel busy loop. No extra polling threads are added.
- Local branch listing reads the symbolic HEAD rather than scanning the entire dirty worktree just to mark the current branch. The actual switch preview/apply retain working-tree, index, worktree, target, filter/hook and submodule safety checks. This reduces a branch preview from 30 to 18 main Git commands and the sampled push apply from 125 to 107.
- After a write, read one working snapshot and reload the graph only if the history fingerprint changed. Remove the preceding redundant graph read. Initial graph loading passes its known fingerprint into the first snapshot refresh, and explicit Refresh still forces one graph read with a visible busy state. Branch counts, merge state, drafts, graph selection and scroll anchors retain their existing updates.

Preview tokens, per-write revalidation, remote checks, hook/filter execution rules, source/target checks, Git locks and recovery are not cached away or bypassed. Repeated remote inspections and deeper process-count reductions remain possible future work, requiring equivalent race/recovery guarantees. A release-mode package and long-session large-repository/WebView RAM measurements remain separate from this debug benchmark.

## Verification

- Six Git runner/unit checks passed, including PATH precedence, child deadline/status and existing lock ownership behavior.
- 79 real-repository integration checks passed serially: branches (8), submodule merge preview (4), merges (22), refs/push (20), remotes/pull (10), working writes (15). This includes hidden/dirty state, stale reviews, native locks, configured hooks/filter/merge-driver behavior, real submodule pointers, recovery and remote helper timeout.
- All 32 frontend unit checks and TypeScript passed. Seven targeted browser flows passed across focused runs: branch switch and dirty refusal, fetch/local tracking/pull, parent switch preserving a child checkout, external-ref/scroll context refresh, reviewed push preserving dirty work, conflict resolution/two-parent merge, and exactly one snapshot plus one graph read after a branch write. The latter uses the actual IPC command name graph_page.
- Strict Clippy for Git/core/desktop all targets and formatting passed. No developer repository was switched, fetched, pushed or merged; all Git writes and benchmark fixtures are temporary.
- Local debug artifact built and signature/embedded assets verified: `target/debug/bundle/macos/GitNova.app` (78.85 MiB). Verification opened neither a GUI nor a developer workspace.
