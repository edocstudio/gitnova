# Stash and standalone index conflict resolution

The conflict editor previously required an owned merge session. A conflicted
stash Apply/Pop creates unmerged index entries without MERGE_HEAD, so it fell
through to ordinary staging controls and unsupported-conflict guidance.

The native state reader now reports an `unmerged` inspection phase for these
entries. Resolution reuses the existing per-file editor and explicit review/apply
protocol without creating a synthetic merge session. Other native operations
retain their ownership restrictions. Conflicts are excluded from ordinary
staged/unstaged lists. A conflicted stash application opens the resolver after
refresh; each successful resolution selects the next conflict. Resolving the last
file returns to normal Working changes, preserving the stash, branch, HEAD,
unrelated staged files and unstaged bytes. No automatic commit or stash removal
occurs.

Preview authority covers the current index, HEAD, branch, configuration and the
selected file's bytes/mode. Confirmation rechecks authority and captures the
working file, raw index and conflict-stage blobs in a private recovery directory
before replacing/staging only the selected file. Marker-bearing working/edited
results are rejected. Whole-file choices handle binary/deleted results; gitlink
conflicts remain separately inspected. File/patch/recovery limits remain bounded.

Validation on macOS, using disposable repositories and synthetic content:

- `cargo test -p gitnova-core --test stashes --test merges -j 1 -- --test-threads=1`:
  17 stash tests and 24 merge tests passed. This includes standalone conflicts,
  stale-preview rejection, per-file resolution and explicit deletion.
- Final targeted native recheck after fingerprint refinements:
  `cargo test -p gitnova-core --test stashes conflicts -j 1 -- --test-threads=1`:
  2 tests passed.
- `cargo clippy -p gitnova-git -p gitnova-core --all-targets -j 1 -- -D warnings`:
  passed with `MACOSX_DEPLOYMENT_TARGET=13.0`.
- `npm run check`: passed; `npm test`: 17 files / 69 tests passed.
- Playwright, one worker: Thai conflicted Pop automatically opening the editor;
  standalone two-file conflicts surviving reopen and preserving unrelated staged
  and unstaged content; existing merge block drafts/language-switch behavior:
  3 tests passed. A first run exposed failure to select the next unresolved file;
  that behavior was fixed and the cases passed on rerun.
- `git diff --check`: passed.

No developer repository conflicts were resolved, staged or discarded by these
checks. Hosted CI and Windows/Linux runtime execution were not run for this task.

Local packaging: `npm run build:local` passed. Installed over the existing
Desktop/GitNova.app after verifying it was closed. The installed bundle passed
`codesign --verify --deep --strict` and `--verify-bundle`; its executable hash
matched the built artifact. This remains a debug/ad-hoc signed macOS alpha,
without notarization. Existing bundle-size warnings and incomplete third-party
license audit are unchanged; this task does not certify license compliance.
