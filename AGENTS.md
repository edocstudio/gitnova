# Session startup and handoff

Before planning or changing files, read [.docs/HANDOFF.md](.docs/HANDOFF.md).
It records the current task, completed work, remaining validation, relevant files
and evidence. Then inspect `git status --short`, `git branch --show-current` and
`git log -5 --oneline` in this repository. Verify the recorded version against
`apps/desktop/package.json` and `Cargo.toml`; reconcile stale notes with the actual
checkout and the latest user request. Preserve unrelated edits.

Do not restart completed work, rerun long verification merely to reconstruct
context, or ask the user to repeat information already recorded. A pending item
in the handoff is context, not a replacement for the current user request. Past
confirmation that the desktop app was closed is not evidence that it is closed
now; inspect current processes before packaging or installation.

Update `.docs/HANDOFF.md` at meaningful task boundaries and before ending a
session with incomplete work. Record the result, branch/base or relevant commits,
exact next step, checks actually run and their limits, and any active task-owned
process or blocker. Include the handoff update in the task commit when committing.
Keep durable rules and user preferences here; keep the handoff concise and link
to detailed evidence under `docs/`. Do not store secrets, credentials, raw private
logs or a copied conversation in either file. Do not invent a task's own final
commit SHA before it exists; the next session can obtain it from Git.

# GitNova product and interaction rules

GitNova is a graph-first, multi-repository desktop Git client. Preserve repository and worktree identity and real Git integration.

For every workflow, follow the product rules in `docs/ux/workflows.md`: make current context, selected scope, next action and resulting state clear. Keep working files, staged content and commit history visibly distinct. Apply these rules to future features as well as current screens.

Before adding a screen, sidebar, panel, dialog, button or navigation item, document why the capability cannot be delivered naturally in the existing graph context. Favor fewer interactions and stable placement. Reuse existing graph, inspector, picker and diff dock surfaces. Do not add decorative walkthroughs or duplicate controls.

Prioritize clear, ordered interactions. Build original code-native UI, icons and assets; do not copy proprietary implementations or artwork. Preview and confirm consequential Git writes in the existing context; never discard, stash, force, merge or rebase implicitly. Keep failures and recovery honest. Test writes only in temporary repositories, never in the developer's project repositories.

Product UI strings and accessible labels use the frontend i18n lookup with entries in both English and Thai catalogs. Preserve repository/workspace names, refs, paths, commit messages/authors, code and typed drafts as original data. Do not translate by rewriting the DOM or mutating native Git DTOs. Language changes must preserve in-progress UI state.

# Resource limits during verification

Run only one build/test runner at a time. Cargo compilation defaults to two jobs in .cargo/config.toml; use --test-threads=2 for Rust integration suites and retain the single Playwright worker. Finish and inspect one suite before starting another. A stalled executable launch is not a reason to start repeated copies: keep at most one diagnostic probe, terminate only task-owned stalled probes, and collect read-only system evidence before resuming. Do not launch the native app or heavy verification while investigating a machine freeze.

# Persistent settings

User direction, 2026-11-17: after completing requested code changes, commit the task changes on a descriptive branch and report the branch name, commit SHA, commit message/comment and verification result. Preserve unrelated existing edits outside the task commit.

User direction, 2026-11-19: every task that changes GitNova code must advance
the version, build a fresh macOS release `.app` containing the changes, install it
at `Desktop/GitNova.app`, and verify the installed version before reporting the
task complete. Apply this to UI, styles and small fixes as well as larger changes.
Documentation-only work does not require a version bump or build.

Use Semantic Versioning: patch for compatible fixes, minor for backward-compatible
features, and major for breaking changes. Advance from the current source version
and check the installed version to avoid reusing or rolling back a delivered
version. Use `apps/desktop/scripts/set-version.mjs` to synchronize metadata,
regenerate the native attribution inventory, and run the macOS release build.
Check that GitNova is closed before packaging and replacing the app. Verify the
installed bundle's version matches the source and that its embedded UI passes
verification. Report the installed version with the branch, commit and checks.

User direction, 2026-10-28: persistent application/workspace/repository settings belong in the header Settings dialog, with explicit scope. Do not add connection forms or preferences to commit details. Keep Git action inputs and reviewed write confirmations in their existing graph context.

Release settings exception (2026-11-25): configure repository overlay mappings in its Releases inspector. Global Settings chooses the visible release environments.

CI account scope: global host/company connections in Settings, independent of repository, branch and commit. GitLab/GitHub account identity is provider plus normalized HTTPS host/base path; Jenkins identity is controller plus username. Match repositories by validated fetch remote, never folder/project names or host substrings. Keep companies isolated, preserve Keychain credentials and explain global disconnect effects.

User correction, 2026-10-28: Settings contains global host/account Connections; remove the repository-remotes section. Repository destination setup and hosted-repository creation belong to the existing Push context when a destination is missing. Keep reviewed repository creation, local remote configuration and code publication as distinct, honestly reported effects.

# Code quality

Use idiomatic, explicit imports and clear resource ownership. In Python prefer
focused `from module import name` imports, with aliases for ambiguous names;
avoid wildcard imports and use qualified imports when they clarify ownership.
Keep typed, bounded caches and effect/subprocess cleanup explicit in TypeScript
and Rust. Validate runtime preconditions with errors, not Python `assert`
statements that disappear under optimization. Refactor the touched workflow
coherently, preserve behavior, and verify affected callers; avoid repository-wide
style rewrites mixed into a defect fix.
