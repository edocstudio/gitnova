# Large stash review verification — 2026-10-30

## Defect and corrected scope

`Complete stash review exceeds the patch budget.` came from treating a 512 KiB
aggregate code-display limit as an operation-admission condition. M85 separates
bounded text from complete reviewed path/content identity. Both Push and saved
Apply/Pop/Drop use the corrected preview. No user repository or real credential
was used for verification, and no stash was applied to the user's checkout.

## Actual checks

- Final native stash suite: **15 passed**, one Cargo job/test thread, 74.74s.
  Covers large working/index/untracked text, bounded preview, omitted-prefix
  file inspection, stale tail edits, exact cross-branch Pop with staging retained,
  saved index-only paths and literal pathspecs, index-only gitlink blocking,
  conflict retention, packed refs, collision/lock/hook guards and SHA-256 Git.
- Frontend: TypeScript check and **55 unit tests passed**.
- Browser/real-core bridge: **6 passed**, one worker, 2.5 minutes. New large-stash
  flow inspects a file outside the aggregate prefix and switches Working/Index
  comparisons before Stash/Pop, then compares complete restored bytes and saved
  staging. Existing advanced Apply/Drop/draft, Thai conflicted Pop, cross-branch
  graph, packed-ref fallback and changed-branch review scenarios also passed.
- Native Tauri command test: **1 passed**, 27.63s. Includes a positive selected
  stash-file patch, invalid path scope and rejection of untrusted remote origins.
  This is a command/mock-runtime test, not a real WebView gesture test.
- Workspace/all-targets Clippy with warnings denied passed. The existing macOS
  build-script linker advisory remains; this is not an all-platform CI claim.

The initial old oversized-preview assertion was updated to verify a bounded,
read-only successful preview instead of expecting rejection. Additional fixtures
distinguish a failed receipt from an admission error, and explicitly construct a
valid saved-index-only shape rather than assuming every Git version creates it
from the same staged-added/deleted working state.

All write fixtures were newly created temporary repositories. Native snapshot
revalidation, selected content hashes, immutable stash identity, operation locks,
collision/gitlink protection and recovery/removal checks remain in place. No
patch cap was raised and no automatic retry, commit or push was introduced.

The code preview does not certify semantic correctness or remove the existing
16 MiB selected working-file hash bound, 20,000-path/metadata limits, supported
stash shapes or Pop/Drop metadata restrictions. See [M85](../milestone-85.md).

After a final defensive parent lookup, the selected saved-index/literal-path
regression passed again, followed by Git-library/all-targets Clippy. The final
local build passed frontend TypeScript compilation, bundling, five attribution
resource checks and embedded UI verification. With no user app running, the
canonical Desktop GitNova.app was updated. Deep/strict signature checks,
system-PATH `--verify-bundle` from `/tmp` and executable hash equality passed.
The artifact remains a macOS ad-hoc debug app, without notarization or a public
release. No alternate installed app was created.

Installed executable SHA-256:
`03cb50ea9f2fe1560164716f1d652eddc26686df55b78ec0e9cd22aacf7b3baa`.

JSON/catalog parsing and the identical 2,172-key English/Thai sets passed.
All 206 relative file links in changed/new documentation resolved locally;
whitespace and bounded source-text privacy-pattern checks passed. These do not
certify external URLs, image content, comprehensive secret absence or other OSes.
