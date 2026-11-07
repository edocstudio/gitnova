# Toolchain upgrade — 2026-11-07

This review updates development/build tools, not the GitNova product version
(1.0.0). It is not a security certification or a comparative performance result.

## Versions and scope

| Tool | Previous project selection | New selection |
| --- | --- | --- |
| Rust / Cargo | Rust minimum 1.98, CI floating stable | 1.99.1, pinned in rust-toolchain.toml and CI |
| Node.js | .nvmrc 20.20.2, CI 22 | 26.13.0 LTS, pinned in .nvmrc and CI |
| Python | Runner/system default | 3.15.2, pinned in .python-version and CI |
| TypeScript | 5.9.3 | 7.0.2, project-local npm dependency |
| Vite / React plugin | 8.3.5 / 6.1.2 | 8.3.6 / 6.2.1 |
| Vitest / Playwright | 4.1.12 / 1.64.1 | 5.1.0 / 1.65.0 |

Stable releases were checked against the [Rust release announcement](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/),
[Node downloads](https://nodejs.org/en/download), [Python downloads](https://www.python.org/downloads/)
and the [TypeScript 7 announcement](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/).
Exact npm versions are resolved in the lockfile. Node 27 prereleases are excluded.

JavaScript build scripts use Node; desktop JavaScript runs in the OS WebView.
Python is used by QA, benchmark and license scripts, not the installed application.
HTML, CSS and SVG have no standalone compiler version to update. Shell,
PowerShell, WebView and OS libraries remain platform-provided prerequisites.
This change does not replace users' system installations. React, Tauri and the
Cargo dependency graph remain unchanged; upgrading a compiler is separate from
upgrading every library.

The local Rust verification installation is isolated under ignored target/;
Python was installed through uv and Node used an existing matching installation.
Contributors should install Rust through rustup, run nvm install / nvm use if
using nvm, and select Python 3.15.2 for repository QA (for example,
`uv python install 3.15.2` and `uv run --python 3.15.2 python ...`).

## Verification

Local verification on macOS Apple Silicon, run sequentially with one Cargo
compilation job and two Rust test threads:

- Rust formatting and core/workspace Clippy with warnings denied: passed.
- Core/workspace library tests: 69 passed, 1 ignored. Integration suites and
  Windows/Linux native GUI acceptance are not covered by this result.
- Frontend TypeScript check and Vite production build: passed.
- Vitest: 17 files, 70 tests passed.
- Playwright 1.65.0 / Chromium 158: three real-core browser regressions passed (history/workspace navigation, stage/unstage/commit and branch switching) using disposable repositories. No native WebView/provider acceptance is implied.
- Node prerequisite/packaging/release preparation tests: 15 passed, 1 skipped
  because local GPG was unavailable.
- Python 3.15.2 QA/license/benchmark tests: 8 passed.
- Workflow YAML parsed; toolchain pins, npm lockfile engine consistency and local Markdown link targets checked.
- macOS local debug/ad-hoc bundle built with Rust 1.99.1 and Node 26.13.0; embedded UI verification, codesign verification and five bundled license resources passed. Installed over the existing Desktop GitNova.app and verified version 1.0.0, binary hash equality and signature. This is not a signed/notarized public installer.

Vite still reports a large frontend chunk and an existing mixed static/dynamic
StashControls import. Node emits an experimental localStorage warning in some
unit-test workers. These are not resolved by this toolchain change. No new
long-session RAM or user-operation speed claim is made.

## Security and attribution limits

`npm audit --json` reported zero vulnerabilities before and after the update.
This describes the registry's findings, not proof that all packages are safe.

An [OSV querybatch check](2026-11-07-toolchain-osv.json) inspected all 536 public crate/version pairs in
Cargo.lock. It is not a cargo-audit run. The existing Linux GLib 0.18.5 finding
RUSTSEC-2024-0429 / GHSA-wrw7-89jp-8q8g remains. The current GTK/WebKit/Tauri
constraints cannot use the fixed GLib line through an isolated compatible
version bump. No incompatible override or advisory suppression was introduced.
The proc-macro-error, zbus_names, zvariant and zvariant_derive maintenance
advisories also remain; aliases and multiple resolved versions are not counted
as separate vulnerabilities. See the [previous review](2026-11-05-security-and-performance.md)
for the underlying dependency paths and follow-up requirements.

License evidence was regenerated against the updated npm lockfile and current
macOS target: 403 components, 246 unique texts collected, 67 unresolved entries,
27 manual terms reviews and 98 assets. Optional native TypeScript packages add
platform-specific evidence gaps. `auditComplete` remains false. Original
upstream notice bytes, including CRLF/trailing spaces, are preserved; only the
generated evidence file has a scoped whitespace attribute exception.
Binary license inclusion checks do not establish full license compliance.
