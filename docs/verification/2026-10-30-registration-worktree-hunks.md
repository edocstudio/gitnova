# M77 verification — submodule registration, moved worktrees and text hunks

Local macOS verification, 2026-10-30. Git writes used temporary repositories and
local temporary remotes. No user repositories, hosted publication or real
credentials were used as test targets. Checks ran serially with one Cargo job,
one Rust test thread and one browser worker.

## Implemented and checked

- Add/download or register an existing independent submodule checkout; retain
  unrelated staged content and the parent HEAD. Commit the staged gitlink and
  manifest using the application commit flow.
- Remove registration, then commit removal; retain dirty child files, child
  commits and absorbed object storage. Recovery includes the parent index,
  manifest, common/config.worktree configuration when present, an original-file
  presence manifest and the reviewed request. Recovery files are private.
- Reject stale previews, occupied paths, edited manifests, foreign native locks,
  unsafe paths and retained metadata collisions without overwriting them.
- Reattach a moved linked worktree through its existing Git locator; preserve
  dirty working bytes, index, HEAD, branch and stable catalog identities. Reject
  a wrong repository or changed locator.
- Stage/Unstage added/deleted text hunks, including an unborn repository with no
  initial index. Preserve working bytes and unrelated staging. Refuse new-file
  conversion settings rather than bypass native Git line-ending/encoding rules.

## Commands and actual results

From the repository root, with `CARGO_BUILD_JOBS=1` and local deployment target 13:

```sh
cargo test -p gitnova-core --test hunks --test related_maintenance --test submodule_lifecycle --locked --offline -j1 -- --test-threads=1
cargo test -p gitnova-core --test amend --locked --offline -j1 -- --test-threads=1
cargo clippy --workspace --all-targets --locked --offline -j1 -- -D warnings
```

Final targeted Rust run: **24 passed** (9 hunk, 11 maintenance, 4 registration).
Amend regression suite: **13 passed**, including exact index/unstaged preservation,
linked worktrees, merge parents, stale previews and recovery refs.
Clippy completed successfully. A toolchain linker advisory about a cached build
script's macOS deployment version appeared; it was not an application test failure.

An earlier broader regression run also passed **59 tests**, covering the then
8 hunk tests, 12 linear Rebase, 11 maintenance, 4 registration, 11 worktree
creation and 13 worktree removal tests. Subsequent edits to the three targeted
suites were rerun in the final 24-test command above. This is not a full workspace
test-suite certification or hosted Linux/Windows result.

From `apps/desktop`:

```sh
npm run check
npm test -- --maxWorkers=1
npm run test:e2e -- --grep 'submodule registration lifecycle|moved worktree repair|submodule newer version|Stage and Unstage hunk|metadata.*dirty'
```

TypeScript passed. Vitest: **53 tests / 13 files passed**. Final Playwright:
**5 passed**, exercising existing text hunk controls, worktree lock/unlock,
newer-submodule review, registration Add → Commit → Remove, and moved-worktree
reattach. Browser tests use the real Rust core bridge, not a native WebView
transport or OS-signing certification. Earlier failed fixtures were corrected
for the explicit bridge request import and picker readiness/canonical temporary
path; no operation refusal was removed to obtain a pass.

`git diff --check` passed after the edits. A local check of **155 relative Markdown
link targets** in the changed current documentation found no missing targets.
Added source/documentation lines were checked for the previously prohibited
competitor/company identifiers; none were found. This is not a full repository
secret audit.

After the owner closed GitNova, `npm run build:local` passed. The local debug,
ad-hoc-signed macOS bundle was copied over the existing Desktop/GitNova.app.
`codesign --verify --deep --strict` passed; built and installed executable
SHA-256 hashes matched. The installed `--verify-bundle` check passed from /tmp
with a minimal PATH, verifying embedded UI assets without opening a workspace.
No alternate app copy was created, running app terminated or notarization claimed.

Installed executable SHA-256:
`2bdc6f64600b546320a8d1bda8a87cc26bca1514cecaa90a833fc01b38a204fa`.

## Limits retained

This increment does not implement topology-preserving/root/unrelated-history
Rebase, interactive reordering, worktree metadata prune, arbitrary move discovery
or automatic nested-checkout transitions. Registration Add/Remove stages the
parent changes for review; generic pointer actions remain separate and do not
claim complete registration restaging/unstaging support. Text conversion,
binary, mode-changing, non-UTF-8 and incomplete hunk patches retain file-level
fallbacks. No benchmark or long-session RAM claim is derived from these tests.
