# M82 verification — 2026-10-30

Verification uses temporary real repositories, serial runners and one worker.

- Initial expanded investigation suite: 9 passed after refusing configured blame
  ignore files before execution. Covers committed line attribution after rename,
  excluded dirty content, exact original filenames/OIDs/line numbers, input range
  bounds, binary/oversize refusal, registered package identity and duplicate-name
  ambiguity, plus prior literal/follow/gitlink/comparison cases.
- Four focused browser checks initially produced three passes and one failure.
  The failure exposed same-selected-commit detail navigation that cleared data
  without triggering another fetch. The corrected same-OID/historical-path/package
  navigation test passed (9.7 seconds); prior investigation, interactive replay
  and moved-worktree browser regressions passed in that batch.
- Further native checks include alias/workspace constraints remaining unverified.
  Final native rerun: **9 passed** (14.74 seconds), including alias/workspace unknown coverage. Workspace/all-targets Clippy, TypeScript and **53 frontend unit tests** passed. The cached build-script macOS linker advisory remains.

The first browser fixture also expected rename attribution below native Git's
similarity threshold; the fixture was adjusted to preserve enough original text.
The product does not promise attribution beyond native rename heuristics.
No hosted provider or all-platform/native acceptance is inferred from these checks.
