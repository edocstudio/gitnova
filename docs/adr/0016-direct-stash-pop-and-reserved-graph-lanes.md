# ADR 0016 — Direct stash/pop on the current branch

The supplied screenshots show excessive action/mode selection and a stash glyph sharing another real commit lane. Replace this interaction rather than add a new navigation surface.

## Existing graph context

Place Stash and Pop beside the existing Working changes row. This is already the current worktree/branch context; no new screen, sidebar or global dashboard is necessary. Stash opens one compact scope review, then confirmation. Pop targets the latest saved entry on the currently checked-out branch, regardless of the branch where it was saved. Choosing an older graph stash opens that exact entry's Pop review directly. Native preview/token revalidation and explicit confirmation remain required by repository rules.

Make the current destination branch prominent. Keep Apply-and-retain and recoverable Drop as secondary advanced actions. Move saved-index comparison, restore-index selection and technical recovery metadata under optional details. Default Pop applies working content without --index; saved staging can be restored explicitly. Stash defaults to including new, nonignored files, with exact reviewed file scope and an explicit option. Success returns to Working changes showing the result. Blocked operations lead to that existing inspector; do not silently reset/stash/abort anything.

Stash records remain repository-wide and immutable. Display the human message without GitNova's generated UUID suffix, preserving the original full record in details/tooltips and native Git. Never rewrite existing records to match the current branch.

## Graph lanes

Allocate stash nodes strictly beyond every real Git node, incoming stroke and outgoing lane. Multiple entries sharing a base receive separate reserved lanes, with explicit continuations to their actual base. Never choose a lane as base.lane + 1: that lane may already belong to another branch. Unloaded bases have no invented edge. Preserve virtual row identity, scroll and keyboard behavior.

Verify real main → stash → develop → Pop with unchanged branch tips, restored content, retained stash on conflicts and no mock Git integration. Verify graph geometry with occupied neighboring lanes and multiple stashes, then inspect actual browser screenshots.
