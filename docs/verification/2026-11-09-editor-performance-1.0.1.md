# Editor measurements and version 1.0.1

The previous working-file edit fix was measured before making further runtime
changes. No additional runtime optimization is included in this follow-up:
the observed improvement already has a concrete cause (bounded DOM text), and
this evidence does not identify a new Git-operation bottleneck.

## Reproducible, isolated measurement

Run `npm run bench:editor -- /absolute/path/to/report.json` from `apps/desktop`
with the project's Node version and installed Playwright Chromium. The default
output is ignored `target/editor-benchmark.json`. No native app, provider account,
Git write or project file is used. The source uses the production section splitter.

The [JSON report](../benchmarks/editor-layout-2026-11-09.json) retains all 28
samples, machine/browser identity, source revision and limitations. Seven samples
per fixture/profile alternate which profile runs first. Each sample uses a fresh
browser context, populates a read-only baseline and textarea, waits two animation
frames, types one character at the end, and waits for painting again. Profiles
share the same bounded layout CSS; text length is the independent variable.
This reconstructs whole-document versus section rendering, rather than comparing
two installed native binaries. IPC, React state updates, save and other controls
are outside this layout experiment. Frame scheduling and automation overhead
remain part of these measurements. No results or failed samples are discarded.

| Synthetic UTF-8 file | Whole-document render median | Section render median | Whole-document type/paint median | Section type/paint median |
| --- | ---: | ---: | ---: | ---: |
| 630,000 bytes | 274.6 ms | 39.5 ms | 58.4 ms | 29.1 ms |
| 1,680,000 bytes | 650.7 ms | 39.9 ms | 103.1 ms | 31.3 ms |

All 28 typing checks passed. Section rendering held 32,760 UTF-16 code units
in each visible field; full-document fields held 390,000 and 1,040,000 units.
Observed JavaScript heap medians were approximately 2.98/4.70 MiB for the two
whole-document cases, versus 2.36 MiB for sections. These are **JavaScript heap**
samples, not process RAM; they exclude native DOM/layout, GPU and other app
allocations. They cannot prove long-session RAM stability or native WebKit
performance. The complete draft is still kept in memory and saved as a whole.

The supplemental JSON does not replace `docs/benchmarks/latest.json` or update
the website's historical benchmark snapshot. Its different workload/schema must
not be presented as a speedup in branch switching, Pull, Push or Merge.

## Version configuration

`npm run version:set -- 1.0.1` synchronizes Cargo workspace version, first-party
Cargo lock entries, desktop package and root npm lock metadata, and Tauri config.
Dependencies remain unchanged. It does not commit, tag, publish or install.
The UI reads `apps/desktop/package.json`; rebuilding is necessary to update an
installed app. Usage is also documented in the root README.

License inventory was regenerated offline: only lockfile provenance hashes
changed, with 67 unresolved entries still recorded (`auditComplete: false`).

## Checks

- Version synchronization and release preparation: 5 passed, 1 optional GPG test
  skipped. No signing key was created.
- `npm run build`: passed (TypeScript and Vite).
- `npm test`: 18 files, 72 tests passed.
- `npm run build:local`: passed with one Cargo build job. Installed 1.0.1 over
  the existing Desktop `GitNova.app`; installed version, strict ad-hoc signature,
  binary SHA-256 equality and `--verify-bundle` all passed. No real workspace was
  opened. This local debug/ad-hoc bundle is not a notarized public installer.
- No dependency upgrade, Git mutation in user project repositories, long-session
  soak, Windows/Linux acceptance or hosted CI run was performed for this follow-up.
