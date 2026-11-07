# Stable release readiness

The owner requested version 1.0.0 only when distribution is ready. No version
has been changed and no public release has been approved by this check.

## Verified hosted evidence

Source `408be280a66efca55ae7be548df5ffae993d2789`, candidate run
[42355615724](https://github.com/edocstudio/gitnova/actions/runs/42355615724):

- macOS ARM64 desktop passed, including all browser workflows and the extracted
  release-profile app's native startup/IPC checks. The ZIP is ad-hoc signed,
  not notarized. SHA-256:
  `e6a9ede17e4b4142b7538ffaf06265097707c3979edf14a2610c9be35f3eb719`.
- Linux x64 desktop passed, including all browser workflows, release-profile
  Debian and AppImage startup/IPC checks. Debian SHA-256:
  `c590480319f54f14ff90718f8d669490bf05fdc01a3710bab9e7859b9894279b`.
  AppImage SHA-256:
  `7506154a5008dfcce0e6036cdec588a479a41a3367e934f63771adfbc64cd779`.
- Windows native IPC/PTY tests passed. Desktop browser execution timed out
  after 90 minutes (117 passed, 17 failed, one interrupted), so no Windows
  installer was produced by that run. Windows core failed on invalid
  tab-containing fixture paths;
  commit `233748e` fixes those paths while retaining Unix coverage.
- The candidate run for `233748e`,
  [42358047112](https://github.com/edocstudio/gitnova/actions/runs/42358047112),
  was canceled after a newer tag-form defect was identified. Results from
  different source commits cannot be combined into
  one approved release. The owner has merged that fix into main via PR #33.

These are historical observations, not live CI status. Read each linked run's
current result before drawing a release conclusion.

The subsequent main check run
[42359101210](https://github.com/edocstudio/gitnova/actions/runs/42359101210)
exposed a tag-form loading race on Linux: remote discovery briefly enabled the
name input before tag inventory disabled it again. The resulting empty name
kept Review disabled. Tag inputs now remain disabled until inventory is ready;
the retry action remains enabled after inventory failure. Local real-Git browser
checks passed for graph/context/tag creation, delayed/failed inventory retry and
complete remote tag inventory (3 tests). Desktop TypeScript, shell syntax,
non-runner installation refusal, workflow YAML parsing and diff whitespace also
passed. No local full app build or native Windows/Linux installer run was
performed for this change. Hosted execution is still required.

## Additional installation coverage

This change adds actual apt install/reinstall/purge checks and NSIS
reinstall/uninstall checks on disposable hosted runners. Both preserve the exact
installer digest in lifecycle evidence. The previous Debian check only extracted
the package. Same-version reinstall does not establish previous-version upgrade
or user-data migration compatibility. Hosted execution remains required.
Read-only inspection of the verified Debian archive confirms package identity
`git-nova`, executable `/usr/bin/gitnova-desktop` and attribution resources under
`/usr/lib/GitNova/licenses`; the install guard uses that actual package identity.

## Remaining release gates

- Successful core, desktop and native installer checks for one exact source
  commit on all three platforms, followed by four-package assembly.
- Review of actual shipped dependency/asset obligations. Current inventories
  explicitly remain incomplete; the refreshed macOS inventory has 49 unresolved entries
  and 27 component-specific review items. Inclusion of license files is not
  completion of that review. See [license evidence](../licenses/README.md).
- Native manual acceptance and supported OS/architecture scope. Current
  candidates cover macOS ARM64, Windows x64 and Linux x64 only.
- Signing remains an explicit owner choice. No owner GPG key, Apple Developer
  ID or Windows certificate has been created. The approved unsigned alpha path
  does not itself authorize stable 1.0.0 or suppress platform security warnings.

Until these gates are resolved, keep 0.1.0 alpha and the website download feed
unpublished. Do not rename older artifacts as version 1.0.0: changing version,
signing or repackaging requires rebuilding and verifying the resulting bytes.

## Windows fixture follow-up

Inspection of the completed Windows desktop log identified path assertions using
Node temporary-directory 8.3 aliases/backslashes against canonical paths returned
by native Git. Fixture roots now resolve to their native canonical spelling;
path-label assertions accept the Windows separator spelling without weakening
checks of the target checkout. The unavailable-path assertion accepts Windows'
actual OS error message as well as the Unix messages.

Native Git intentionally removes `GIT_*` environment overrides. The disposable
browser-test HOME now contains `core.autocrlf = false`, matching fixture creation
and ensuring that child clones, submodules and worktrees use the same byte policy.
This changes no application behavior, user configuration or runner-wide Git
configuration. Content, dirty-work protection, recovery-ref and write-result
assertions remain in place; no timeouts were increased and no tests were skipped.

Local TypeScript checking and 16 targeted real-Git browser workflows passed
(2.3 minutes), covering submodule initialization/update, clone, initialization,
worktree creation/removal/repair/prune/move, unavailable repositories and guided
branch/Pop/pull preparation. This is macOS verification of the affected flows;
Windows execution remains necessary to confirm the platform fixes and investigate
any remaining disabled-control failures.

## Confirmed installer lifecycle and conflict retry

PR #34 run [42361207973](https://github.com/edocstudio/gitnova/actions/runs/42361207973)
passed Linux desktop checks, including all browser workflows, actual apt
install/reinstall/purge and AppImage native startup. The tested PR merge source
is `0c3808363101b68a013ece32dbcae606a721958a`, not a final release revision.
Debian SHA-256:
`aa0d096561749d6ccaf6fb686c9221b4cc3e2bfc37c98df860fa3d012ba176a6`.
AppImage SHA-256:
`6989b465ec00d786a1bbd92fbd559ce9a5be2436c1a4666c9f0e8da0f00f7dce`.
Downloaded lifecycle evidence binds the Debian digest to successful installation,
installed native acceptance, same-version reinstall and purge.

The same run exposed a macOS conflict-preview race (138 browser tests passed,
one failed). After an external edit was refused, the file observation refreshed
but the parent working snapshot could remain old. A later polling tick then
invalidated the retry preview. The refusal now refreshes the file and parent
observations while retaining the draft and requiring explicit review again.
No content is written by that refresh.

Three affected conflict workflows passed three repetitions each locally (9
passes). A strengthened regression pauses periodic observation, retries and
resumes observation: it fails on the old implementation and passes three
repetitions with the fix. TypeScript checking also passed. macOS CI now retains
failure traces, as the other desktop runners already do. No suite was weakened
or removed.

Supplemental license evidence now includes pinned original texts for the
`dlopen2` crates and the MIT option for the application-indicator binding, plus
Mozilla's canonical MPL 2.0 text for `selectors`. Evidence validation and offline
macOS collection passed (392 components, 248 unique texts, 49 unresolved,
27 component-specific reviews). Collection remains explicitly incomplete;
source availability, generated SDK bindings, assets and bundled native libraries
must not be represented as approved by these text additions.

## Hosted diagnostic bounds

The older Windows core log continued to pass tests; its long duration was not
an established deadlock. Core jobs now have a 90-minute bound. Platform browser
jobs stop at the first failure so failure traces upload before the job deadline;
a green run still executes every browser test. This does not turn a failed test
into success, remove coverage from a successful run or raise per-test timeouts.

## Native path and rebase diagnostics

The completed Windows log for main run
[42359101210](https://github.com/edocstudio/gitnova/actions/runs/42359101210)
identified worktree failures caused by ordinary Git paths being compared with
Rust verbatim paths, and verbatim destinations being passed back to Git.
Git absolute path decoding now normalizes Windows prefix syntax without a
filesystem lookup; worktree writes use the existing validated Git path encoder.
Missing registrations retain their original location. Relative submodule URL
sync uses Git's native top-level spelling when no parent remote is configured.
The same-repository push fixture now forms a valid Windows file URL.

The four affected Rust integration suites passed locally (62 tests), and
targeted Clippy passed. Windows-only prefix tests cover disk, UNC, missing,
already-verbatim, relative and invalid UTF-8 inputs. Hosted Windows results
remain necessary; local macOS results cannot establish Windows acceptance.

Candidate run
[42364459684](https://github.com/edocstudio/gitnova/actions/runs/42364459684)
exposed a Linux browser test reading `HEAD^2` while rebase was replaying a
temporarily linear commit. The test now polls the existing HEAD parent list
until the recreated two-parent merge is checked out on the expected branch.
All topology, other-branch and working-tree assertions remain. Three local
repetitions passed, as did TypeScript checking.

Windows candidate `7d1636a` passed 31 browser workflows before its stash
round-trip assertion expired. The retained trace shows inventory completed and
the selected patch still loading, without a blocked operation or data error.
That asynchronous readiness assertion now uses the same 45-second operation
budget as the adjacent large-stash workflow, rather than the 15-second default.
No global assertion timeout or product validation was weakened. Three complete
local stash round trips passed with staging, untracked text, draft preservation,
apply, drop, pop, recovery and HEAD assertions intact.

Candidate `02cd576` also caught a Windows-only Clippy useless-conversion warning
in the relative URL sync change. Both platform branches now return an owned
string without that conversion. A focused Windows diagnostic mode is available
to check these regressions before repeating full installer validation; it cannot
assemble or publish release artifacts.

Focused Windows run
[42368048679](https://github.com/edocstudio/gitnova/actions/runs/42368048679)
passed formatting, targeted Clippy, the Windows path unit test and 52 of 53
applicable integration cases. The remaining case failed during fixture setup
when re-adding a removed checkout using its Rust canonical verbatim path.
The fixture now supplies the original ordinary path to native Git, retaining
all membership, branch, recovery, staged/working content and retired-identity
assertions. The corrected case passed locally. The diagnostic Rust suites use
two test threads, matching local integration verification. UI diagnostics did
not execute in that failed run; they are not counted as passed.

## Targeted distribution evidence

Focused Windows diagnostic [42371179624](https://github.com/edocstudio/gitnova/actions/runs/42371179624)
at `755bc7d` passed all 53 Rust integration cases and all three intended browser
regressions: advanced stash round trip, reviewed worktree creation/open and
two-parent merge recreation during rebase. This is regression evidence, not
installer or entire-suite approval. All 70 frontend unit tests also passed
locally.

Read-only inspection of the earlier Linux AppImage (SHA-256
`6989b465ec00d786a1bbd92fbd559ce9a5be2436c1a4666c9f0e8da0f00f7dce`)
found 418 file/link entries, 175 regular shared-library files and 111 native
package copyright notices. Those notices referenced common license texts not
present in the image. The Linux configuration now includes the build host's
`/usr/share/common-licenses` directory, and candidate verification requires every
referenced text to resolve inside the extracted AppImage. Synthetic checks cover
missing texts, trailing sentence punctuation and an escaping symlink. A rebuilt
native AppImage must pass this check; the earlier archive is not retroactively
approved. License choices and source/relinking obligations remain separate.

The corresponding Debian archive contains ten regular files: the application,
desktop entry, three icons and five attribution resources; no `.so` files are
bundled there. This does not establish the licenses of code statically linked
into the executable.

Windows diagnostic [42369297150](https://github.com/edocstudio/gitnova/actions/runs/42369297150)
passed all 53 applicable Rust integration cases and the complete advanced stash
round trip. Recreated-merge UI verification still encountered the 15-second read
deadline while the trace showed the native operation applying, with no error.
That topology poll now uses the existing 45-second write budget, with a
120-second total fixture budget for setup, preview and application. The
two-parent, source branch, target branch and clean-working-tree checks remain.
The worktree UI case did not execute after this failure and is not counted as
passed.

The full Windows desktop job at `dbb452c` passed native IPC/PTY and packaging
evidence checks, then failed its native EN/TH date-output conformance test at
the five-second unit-test deadline (16.3 seconds observed with one worker).
Only that conformance test now permits 30 seconds. Its native-output assertions
and the separate formatter-construction bound remain unchanged; no product
performance result is inferred from this CI allowance. All four date tests
passed locally after the adjustment. Hosted installer validation must run again
with the correction.

Offline collection for `x86_64-pc-windows-msvc` now preserves the original MIT
texts for the three WebView2 Rust bindings, pinned to the published crate VCS
revisions. The collection has 397 components, 249 unique texts and 37 unresolved
entries; none of those unresolved entries is a Rust crate. The remaining entries
are npm packages absent from this macOS cache, including optional platform build
tools. This cross-target collection is not native Windows installer validation.

A local Vite production build inspected in memory found code contributions from
six npm packages: `@tauri-apps/api` 2.12.4, `@xterm/addon-fit` 0.11.0,
`@xterm/xterm` 6.0.0, `react` 19.3.1, `react-dom` 19.3.1 and `scheduler`
0.28.0. Collected license texts exist for all six. This narrows frontend review;
it does not turn the entire development dependency inventory into a shipped
component list, classify bundled native libraries or approve SDK terms.

The source uses system font stacks rather than distributing font files. The
brand vector master and generated icon family are recorded by
`apps/desktop/scripts/generate-brand.mjs`. Documentation screenshots in the broad
asset inventory are not automatically included in installers. Final artifact
contents still need to be matched to the corresponding source and obligations.

The Linux PR job at `509d2a1` exposed a progress-inspection race: discard
completed between the separate visible/progress-maximum browser assertions.
The affected fixture now holds transport delivery after a real Rust discarding
event until its busy-UI assertions and screenshot finish, releasing the hold in
`finally`. Git execution, recovery-file contents, pending-merge state and final
UI assertions remain real and unchanged. Three local repetitions passed. This
is test synchronization, not an artificial delay in application operations.

Release candidate `509d2a1` passed Linux and macOS desktop checks. Downloaded
release-profile packages were independently hashed: macOS ZIP
`079c94ea73f24fc4cf8718b976ac76fe55d4f032b2c6e834bf3bccc9bd697050`,
Debian `feb5defc686805809c85ad7d8456bd4a24a004dbaa63344686ced99a89d23f41`,
AppImage `3b17c23e5ec329a1de8cd15afcca93d9c912398586dbfca45b2c213a02a8641c`.
The macOS ZIP passes ad-hoc code-signature and five-resource attribution checks.
Read-only SquashFS inspection matched all 111 native notices and 15 referenced
common texts to the new AppImage report. Linux's 16 packaging/signing checks
passed, including signing exact synthetic bytes with a disposable GPG key;
no owner key was created. These packages are still unsigned alpha candidates.

That Windows desktop run passed 32 browser workflows before the Thai stash Pop
readiness assertion expired at 15 seconds. Its retained trace shows the button
enabled after that deadline, without a blocked reason. This specific readiness
assertion now uses the adjacent stash workflow's 45-second budget. The complete
Thai conflict-resolution and discard workflows passed twice locally, retaining
stash recovery, HEAD, resolved index and content assertions.

Windows browser execution now uses four separate hosted shards, one worker on
each runner. Listing the actual Playwright selections verifies 35 tests per
shard, all 140 tests selected exactly once. Fully-parallel registration enables
test-level sharding within the single graph spec; worker count stays one.
Only shard 1 builds the Windows installer, avoiding duplicate package artifacts.
Every shard must succeed for the reusable workflow to permit release assembly.
Rust suite thread limits remain unchanged. Hosted validation of this revision
is required before claiming Windows or same-source release approval.

## Sharded Windows follow-up

Candidate [42376588849](https://github.com/edocstudio/gitnova/actions/runs/42376588849)
at `f26033c` passed Linux desktop packaging, Ubuntu/macOS core checks and
one Windows browser shard. The Windows packaging job has not completed.
This does not approve the complete candidate: other Windows shards
exposed 15-second readiness deadlines while real merge/submodule writes were
still applying. The retained failure contexts show disabled Applying/Updating
controls, rather than a completed operation or a reported rejection.

Only the three observed completion assertions (large merge, merge retaining a
child checkout, and reviewed submodule update) now use the existing 45-second
write budget. The submodule fixture allows 180 seconds for setup, review and
completion. Each affected workflow passed twice locally; all existing tree,
parent, child-ref, index and content assertions remain. These are macOS
regression results; hosted Windows execution is still required.

Each Windows shard now reports all 35 selected tests rather than stopping at
its first failure, within the unchanged 90-minute job limit and single worker.
Distinct shard names identify failures in the Actions summary. Failed tests
still fail their job and prevent assembly. No application runtime guard,
performance target or global browser assertion timeout was changed.

The Windows packaging shard at `f26033c` subsequently passed all 35 browser
tests and reached license collection. Collection correctly refused supplemental
texts whose exact hashes changed after Git's Windows CRLF checkout conversion.
The upstream evidence directory now has `-text` attributes, preserving original
bytes instead of normalizing them or relaxing digest checks. A disposable Git
fixture with `core.autocrlf=true` reproduces the mismatch without the attributes
and verifies every supplemental file against its pinned hash with the fix.
Both evidence tests and local offline Windows-target collection passed; that
collection still explicitly records 37 unresolved candidates and is not audit
approval. A new hosted Windows installer build remains necessary.

The Linux PR check at `358ab9d` located and selected the correct old search
result but missed keyboard focus. App-level animation-frame focusing could run
before the virtual history mounted the target row after scroll restoration.
History now acknowledges a bounded, repository/selection-scoped focus request
when that row mounts. The request is consumed once; subsequent viewport changes
do not reclaim focus. Six local search/failure-navigation repetitions passed,
then two strengthened workspace-search repetitions verified that moving focus
to Refresh and scrolling history preserves that focus. TypeScript passed.
Hosted validation of this change remains required; this is not installer or
manual team acceptance.

All core jobs, including Windows, completed successfully on candidate
`f26033c`. Windows PR shard 4 at `358ab9d` completed 33 browser cases and
reported two stash-preparation readiness failures. Its trace still shows
Reading changes with the reviewed Stash button disabled while the selected
file patch loads. Only these two readiness assertions now use the existing
45-second budget. Both complete preparation flows passed twice locally,
including cancellation, explicit carry, original saved-object selection,
working-file contents and unchanged branch before confirmation. All 35 tests
remain selected; no runtime write guard or global timeout was relaxed.
