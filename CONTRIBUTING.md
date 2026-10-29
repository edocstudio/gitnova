# Contributing to GitNova

Thank you for helping make Git easier to understand. Contributions can be a small documentation fix, an English/Thai translation, a reproducible bug, a focused Git fixture or an implementation change.

## Before you start

Follow the setup in [README.md](README.md#getting-started). Read the [development guidelines](docs/contributing/development.md) and [workflow principles](docs/ux/workflows.md). For larger features, open a [GitHub issue](https://github.com/edocstudio/gitnova/issues) to discuss the intended behavior and scope before implementation.

Keep the Git graph central. Prefer an action in the existing graph, inspector, picker or diff dock over adding another screen. Document why a new surface is necessary before adding one. Keep Rust domain and Git services independent of Tauri.

## Report a bug

For a non-security bug, use [GitHub Issues](https://github.com/edocstudio/gitnova/issues). Include the GitNova version, OS, Git version, exact steps, expected/actual behavior and the relevant redacted error or screenshot. For Git-state issues, describe branch/upstream, staged/unstaged state, submodules/worktrees and any pending operation. A minimal disposable repository that reproduces the behavior is especially helpful.

Remove tokens, private remote URLs and proprietary code from anything you post publicly. Recovery folders may contain original files: share only the specific redacted evidence needed for the report. Never publish credentials or details of an unpatched vulnerability in a public issue, discussion or pull request. Follow [SECURITY.md](SECURITY.md) for the verified reporting-channel status and safe contact procedure; an ordinary bug report is not a private security channel.

Public documentation screenshots must use disposable repositories and synthetic accounts/data. Review visible remote URLs, authors, file contents and paths before adding an image; never capture a workplace project for public documentation.

## Source-build setup

Follow the [README setup steps](README.md#install-the-source-build-tools-first) before starting Tauri. Rust/Cargo and OS build libraries are separate from Node/npm. From `apps/desktop`, run `npm ci` and `npm run doctor`; the normal `npm run tauri dev` also checks basic tools before starting. If installation has just finished, reopen Terminal so Cargo is visible in `PATH`. The checker does not install tools or prove all platform libraries are available.

## Make a focused change

1. Create a branch in your checkout or fork and keep the change small enough to review.
2. For Git writes, define the preview, exact scope, revalidation, receipt and recovery behavior before enabling the action. Do not implicitly stash, discard or force an operation.
3. Put Git fixtures in temporary directories. Never switch, reset, clean, merge or otherwise modify a developer's real project as test setup.
4. Add product text through `t(...)` with matching English and Thai catalog entries. Preserve repository names, refs, paths, code and typed drafts as original data.
5. Explain the final behavior, relevant limitations and checks in your pull request. Include screenshots for visible interaction changes.

## Run relevant checks

Run one suite at a time. Start with the integration target affected by your change rather than repeatedly running every suite.

From the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --exclude gitnova-desktop --all-targets --locked -j 1 -- -D warnings
cargo test --workspace --exclude gitnova-desktop --locked -j 1 -- --test-threads=1
```

Frontend checks, from `apps/desktop`:

```sh
npm ci
npm run check
npm test -- --maxWorkers=1
npm run build
```

For browser workflows, install Chromium once, build the real-core test bridge from the repository root, then run the relevant browser flow:

```sh
cargo build -p gitnova-core --example ui_bridge --locked -j 1
cd apps/desktop
npx playwright install chromium
npm run test:e2e -- --grep '<workflow name>' --workers=1
```

The test transport uses real Git repositories and Rust services but emulates the browser/native IPC connection. It does not certify the native WebView or live hosting credentials. Desktop compilation requires the platform's Tauri dependencies. See the [full verification reference](docs/implementation.md#verify) and [CI workflow](.github/workflows/ci.yml) for broader checks.

## Project scope and licensing

Use the [delivery priorities](docs/delivery-priorities.md) and [architecture plan](docs/architecture/overview.md) to place larger work. Describe a feature as implemented only when it has real Git integration and suitable verification. Avoid performance or platform claims that have not been measured.

GitNova is [Apache-2.0 licensed](LICENSE). Contributions intentionally submitted for inclusion are governed by section 5 of that license unless explicitly stated otherwise; section 5 also preserves any separate license agreement that applies to a contribution. This clarification does not introduce a CLA, DCO or mandatory sign-off requirement.

Keep third-party license/attribution requirements intact and use original or appropriately licensed code and assets. [NOTICE](NOTICE) records project attribution, not a completed dependency-license audit. Before distributing a binary, review the [license and artifact checklist](docs/contributing/binary-distribution.md), including direct/transitive components and assets. Build success alone does not establish distribution compliance.
