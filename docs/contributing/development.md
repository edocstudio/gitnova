# Development

Keep core crates independent of Tauri. UI inputs may select an operation, not a shell string. Add capability ports only when a behavior is being implemented. No speculative empty release crates.

Before adding a surface, update docs/ux/surface-budget.md with evidence that the existing graph context cannot naturally deliver the capability. Prefer one contextual action to another page.

Git fixtures belong in temporary directories. Never use the real checkout for destructive test setup. Keep reads bounded, preserve native paths, distinguish repository/worktree IDs, and test external Git changes. Do not silently retry partially completed writes with another adapter.

Run the relevant checks in [CONTRIBUTING.md](../../CONTRIBUTING.md). CI defines core fixture jobs on Linux, macOS and Windows, and desktop/browser jobs on macOS. Local execution is not proof that hosted CI passed. Do not claim benchmark targets or cross-platform support without measurements.

Use English for code, identifiers and repository documentation; communicate user-facing progress in Thai. Keep changes scoped to a milestone. Git writes need operation previews, revalidation, receipts and recovery design before they are enabled. Smart change preview remains read-only in its first milestone.

Product text uses `t(EnglishSourceMessage, values)` and matching en/th catalog entries. Use numbered placeholders for inserted data; keep whole phrases together when word order differs. Dates use the active date locale. Git names, messages, paths and code never pass through translation. Unknown Git diagnostics remain original technical evidence. Verify a language switch without losing drafts/selection, and keep the catalog/placeholder checks passing.
