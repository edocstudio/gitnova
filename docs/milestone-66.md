# M66 — Large branch-history Push review

A new remote branch has no destination tip, so its reachable history can contain hundreds or thousands of commits even when the feature adds only a few changes to an established project. The previous 500-subject review bound incorrectly became a publication blocker. A branch update is fully scoped by its immutable reviewed source SHA and live destination; the number of subject rows displayed should not determine whether that exact normal update is possible.

Push review now retains the exact full range count and at most the newest 500 commit records. It explicitly says when the displayed list is shortened and that the entire branch at the reviewed SHA will be published. A new branch labels its count as branch history and explains that these are not necessarily new commits/objects being uploaded. It does not infer a trustworthy remote object inventory from cached refs or claim a transfer-size estimate. Existing destinations continue to use their live tip as the outgoing range baseline.

No new screen, control or setting is necessary: the existing Push confirmation shows the count, bounds and source/destination. English/Thai explanations reuse that review. A versioned token binds the existing source/state/config/destination/tracking evidence plus count; old review tokens are invalidated. Incomplete or malformed bounded records still fail. No unbounded subject list is allocated.

Native destination validation, ancestor checks, pinned OID refspec, no-force/no-tags/no-mirror behavior, submodule availability checks, hooks-after-confirmation, tracking/upstream review and partial-failure receipts remain. Apply revalidates the live destination. This change does not force a rejected remote or guarantee server permission/transport success.

README also includes the requested official macOS/Linux rustup installation and current-shell activation commands; Windows uses its installer. No installer is executed by this change.

See [verification](verification/2026-10-29-large-push-review.md).
