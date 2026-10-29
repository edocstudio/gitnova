# Milestone 69 — Resizable graph columns

The existing history header exposes accessible vertical separators for all six columns. Pointer capture supports drag; keyboard arrows and Shift adjust widths; Home or double-click restores the per-column default. All visible commit/stash rows share the header track sizes. Horizontal scrolling synchronizes in both directions, including header focus scrolling. The header width accounts for the actual history viewport width.

Widths are bounded device-local values in `gitnova.history-column-widths.v1`; only six known numeric fields are accepted. Persistence occurs at drag completion and keyboard changes, with session fallback when storage is unavailable. Updates are animation-frame coalesced. No native API/dependency or Git operation is introduced.

Graph spacing adapts to width without scaling node circles or cutting out lanes; the minimum grows with loaded lane count. Automatic graph width is capped where legibility permits, reducing empty space beside visible branches. Commit message defaults to filling remaining space; a manually sized message stays fixed, with surplus space after Date. Table overflow preserves access to every column on small windows. Existing selections, virtualized row positions and stash actions retain their behavior.

See the dated verification report for tests and local packaging status.
