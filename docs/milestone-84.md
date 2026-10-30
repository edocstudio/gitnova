# M84 — direct block conflict choices

Selecting an editable text conflict opens the existing diff dock. Current and
Incoming appear side by side for the selected conflict block; the full Result
draft is below them. Tick either side or both. Both concatenates Current before
Incoming; it does not infer a semantic merge. Clearing both restores that block's
unresolved markers. Previous/Next revisit the original blocks while other choices
stay intact. Untouched text and line endings are preserved.

A checkbox starts an in-memory draft directly. Edit result also allows typing a
custom result. Manual changes become a new draft snapshot; old block descriptors
are not applied over that text. Returning to inspection and editing again keeps
a draft only while its original working content and file token still match.
Changing language preserves the draft and active choices.

Review edited result lives beside the choices in the dock. It remains disabled
while recognized conflict markers remain. Review rechecks the native conflict
snapshot; Apply resolution is still a separate confirmation that saves and stages
the selected path. No checkbox saves, stages, continues an operation or commits.
An external edit refuses review and keeps the draft visible.

Whole-file replacements remain in secondary details in the existing inspector,
explicitly labelled as replacing the complete file. Binary, unavailable-text and
submodule cases retain their native guarded workflows rather than becoming text
block selections. Rebase names target-plus-replayed work and the replayed commit;
Revert names its pre-commit incoming version.

The conflict dock has a larger default height; explicit user resizing still wins.
The existing dock separator supports dragging, double-click enlargement,
ArrowUp/ArrowDown resizing and Home/End bounds. Small-height inspectors scroll
instead of allowing the three file groups to overlap resolution controls.

See [verification](verification/2026-10-30-conflict-choices.md) and the
[surface budget](ux/surface-budget.md#m84--direct-conflict-choices-in-the-diff-dock).
