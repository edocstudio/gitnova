# GitNova product and interaction rules

GitNova is a graph-first, multi-repository desktop Git client. Preserve repository and worktree identity and real Git integration.

For every workflow, follow the product rules in `docs/ux/workflows.md`: make current context, selected scope, next action and resulting state clear. Keep working files, staged content and commit history visibly distinct. Apply these rules to future features as well as current screens.

Before adding a screen, sidebar, panel, dialog, button or navigation item, document why the capability cannot be delivered naturally in the existing graph context. Favor fewer interactions and stable placement. Reuse existing graph, inspector, picker and diff dock surfaces. Do not add decorative walkthroughs or duplicate controls.

Prioritize clear, ordered interactions. Build original code-native UI, icons and assets; do not copy proprietary implementations or artwork. Preview and confirm consequential Git writes in the existing context; never discard, stash, force, merge or rebase implicitly. Keep failures and recovery honest. Test writes only in temporary repositories, never in the developer's project repositories.

Product UI strings and accessible labels use the frontend i18n lookup with entries in both English and Thai catalogs. Preserve repository/workspace names, refs, paths, commit messages/authors, code and typed drafts as original data. Do not translate by rewriting the DOM or mutating native Git DTOs. Language changes must preserve in-progress UI state.

# Resource limits during verification

Run only one build/test runner at a time. Cargo compilation defaults to two jobs in .cargo/config.toml; use --test-threads=2 for Rust integration suites and retain the single Playwright worker. Finish and inspect one suite before starting another. A stalled executable launch is not a reason to start repeated copies: keep at most one diagnostic probe, terminate only task-owned stalled probes, and collect read-only system evidence before resuming. Do not launch the native app or heavy verification while investigating a machine freeze.

# Persistent settings

User direction, 2026-10-28: persistent application/workspace/repository settings belong in the header Settings dialog, with explicit scope. Do not add connection forms or preferences to commit details. Keep Git action inputs and reviewed write confirmations in their existing graph context.

CI account scope: global host/company connections in Settings, independent of repository, branch and commit. GitLab/GitHub account identity is provider plus normalized HTTPS host/base path; Jenkins identity is controller plus username. Match repositories by validated fetch remote, never folder/project names or host substrings. Keep companies isolated, preserve Keychain credentials and explain global disconnect effects.

User correction, 2026-10-28: Settings contains global host/account Connections; remove the repository-remotes section. Repository destination setup and hosted-repository creation belong to the existing Push context when a destination is missing. Keep reviewed repository creation, local remote configuration and code publication as distinct, honestly reported effects.
