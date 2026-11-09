# Editor syntax colors and terminal welcome

The existing Edit file dock now highlights the visible section of YAML, JSON,
JavaScript/TypeScript (including JSX/TSX), Python, Rust, HTML and CSS. The matching
Lezer grammar loads on demand from bundled assets. Unknown formats remain exact
plain text. No provider call, code execution or remote grammar download occurs.

The native textarea still owns text input, selection and clipboard behavior.
Colored React text nodes form a noninteractive, accessibility-hidden mirror;
both scroll axes track the textarea. Colors follow the existing dark/light
theme. Source resembling HTML is rendered as text, never inserted as HTML.
Highlighting is limited to the current section (65,536 UTF-16 units maximum)
and 4,096 styled tokens. Failed or over-budget highlighting falls back to plain
text. Section boundaries can interrupt multiline constructs, so syntax colors
are an aid rather than whole-file validation. Existing 4 MiB read/save, stale
snapshot checks and draft preservation remain in place.

New terminal sessions display an original blue/cyan GitNova ASCII heading,
the package version and localized slogan. Narrow terminals use a compact title.
The frontend writes this only to the terminal display after opening succeeds;
it is never written through the PTY input API. Hiding/reopening an existing
session keeps the original buffer without restarting the shell or banner.
No startup scripts, shell history, authentication or command execution change.

React review: per-file language loading is cancelled on unmount, parsed tokens
are memoized by the visible text/parser, read-only text avoids re-parsing on
draft edits, source uses escaped text nodes, the native textarea remains labeled,
and existing terminal disposal/session ownership is preserved. No new controls
or settings surface was added.

Validation on macOS Apple Silicon, sequential runners:

- `npm run build`: TypeScript/Vite passed; language grammars are separate chunks.
- `npm test`: 20 files, 77 tests passed, including parser/input/token bounds,
  exact-text preservation and narrow/display-only terminal banner tests.
- Targeted Playwright: 4 passed (2.2 minutes including the core fixture build).
  Cases cover existing draft/stale-save behavior, a multilingual file above
  512 KiB, colored YAML, noninterpreted HTML-like source, scroll alignment,
  theme changes and exact whole-file save with unchanged HEAD/index.
- Final Terminal regression: 1 passed (7.2 seconds), checking a single welcome
  across hide/show, a single shell open, and exact `echo test` input through the
  emulated PTY transport. Graph/workspace reads use real disposable repositories;
  this browser check does not certify a real native PTY implementation.
- Dark/light editor and terminal banner screenshots were inspected locally.
- npm install reported zero known vulnerabilities in its dependency audit.
  Offline attribution collection includes the ten added MIT parser packages;
  existing 67 unresolved inventory entries remain, with `auditComplete: false`.
- No new native RAM soak or Windows/Linux manual acceptance was performed.
- `npm run build:local`: passed. Installed over the existing Desktop
  `GitNova.app` (1.0.1); strict ad-hoc signature, matching built/installed binary
  SHA-256 and `--verify-bundle` passed. All five attribution resources were
  included. No real workspace was opened. This is a local debug/ad-hoc build,
  not a notarized public installer.
