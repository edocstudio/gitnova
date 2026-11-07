# Dependencies and API verification

This table summarizes selected dependency choices and historical verification; it is not a complete direct/transitive inventory, a current security audit or a binary-license compliance result. Exact resolutions are recorded in the lockfiles.

Dependency policy: lock reproducible dependency graphs in Cargo.lock and apps/desktop/package-lock.json. Rust adapters remain private to the core. No gix/git2 dependency until a capability requires it. No UI library, state library, Tailwind or component library is necessary for this small surface; evaluate additions when they reduce actual code or accessibility work.

| Component | Choice | License | Verification |
| --- | --- | --- | --- |
| Desktop | Tauri 2; Rust crate tauri 2.12.5, JS API/CLI 2.12.4 in the lockfiles | MIT OR Apache-2.0 | Official command/config docs, actual cargo check/build |
| Terminal PTY | portable-pty 0.9.0 (desktop adapter only) | MIT | Native shell cwd, input/output, lifecycle and bounds tests |
| Terminal renderer | @xterm/xterm 6.0.0 + @xterm/addon-fit | MIT | On-demand import, bounded scrollback, renderer lifecycle tests |
| UI | React / React DOM 19.3.1 | MIT | npm metadata, typecheck and Vite build |
| Build | Vite 8.3.6, React plugin 6.2.1 | MIT | npm engines; Node 26.13.0 LTS |
| Contracts/storage | Serde 1, serde_json 1 | MIT OR Apache-2.0 | Real serialization and persisted workspace tests |
| Identity | uuid 1 with v4 | MIT OR Apache-2.0 | Rust compilation and identity fixtures |
| Snapshot hashing | sha2 0.10 | MIT OR Apache-2.0 | SHA-256 file/index/HEAD tokens compiled and exercised by stale-write tests |
| Unix process cleanup | libc 0.2 | MIT OR Apache-2.0 | Scoped process-group termination for timed-out Git write children |
| Atomic storage/fixtures | tempfile 3 | MIT OR Apache-2.0 | Persistence and corruption tests |
| Unit tests | Vitest 5.1.0 | MIT | Validated with Node 26; patched npm dependency graph |
| Browser testing | Playwright 1.65.0 | Apache-2.0 | Real-core test bridge; no production bridge |
| Git | Installed Git CLI, tested Apple Git 2.39.2 | GPL-2.0 | External executable, not bundled or linked |

Registry versions above are historical resolution observations, not claims about the latest available versions. The manifests select Tauri major 2. The workspace declares Rust 1.99.1 as its minimum and pins that toolchain; .nvmrc selects Node 26.13.0 and .python-version selects Python 3.15.2; the full dependency graph's minimum supported Rust version has not been certified. Runtime direct dependencies also include the optional hosting adapter's reqwest/url and macOS-only security-framework, plus Moka read caches in core/search. Review all manifests and lockfiles, including target-specific and transitive components, for an actual distribution inventory.

API references:
- https://v2.tauri.app/develop/calling-rust/
- https://v2.tauri.app/reference/config/
- https://vite.dev/guide/
- https://git-scm.com/docs/git-log
- https://git-scm.com/docs/git-diff-tree
- https://git-scm.com/docs/git-worktree
- https://git-scm.com/docs/git-submodule

The app's Apache-2.0 license does not replace dependency licenses. NOTICE contains project attribution, not an audited dependency inventory. Release packaging needs a complete direct/transitive and asset license/notice audit plus verification of the actual artifact; use the [binary-distribution checklist](../contributing/binary-distribution.md). Git is currently an external installation prerequisite. No distributable Git binary is included.

Terminal-specific upstream license texts are retained in [terminal components](../licenses/terminal-components.txt) and copied into the app resources. This does not replace the full transitive dependency distribution audit.

M83 adds an [offline locked inventory and collected license evidence](../licenses/README.md), including explicit unresolved entries and terms/asset review gaps. Local macOS packaging verifies five attribution resources byte for byte and refuses stale lock hashes. Inclusion checks do not establish complete coverage, shipped-component classification or fulfillment of license obligations.

TypeScript 7.0.2 is a project-local build/type-check tool. JavaScript uses the
pinned Node runtime for tooling and the platform WebView in the desktop UI.
HTML/CSS/SVG do not have separately installed language runtimes. Shell scripts
use the OS/runner shells; OS WebView, shell and system-library security updates
remain the responsibility of the supported platform, rather than npm or Cargo.
