# Branch picker viewport correction — 2026-11-13

Release validation of source `12a826c` in run 43306812721 found the branch menu extending 76px beyond the left viewport edge on Linux. The added Undo/Redo controls left less space before the right-aligned popup in the narrow repository toolbar.

Keep this existing popup left-aligned to its trigger inside `.repo-actions` at the narrow breakpoint. No panel, action, Git behavior or confirmation is added or changed. The existing viewport regression retains all edge/height assertions and adds 850px alongside 1360, 1000 and 780px.

Verified locally on macOS with Chrome and a real Rust core bridge: the existing differently named upstream Fetch/Pull test passed at all four viewport sizes, including branch popup bounds, counters, reviewed Pull and fixture state preservation before confirmation. Only disposable test repositories were written. The local browser transport is emulated and does not certify Linux or Windows native execution. Full hosted release validation is required for the new source.

Patch metadata is synchronized to 1.0.11 using set-version.mjs and target-specific macOS attribution inventory regenerated. The inventory remains incomplete license-review evidence; it is not compliance approval. Previously published release assets remain unchanged.
