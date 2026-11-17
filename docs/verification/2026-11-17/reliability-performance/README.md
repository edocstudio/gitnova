# GitNova 1.0.20 reliability and performance

Source baseline: `e2297e3` (main). Work branch:
`fix/desktop-reliability-and-performance`.

## Behavior

- Diff-to-editor matching avoids splitting/scoring the entire working file when
  the expected line and bounded context already match. Fallback normalizes each
  line once, scores candidates once, and preserves nearest/left tie breaking,
  CRLF and UTF-16 caret offsets.
- A successful file save clears the draft even if the subsequent refresh fails.
  The editor reports success and offers a read-only refresh retry. External edits
  after the save cannot be overwritten by this retry.
- Endpoint review requires one fetch URL and at most one separate push URL,
  matching the single-destination Push contract. Existing multi-URL config remains
  readable; the user explicitly selects the reduction and reviews old/new values.
- Historical child objects share one object batch and one cached-ref reachability
  walk per child, rather than spawning those commands for each gitlink. Inspection
  is limited to 500 outgoing parent commits, 64 path/OID pairs, a 10,001-entry
  sentinel traversal, and existing byte/process budgets. Unobserved objects after
  the traversal cap, unavailable objects and changing refs remain unknown. This is
  offline evidence; native Git's final submodule guard is unchanged.
- Opening an exact required child commit uses the first graph page with a pinned
  inspector/branch action, including orphan objects. Search focus reuses its
  metadata traversal, with the final fresh fingerprint check retained.
- Child publication keeps a temporary header action back to the parent Push.
  Destination and upstream/upload options are restored, but review tokens are not.
  A moved parent source branch requires a fresh Push.
- The stale-child-tracking test now expects native refusal until an explicit child
  Fetch repopulates cached refs. The unused fetch flag was removed without changing
  tag behavior. A related formatting failure was corrected.

## Reproduction

Run verification serially, with two Rust test threads and one Playwright worker.
All test Git writes use disposable repositories. No project repository is used as
an action fixture.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --exclude gitnova-desktop --all-targets --locked -- -D warnings
cargo test --workspace --exclude gitnova-desktop --locked --no-fail-fast -- --test-threads=2
npm --prefix apps/desktop test
node apps/desktop/scripts/bench-edit-location.mjs e2297e3 docs/verification/2026-11-17/reliability-performance/editor-location.json
```

The location benchmark records three sequential samples per baseline/candidate
case, source hashes, Node version and offset equality. It measures pure-function
execution in Node, not native UI response, memory use or physical presentation.

Native workflow driver: `apps/desktop/scripts/native-workflows.mjs`. It requires
an explicit isolated acceptance mode; the native host validates every catalog
location under a temporary fixture. Scripted visible DOM actions create/review a
child branch, publish the child, return to the original parent destination and
review/publish it, migrate the endpoint through confirmation, and reveal/save a
distant diff in a shifted working file. The external runner verifies actual remote
refs, content, HEAD and index. This supplements browser real-core tests without
claiming OS credential, other-platform or physical-screen acceptance.


## Core verification results

The broad workspace run (`--exclude gitnova-desktop --no-fail-fast`, two test
threads) exercised 504 tests: 501 passed, three stale-lock tests initially failed,
and one additional test was ignored. The failures came from their existing global
Git-process probe overlapping another test's deliberately orphaned Git process.
The production recovery guard was preserved. The five stale-lock tests now share
a fixture-local suite mutex and all five pass with two runner threads. All four
submodule diagnostic tests, including the 10,010-commit truncation case, also pass
after the final test-fixture cleanup. No other workspace suite failed.

The complete `ref_and_push` suite passes all 22 tests. Search's 14 tests pass,
including a command-count assertion that focus uses one metadata `log` traversal.
The grouped submodule test asserts one `cat-file` batch and one `rev-list` walk
for multiple historical OIDs in one child.


## Frontend and desktop checks

- `cargo fmt --all -- --check` passed after the related formatting repair.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` passed,
  including the native desktop crate.
- Desktop's 11 native command/IPC/terminal tests passed. Direct `cargo test` emitted
  pre-existing macOS deployment-target linker notes (11.0 versus dependencies
  built for 13.0); packaging explicitly targets macOS 13.0.
- Frontend: 24 unit files / 96 tests passed. The location tests include 400
  deterministic oracle comparisons and a 4.2-million-UTF-16-unit CRLF fixture.
- Nine affected browser workflows were exercised with the real Rust core. Eight
  passed on the broad targeted rerun. The newly added moved-parent case found a
  navigation reset clearing its explanation. The explanation now uses persistent
  repository feedback; both child/parent flows pass after this fix. Other covered
  flows: saved-file refresh failure and read-only retry, explicit multi-URL
  reduction, endpoint migration/review/locale, redirected URL recovery, historical
  working-file saves, and distant diff reveal in both layouts.
