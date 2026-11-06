# Version tag inventory verification — 2026-11-06

## Confirmed gap and change

Create tag previously listed local tags from the graph's refs snapshot. That
snapshot cannot include remote-only tags, and can be stale after external changes.
A successful branch fetch does not guarantee a complete local tag inventory.

The existing tag review now loads fresh local tags independently of graph
pagination and reads tags from the selected remote's actual push endpoint with
`ls-remote --refs --tags`. Names are merged with source labels, searched across
the returned inventory, sorted numerically and displayed five per page. Known
local or selected-remote duplicates block creation. Review checks remote names
again to catch updates after opening the list. Final create/push guards remain.
Inspection does not fetch tags, change refs, or update checkout/staging. A 4 MiB
remote output limit fails explicitly rather than reporting a partial list as
complete. Offline local-only mode remains available; other remotes are checked
by selecting them, not silently claimed to have been inspected.

## Checks actually run

- `cargo test -j 1 -p gitnova-core --test tags -- --test-threads=1`: 3 passed.
  Includes packed local tags on an older commit, annotated remote-only tags,
  different fetch/push URLs, invalid remote selection, publication conflict guards,
  and preservation of HEAD, index and working content.
- `npm run check`: passed.
- `npm test`: 17 files, 69 tests passed.
- Targeted Playwright: 2 passed with one worker. Covers original annotated tag
  create/push/verify and the new remote-only inventory, three-page search,
  duplicate prevention after the remote changes, and local-only mode.
- `cargo clippy -j 1 -p gitnova-git -p gitnova-core --all-targets -- -D warnings`:
  passed with `MACOSX_DEPLOYMENT_TARGET=13.0`.

Only disposable local repositories were used for Git write tests. No real
provider credentials or project remotes were used to verify this change.
No Windows/Linux hosted run or full application regression suite is claimed.
The local macOS build remains debug/ad-hoc, not a notarized release.

## Local artifact

`npm run build:local` passed. Updated the existing Desktop/GitNova.app while it
was closed, verified its ad-hoc code signature and embedded UI with
`--verify-bundle`, and matched its executable SHA-256 to the build output:
`bf5b21a13e3a84736d602931452b5528720626b4b2d734618e7ec0b5bc513735`.
Existing bundle-size/dynamic-import warnings remain; bundled attribution
inventory reports 50 unresolved items and does not certify license compliance.
