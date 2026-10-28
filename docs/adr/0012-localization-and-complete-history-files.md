# ADR 0012 — English/Thai UI and complete historical file lists

Status: accepted, 2026-10-26.

## Blocked-operation clarity

An uncommitted working-tree change blocks the current reviewed merge/cherry-pick/revert workflow. This is intentional protection; GitNova does not stash or discard it implicitly. However, the previous UI displayed a predicted “Merge commit” outcome even when dirty-state validation had prevented prediction. The blocked review now omits predicted outcome/file details. When uncommitted work is the blocker, its existing Back control becomes Review working changes and opens the existing working inspector/diff. No new panel or automatic Git write is introduced. Other blockers retain Back and their explanation.

## File metadata is independent of patch text

Previously, the historical inspector and file selector counted files parsed from a 512 KiB patch. That made the displayed file count, line totals and selectable files incomplete for large commits. The truncation notice disclosed this, but the result was still poor investigation UX.

CommitDetail now includes a complete bounded `files` manifest. Git `diff-tree --numstat -z --no-renames` provides raw relative paths and optional additions/deletions; binary statistics are null and labelled Binary. The manifest uses the same first-parent comparison as the patch, with an empty-tree comparison for roots. Native path identity is preserved independently of display names. Renames remain separate old/new paths, consistent with existing no-renames presentation.

Metadata has a 4 MiB output limit and 20,000-file limit. Exceeding either produces an explicit read error instead of a partial list presented as complete. Inspector totals use this manifest even when the aggregate patch is truncated. When aggregate patch text is truncated, selecting a manifest entry calls the read-only `historical_patch` command with registered membership ID, full immutable OID and encoded repository-relative path. The result stays in the existing dock. The command validates normal relative path components, uses literal pathspecs and compares against the actual first parent, never the current working file. Paths with spaces/quotes remain exact. Root, merge, binary and late-file cases are covered by real Git fixtures.

The per-file patch retains the 512 KiB bound. If a single file exceeds that budget, the dock says code is incomplete; the complete manifest and its line counts remain available. An error or in-flight file request cannot show another selected file's code. Existing working/staged commit reviews retain their stricter complete-preview approval rules. No limits on mutating operations were relaxed.

## Localization boundary

English is the initial fallback. The existing workspace picker contains a labelled Language selector with English and ไทย. This is an application preference, not a repository membership property. `gitnova.locale` in localStorage persists en/th independently of Git state; valid storage events update other windows. Changing language re-renders the current UI without remounting sessions, rewriting commit drafts or replaying an operation. The selector is disabled during a Git write.

`src/i18n.ts` supplies the locale store, React subscription, source-message lookup and numbered interpolation. `locales/en.json` and `locales/th.json` are explicit catalogs. English source messages act as keys. Missing technical diagnostics fall back to their original text. Values are interpolated once and remain React text, so ref/path text containing placeholder syntax or HTML is not recursively interpreted. Accessible labels, placeholders, scope choices, graph actions, branch/remote controls, working/staged flows, search, merge resolution and diff presentation use the same lookup. Dates and relative-time messages follow the chosen locale; Thai calendar formatting follows Intl's th-TH behavior. document.lang tracks the selected language.

Repository/workspace names, branch/tag names, paths, commit messages/authors, typed drafts, code and raw Git metadata are data and are never translated. Known GitNova safety explanations have Thai entries applied at display time. Git subprocess output, unknown native diagnostics and some operation receipts remain verbatim technical evidence, with English fallback; localization does not alter native DTOs, operation kinds or snapshot tokens. This increment adds no translation network service, telemetry, external dependency or new screen.

Catalog checks require matching English/Thai keys, nonempty messages, matching interpolation placeholders and entries for static translation calls in every TSX source. Runtime tests check fallback, invalid/blocked stored preferences and nonrecursive interpolation. Browser checks exercise the real core, locale persistence, exact repository content and preservation of an unsent commit draft.

## Surface and performance

See [surface-budget.md](../ux/surface-budget.md). Locale belongs in the existing workspace preference context; file investigation and blocked-operation recovery remain in the graph/inspector/diff. Historical manifests are currently rendered as the existing scrollable file selector; especially large lists remain a performance-hardening opportunity within the explicit metadata limits. No promise of instantaneous cold reads or full native/cross-platform automation is made.
