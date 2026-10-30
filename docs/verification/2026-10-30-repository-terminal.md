# Repository terminal verification — 2026-10-30

Scope: M75's graph Terminal action, desktop PTY adapter and bounded renderer.
Local macOS checks only; runners were serialized to limit machine load.

## Executed checks

- Desktop native suite: **5 passed**. Three terminal cases cover real PTY cwd with
  spaces/quotes/shell metacharacters, user input and output/EOF, membership
  isolation, four-session admission, explicit close/reaping and slot reuse, output
  retention and dimension limits. The two existing native IPC/Git scenarios also
  passed, with external-origin terminal commands and unknown membership refused.
- After the reload-epoch guard was added, the three terminal tests passed again.
  Unix shell fixtures use isolated HOME and `/bin/sh`, without personal startup
  files. Other OS shell behavior was not exercised here.
- Frontend TypeScript check and **53 unit tests across 13 files passed**, including
  matching English/Thai catalogs and FAQ structure.
- Two targeted browser tests passed: actual graph action/lazy dock, selection
  preservation, Hide/reopen without duplicate startup, independent repository
  sessions, typed input, resize and confirmed End session. PTY transport is
  emulated in browser tests; workspace/graph results use the real Rust core in
  temporary Git repositories. Native PTY behavior is covered separately above.
- The graph test initially caught StrictMode's duplicate mount/start. Startup now
  checks cancellation after one microtask before PTY admission; no timeout or
  retry suppression was added. The browser group passed after that correction.
- Desktop all-target Clippy with warnings denied, formatting and whitespace checks
  passed. Clippy reported a toolchain deployment-target linker advisory (Rust
  objects target macOS 13 while the check linker default is 11); the local bundle
  build explicitly targets macOS 13. The production bundle runs frontend
  typecheck/build as well.

## Resource and behavior limits

At most four retained sessions, 256 KiB native output per session, 64 KiB output
batches, 2,000 scrollback lines, a bounded paste queue and input worker admission.
Renderer output callbacks gate the next read. These are bounds, not a long-term
RAM/CPU benchmark. User commands and detached programs have their own resource
usage. No whole-app soak or Linux/Windows certification was performed here.

Shell commands run directly, outside app Git previews and host commit identities;
GitNova does not inject saved provider credentials into the shell. Normal shell
startup/history behavior remains user-controlled. No user repository commands,
authentication or remote writes were executed for the tests.

## Packaging

The local debug macOS bundle built successfully, with production frontend build,
ad-hoc signature and embedded UI asset verification. Terminal license-resource
contents match the retained upstream texts. The terminal renderer is a separate
lazy-loaded chunk; the existing main frontend chunk-size advisory remains.
After app closure, the existing `~/Desktop/GitNova.app` was updated in place.
Installed ad-hoc signature, matching executable SHA-256, embedded UI verification
and terminal license-resource contents all passed. No alternate app was created,
and the app was not launched into a workspace.
This is a non-notarized local build. No commit, push, PR or release was performed.
Bundled terminal license texts do not certify a full transitive license audit.
