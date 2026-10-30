# Conflict choice UI verification — 2026-10-30

Scope: existing owned Merge/Cherry-pick/Revert/Rebase conflict workflows. This
change is frontend interaction/layout plus documentation; native snapshot,
resolution preview, staging and operation-finish validation remain in place.
No new dependency, provider setting or automatic Git write was added.

## Checks

- TypeScript checking passed during implementation.
- All 55 frontend unit tests passed, including reconstruction/revision/clearing
  of independent choices, preservation of outside text and CRLF, empty-side
  deletion, diff3-base exclusion and malformed markers.
- Seven serial browser scenarios passed in 2.3 minutes using the real Rust core
  and fresh temporary repositories: merge editing/finish; cherry-pick/revert;
  two-block diff3 choices with undo/revisit; direct checkbox entry, English/Thai,
  manual edits and draft return; a conflicting Revert; external-edit refusal;
  and owned Rebase resolution/Continue with its target branch preserved.
- Before resolution confirmation, the relevant scenarios assert working-file
  and index bytes remain unchanged. Finish assertions check actual parentage,
  staged content and conflict removal. External edits refuse review and retain
  the in-memory draft.
- A 900×760 browser fixture checks that the file groups do not overlap resolution
  controls and exercises keyboard enlargement through the existing dock separator.
  Screenshot inspection exposed the old three-row/120px minimum mismatch; the
  inspector now provides room for a full file row per group and scrolls when short.

After the final row-height/default-dock adjustments, the narrow direct-choice
scenario passed again (14.8s total runner time). The final 55-unit suite passed.
English/Thai catalogs have identical 2,165-key sets; JSON parsing, 200 local
relative documentation links, whitespace and the existing bounded privacy-pattern
scan passed. External URLs and screenshot contents are not certified by that scan.

## Local app

The final local build passed TypeScript/frontend compilation and macOS ad-hoc
debug packaging. Existing chunk-size/notarization advisories remain; this is not
a notarized public release. The five attribution resources and embedded UI passed
bundle verification. With the app closed, the existing canonical Desktop
GitNova.app was updated; deep/strict signature verification, system-PATH
`--verify-bundle` from `/tmp`, and source/installed executable equality passed.
No alternate installed app or user repository window was opened.

Installed executable SHA-256:
`79112109a9fd27199afe89bb004ca814a8d76a69bd0393539a6973fc7f109954`.

The browser invokes real Git through the test bridge. It is not proof of every
native WebView/OS combination, arbitrary conflict topology, binary resolution or
semantic correctness of concatenating both text versions. All fixture writes
were isolated from the user's repositories. No commit, push or release was made.
