# Conflict Result highlights

The existing Result editor now marks the output of checkbox resolution choices
without adding a panel or altering the content submitted for review. Blue means
Current, amber Incoming, and green both or manually edited resolved content.
Ranges are calculated against the resolved output, accounting for earlier
choices, CRLF, diff3 base exclusion, and empty/deleted blocks. Deletions mark a
position rather than implying that deleted lines remain in the result.

Typing/paste/undo remaps annotations using the changed prefix/suffix span;
external draft replacement clears obsolete annotations. Scrolling translates
only the background layer, with no text duplication and no React state update
per scroll event. Ranges and line lookup are bounded by the existing editor
size limit. Review/Save/Stage behavior is unchanged.

Validation on macOS:

- TypeScript check passed.
- Frontend unit suite: 21 files, 82 tests passed. Added coverage for output
  offsets, CRLF, both-side selection, manual edits and deletion positions.
- Targeted browser suite: 2 passed; final rerun also passed both tests.
  Tests use the real Rust bridge and disposable Git repositories, check the
  number/type of highlight bands, preserve language-switch behavior, and confirm
  no file/index writes before review/apply. Cherry-pick completion was verified.
- Browser screenshot inspected: chosen Current line has a blue full-width band
  in Result, with ordinary neighboring lines unmarked.
- Version-script regression test passed; metadata synchronized to 1.0.7 and
  attribution inventory regenerated (no dependency changes).
- Diff whitespace check passed.

The full browser suite and native Windows/Linux GUI were not run. Annotation
ranges are draft-local and are cleared when an external draft replaces the
result; they are not persisted as repository metadata.

Delivery: `build:local` passed. Desktop/GitNova.app was updated in place while
GitNova was not running. Installed Info.plist reports 1.0.7; the installed
executable matches the built bundle. Strict/deep signature validation and
`--verify-bundle` passed without opening a user workspace. This remains a local
debug, ad-hoc signed bundle, not a notarized or published release.
