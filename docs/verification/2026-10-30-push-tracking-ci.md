# Push tracking reconciliation — 2026-10-30

## Evidence and cause

The public Actions run 40929828592 at caee44f failed the Ubuntu
`initial_publication` test with `reference already exists` while updating
`refs/remotes/origin/main`. Its desktop job failed five Push/remote-delete
browser cases waiting for the action review to close. Both runners used Git
2.56.0. The local machine uses Apple Git 2.39.2; the original first-publication
browser case passed locally before this fix.

A successful native push can update a matching remote-tracking ref itself,
including when invoked with the destination URL. GitNova subsequently tried to
create or advance that ref using the pre-push expected value. An already-applied
cache update therefore caused an unknown receipt after remote publication;
the UI correctly retained the unsuccessful review. Remote deletion had the
corresponding already-absent cache case.

The default-branch-name hints in fixture setup are informational. They did not
cause these failures. Cancelled matrix jobs are not evidence of platform passes.

## Change

Push and remote deletion share reconciliation after live remote verification:

- Accept only the reviewed cache value or the intended completed value.
- Reject an unrelated concurrent value or a symbolic tracking ref.
- Atomically compare/update/delete/verify through an update-ref transaction.
- Verify absence when native deletion already removed the ref.
- Preserve existing lock files, local branches, drafts, staging and recovery.
- Keep distinct fetch/push URL and mapping rules unchanged. No automatic retry,
  force push, timeout increase, GitHub setting change or warning suppression.

Tests emulate native cache updates with explicit temporary-repository hooks on
older Git, covering first publication, advancing an existing branch and remote
deletion. The low-level regression covers unchanged/already-updated/already-absent
refs, a third-party value, symbolic refs and another writer's lock.

## Local verification

All Git writes use disposable repositories. Hosted Git 2.56.0 has not been rerun
by this task; the remote CI result must be checked after the owner publishes.

- Tracking reconciliation regression: 1 passed.
- Core `initial_publication`, `ref_and_push`, `remote_delete`: 38 passed, serially.
- English/Thai i18n checks: 3 passed.
- Five previously failing browser cases: 5 passed in 57.9 seconds, one worker.
- Formatting, Clippy (`gitnova-git`/`gitnova-core`, all targets, warnings denied) and whitespace checks passed.

No commit, push, release or native application bundle replacement was performed.
Existing lifecycle, investigation and benchmark work remains in the working tree.
