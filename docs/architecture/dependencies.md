# Dependencies and API verification

This table summarizes selected dependency choices and historical verification; it is not a complete direct/transitive inventory, a current security audit or a binary-license compliance result. Exact resolutions are recorded in the lockfiles.

Dependency policy: lock reproducible dependency graphs in Cargo.lock and apps/desktop/package-lock.json. Rust adapters remain private to the core. No gix/git2 dependency until a capability requires it. No UI library, state library, Tailwind or component library is necessary for this small surface; evaluate additions when they reduce actual code or accessibility work.

| Component | Choice | License | Verification |
| --- | --- | --- | --- |
| Desktop | Tauri 2; Rust crate tauri 2.12.5, JS API/CLI 2.12.4 in the lockfiles | MIT OR Apache-2.0 | Official command/config docs, actual cargo check/build |
| UI | React / React DOM 19.3.1 | MIT | npm metadata, typecheck and Vite build |
| Build | Vite 8.3.5, React plugin 6.1.2 | MIT | npm engines; Node 20.x (20.19.0+) or 22.12.0+ |
| Contracts/storage | Serde 1, serde_json 1 | MIT OR Apache-2.0 | Real serialization and persisted workspace tests |
| Identity | uuid 1 with v4 | MIT OR Apache-2.0 | Rust compilation and identity fixtures |
| Snapshot hashing | sha2 0.10 | MIT OR Apache-2.0 | SHA-256 file/index/HEAD tokens compiled and exercised by stale-write tests |
| Unix process cleanup | libc 0.2 | MIT OR Apache-2.0 | Scoped process-group termination for timed-out Git write children |
| Atomic storage/fixtures | tempfile 3 | MIT OR Apache-2.0 | Persistence and corruption tests |
| Unit tests | Vitest 4.1.12 | MIT | Compatible with Node 20; patched npm dependency graph |
| Browser testing | Playwright 1.64.1 | Apache-2.0 | Real-core test bridge; no production bridge |
| Git | Installed Git CLI, tested Apple Git 2.39.2 | GPL-2.0 | External executable, not bundled or linked |

Registry versions above are historical resolution observations, not claims about the latest available versions. The manifests select Tauri major 2. The workspace declares Rust 1.98 as its minimum, and .nvmrc selects Node 20.20.2; the full dependency graph's minimum supported Rust version has not been certified. Runtime direct dependencies also include the optional hosting adapter's reqwest/url and macOS-only security-framework, plus Moka read caches in core/search. Review all manifests and lockfiles, including target-specific and transitive components, for an actual distribution inventory.

API references:
- https://v2.tauri.app/develop/calling-rust/
- https://v2.tauri.app/reference/config/
- https://vite.dev/guide/
- https://git-scm.com/docs/git-log
- https://git-scm.com/docs/git-diff-tree
- https://git-scm.com/docs/git-worktree
- https://git-scm.com/docs/git-submodule

The app's Apache-2.0 license does not replace dependency licenses. NOTICE contains project attribution, not an audited dependency inventory. Release packaging needs a complete direct/transitive and asset license/notice audit plus verification of the actual artifact; use the [binary-distribution checklist](../contributing/binary-distribution.md). Git is currently an external installation prerequisite. No distributable Git binary is included.
