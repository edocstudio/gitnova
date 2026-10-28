# M36 — Continue a diverged Pull without losing local history

The reported local-1/remote-4 state correctly refuses a fast-forward. The defect
was a dead end and a zero-path display despite integration not being computed.

Pull now shows actual local/incoming commit counts and the fetched remote OID,
then offers Review merge in its existing surface. That action only predicts the
merge. Start uses the existing explicit merge workflow; staged/conflict review
and Finish remain in the existing inspector/diff dock. Both histories are kept.
Back returns to the Pull review without fetching again.

The remote preview DTO now distinguishes the fetched remote OID from its
fast-forward target, which is the local OID when ahead. The handoff binds the
source ref/OID and reviewed destination branch/OID. A changed destination refuses
the preview; native merge tokens also revalidate source, branch, index and config
on confirmation. Dirty work leads to existing Working changes without auto-stash.
Native Pull remains fast-forward-only, with no implicit merge/rebase/reset.

Blocked Pull, graph checkout and branch-picker previews no longer claim a
computed zero-path result. New labels and explanations have EN/TH catalog entries.
See ADR 0030 for the existing-surface rationale.

Validation, serialized on temporary real repositories:

- 2 native core Pull tests: differently named upstream, dirty/stale refusal,
  divergence protection and a remote OID distinct from the retained local target.
- 3 browser scenarios: existing Fetch/fast-forward Pull; local-1/remote-4 → review
  → Back without refetch → Start → Finish with both exact parents and files;
  changed branch, dirty work and moved remote ref all refuse unsafe integration.
- 26 frontend tests including complete EN/TH keys; TypeScript and Rust formatting.

[Updated Pull review](screenshots/diverged-pull-review.png).

Local artifact `target/debug/bundle/macos/GitNova Updated.app` rebuilt successfully
with TypeScript checking, ad-hoc signature and embedded asset verification. Build
log: `/tmp/gitnova-diverged-pull-build.log`. Quit the previous instance before
opening this bundle. No user checkout was merged, fetched or modified by tests.
