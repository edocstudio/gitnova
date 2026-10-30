# M83 — local distribution evidence

The existing local macOS build now packages and verifies project LICENSE/NOTICE,
terminal attribution, a locked dependency inventory and collected third-party
texts. No new application surface, dependency, release publication or credential
change is introduced.

The offline collector records exact component versions, available license
declarations/text hashes, missing evidence and asset hashes. It includes build/dev
candidates and does not claim exact binary contribution classification. The
bundle verifier refuses stale lock hashes, missing resources and differing bytes.

The inventory remains explicitly incomplete. Public distribution still requires
review of missing texts, component-specific terms/source obligations, shipped
components and asset provenance. Ad-hoc debug packaging does not establish
Developer ID signing, notarization, release installers or Windows/Linux native
acceptance. Those are separate gates, not implied by a local build.

See [verification](verification/2026-10-30-distribution-evidence.md) and
[license evidence](licenses/README.md).