- The parent-return flow checks a custom destination, upstream option, upload
  option state, fresh review, absence of automatic publication and a 1024-pixel
  header without horizontal overflow. A moved parent branch discards stale Push
  context and does not create the proposed child recovery branch.

### Location benchmark

Three sequential samples per implementation, identical result offsets. Timings
are medians in milliseconds on local Node; see `editor-location.json` for exact
samples, runtime and source hashes.

| Synthetic fixture | Baseline | Candidate |
| --- | ---: | ---: |
| unique-20k | 3.10 | 0.40 |
| repeated-131072 | 35.40 | 0.21 |
| repeated-1048576 | 265.47 | 1.64 |
| repeated-4194304 | 1063.37 | 6.27 |
| repeated-4MiB-no-match | 53.31 | 5.73 |


## Packaged native verification

`npm run build:desktop` completed in the release profile for macOS arm64, minimum
macOS 13.0. The embedded UI check, strict deep ad-hoc signature validation and all
five bundled attribution resources passed. No trusted signing/notarization claim
is made. The attribution inventory was regenerated: its existing manual audit is
still incomplete (67 unresolved entries and 27 manual terms reviews, unchanged
from the baseline). This local update is not a public-distribution clearance.

`native-workflows.json` records successful scripted DOM actions in the visible
packaged WKWebView through native IPC and real disposable repositories. The
external runner independently verified both published destination refs, unchanged
original remote branches, the migrated endpoint, an empty replacement remote
(no automatic publication), the exact corrected working text, and unchanged
parent HEAD/index. A first harness attempt compared macOS `/var` versus
`/private/var` aliases literally after its UI flows passed; the fixture root now
uses native canonical identity and the complete rerun passes.

### Foreground limit

The 30-minute foreground attempt stopped after 88.17 sampled seconds and six
completed interaction cycles when WebView visibility changed from visible to
hidden. This is recorded as failed acceptance in `foreground-soak.json`, not as a
completed soak. Read-only console/process evidence is in
`foreground-diagnostics.json`; no app processes remained, and no display/session
settings were changed. Native workflow acceptance above passed separately; a
30-minute foreground RAM result remains unverified.


### Completed background soak

`background-soak.json` passed after 1,805.36 sampled seconds and 153 interaction
cycles in the packaged release app. It used 50,000 commits, 10,000 tracked files,
10 locations, two linked worktrees, two submodules, 200 tracking branches and ten
merge commits. The final fixture checks passed for HEAD, checked-out branch,
staging, refs and working content; only the designated clean fixture performed
reviewed branch roundtrips. No crash or driver failure occurred.

Native RSS peaked at 221.34 MiB; its last-five-minute median was 148.92 MiB.
The sampled combined native/WebKit/Git peak was 603.98 MiB. Warm native RSS slope
was -2.64 MiB/minute; WebKit slope was +2.11 MiB/minute, with rises and drops during
the run. The instrumented formatter count settled at four, canvas context count
at one and pending callback count ended at zero (peak two).

These are background/hidden-WebView measurements, not foreground presentation
latencies or proof that no memory leak exists. WebKit helper attribution can
include unrelated helpers newly launched during the interval; summed RSS can
double-count shared pages and sampling can miss short-lived Git peaks. JS heap
and GPU allocation were unavailable. A controlled foreground run and allocation
profile remain the right follow-up for explaining WebKit's positive warm slope.
There is no controlled before/after app-memory comparison in this delivery.

The native workflow, soak and packaged executable hashes match. No production
source changed after the build; only LF/CRLF browser coverage was expanded while
waiting for the serial soak to complete.


## Final delivery

The final four-case LF/CRLF × Unified/Side-by-side browser matrix passes. This
brings the affected real-core browser coverage to eleven unique workflow cases.
Backend editor text is normalized to LF and existing CRLF restoration remains
unchanged; the additional cases validate the complete offset/section/textarea
flow rather than adding an unnecessary normalization path.

Installed 1.0.20 over Desktop/GitNova.app 1.0.19, retaining the previous app in a
task-owned temporary backup. Source, built and installed versions match; native
workflow, completed background soak, built and installed executable SHA-256 values
match. Strict deep signature verification, all five installed attribution
resources and installed `--verify-bundle` pass. Installed native smoke also passes
catalog IPC, real Git graph IPC, duplicate-instance reuse and fixture preservation.
See `installed-bundle.json` and `installed-native-smoke.json`.

Outstanding validation: uninterrupted 30-minute foreground interaction, real
provider/credential/network scenarios, other-platform GUI acceptance and the
existing incomplete attribution audit. No remote push of the source branch or
public application release was performed.
