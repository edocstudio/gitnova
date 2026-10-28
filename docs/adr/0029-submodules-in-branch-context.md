# Submodules in the active branch graph

The user’s 2026-10-27 reference requires submodules to remain discoverable even
when clean. Working changes only lists changed paths, and the workspace picker
hides these relations behind repository switching. Neither supplies that context.

Use a bounded, collapsible Submodules group beside the existing Working changes
row, above the graph. This reuses the graph surface rather than adding a sidebar
or dashboard. Name the active parent branch. Show all discovered relations,
including clean, missing and nested modules. Open a child from its name; Manage
or right click expands the existing reviewed submodule controls in place.

Distinguish the parent HEAD gitlink, parent index and child checkout. Restore
continues to use the explicitly reviewed parent index revision. Changing parent
branches does not automatically update any child checkout. Never manufacture
ahead/behind counts from dirty status or SHA differences.

Read only the active repository. Bracket discovery with native parent status and bounded index fingerprints,
reject results when parent branch/HEAD changed, and invalidate displayed rows
immediately when graph context changes. Refresh after writes, on focus and every
15 seconds while expanded and visible. Serialize discovery; no polling during
writes or an open management review, and no background discovery when collapsed.
These display reads do not authorize writes: existing native previews revalidate
parent index, checkout and child state before every confirmation.
