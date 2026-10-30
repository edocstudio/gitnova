# Milestone 74 — Connection commit identities

Authentication and commit authorship are independent. An authenticated host
account does not replace Git's configured author email, and a provider login
cannot establish a verified commit email.

Implemented in the alpha:

- Optional explicit name/email per normalized provider/host in existing Settings
  → Connections. Save and clear locally without calling provider APIs or editing
  Git configuration files. Identity rules remain independent of token expiry or
  disconnect; removing the connection also removes its identity override.
- Match fetch `origin`, otherwise the sole remote, using validated host/path
  boundaries. Refuse ambiguous rules/remotes and fall back to native Git settings
  only when no rule matches. Do not infer an email from a login.
- Root-scoped command configuration for commit, amend, merge/replay, stash and
  annotated-tag workflows. Restore scoped context on return/unwind; separate
  synchronous workers do not share mutable identity/environment state.
- Show the effective committer in the existing composer and operation review.
  New commits use the configured author/committer; native amend, cherry-pick and
  rebase retain original authors. Settings never rewrite existing history.
- Include effective Git configuration in ordinary commit tokens; keep amend and
  operation configuration guards. Bind per-file commit review to the same token,
  and use consistent identity scope for snapshots and operation discard review.
- English/Thai UI and FAQ guidance.

Names/emails are metadata stored in the local application-data
`commit-identities.json` file, independently of Keychain credentials. Git signing,
SSH configuration and provider access scopes remain managed by their existing
systems. Native Git helper/hook subprocesses can inherit command-local `-c`
options; direct adapter calls for a different child root do not receive a parent
identity override.

See [verification](verification/2026-10-30-connection-commit-identities.md).
