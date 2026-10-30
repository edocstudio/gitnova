# M76 verification — reviewed newer submodule revision

Local verification on macOS, 2026-10-30. Git writes used temporary fixture
repositories only. No company repositories, real credentials, publication or
remote settings were used for verification.

## Behavior verified

- The existing Manage context separates Restore recorded from Review newer
  version. The latter shows the configured cached remote ref, exact destination
  commit and complete bounded changed-path evidence before confirmation.
- A forward update changes only the selected child checkout. Parent index bytes,
  unrelated staged work and local child branch refs remain unchanged. The parent
  gitlink becomes an unstaged pointer change for separate review/staging.
- Dirty, divergent, missing-target, stale-ref/config and nested-layout cases
  refuse updates. Replacement/grafted history cannot authorize ancestry.
- An additional oracle test compares the reviewed destination against native
  `git submodule update --remote --no-fetch --checkout` in a temporary fixture.
  Its configured remote URL is deliberately unreachable: cached refs suffice.
- Browser checks cover the existing Initialize and Restore flows alongside the
  new flow. Review heading/confirmation stay visible in the scrolled Manage area.
  English and Thai lookup catalogs and FAQ content were updated together.

## Checks actually run

Run serially with one Cargo build job and one browser worker:

| Check | Result |
| --- | --- |
| Core integration: submodule_updates, related_maintenance, hunks, rebase | 47 passed |
| Subsequent submodule_updates suite after additional guards | 21 passed |
| Additional configured-child-remote/native-Git oracle | 1 passed; 21 filtered out |
| Frontend TypeScript and unit suite | Passed; 53 tests across 13 files |
| Targeted real-core browser flows after sticky controls change | 3 passed in 50.9 seconds |
| Workspace/all-target Clippy, locked/offline, warnings denied | Passed after replacing chunks_exact with as_chunks |

These runs overlap; their counts are not a count of distinct tests. The final
oracle test was added after the 21-test suite and workspace Clippy run. The
Clippy command emitted a cached build-script linker deployment-target advisory;
this is not a claim that all toolchain output was warning-free.

The first regression attempt exposed an incompatible early stale-preview error;
the implementation now preserves the existing operation-receipt contract. The
initial frontend check also found a missing translation key, corrected before
the passing run. No failing test was skipped or given a longer timeout.

## Scope and remaining evidence

This is a cached-ref, proven-forward operation, not automatic fetch/update or
support for arbitrary divergent history. Open/fetch the child first for fresh
server evidence. Submodule add/remove and nested-layout transitions remain
outside this increment. Hooks/filters may run only during confirmed checkout;
postcondition failures require inspection and do not imply rollback.

No new long-duration RAM benchmark or cross-platform hosted CI run was performed
for this increment. Existing M73 measurements remain separate evidence; this
change does not establish a new speed/RAM claim or overall readiness percentage.

`npm run build:local` passed with the combined submodule and tag changes. The
debug macOS bundle passed strict/deep ad-hoc signature verification and
`--verify-bundle`; it is not notarized or a release artifact. Vite reported its
existing large-chunk warning. After the user closed the app, the built bundle was copied over the existing
Desktop GitNova.app. The installed copy passed strict/deep signature and
`--verify-bundle` checks, and its executable SHA-256 matched the built bundle.
No alternate app copy was created or normal application window launched.

Final `git diff --check` passed. A local Markdown target check found all 152
relative links in the eight changed/new documents; external links were not
rechecked. A targeted added-line scan found no competitor/company-domain or
common token-prefix matches; it is not a full secret or license audit. No commit
or push was performed.

## Follow-up: existing tag names during Create tag

The user requested tag-name reference in the existing Create tag form. The form
now reuses repository graph refs, shows five local/fetched tags per page, supports
case-insensitive name search and flags exact duplicate names. Numeric descending
name sorting places v10 before v2; it does not infer semantic-version precedence
or release dates, suggest the next release, or fetch remote-only tags. Tag
creation/push still uses the existing native preview and explicit confirmation.

TypeScript and all 53 frontend tests passed after the addition. Browser coverage
extends the existing real-Git create/push test with numeric ordering, pagination,
search reset, empty search results and duplicate-name refusal. Both the extended
create/push test and the existing keyboard-focus regression passed (2 tests,
16.7 seconds). The create/push fixture also verifies that other tags, HEAD,
working files and parent staging are preserved. No new network request is made
to populate the reference list.
