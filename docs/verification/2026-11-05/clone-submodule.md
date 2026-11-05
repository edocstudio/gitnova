# Connected clone browser, submodule authentication and discovery reads

## Scope and behavior

Clone's existing workspace picker now offers saved GitHub/GitLab Connections. The authenticated repository list is read in explicit pages of 100 and rendered five rows at a time. Search filters loaded results; it does not claim to search every repository on the server. Up to 500 rows per host scope and eight session scopes are cached for 60 seconds. Connection changes clear the cache even while Clone is closed. Selecting a row fills the existing source URL and preserves the destination draft. Normal preview/confirmation still precedes clone.

Provider lists are read-only. GitHub lists the token's owned/collaborator/organization-member repositories; GitLab lists member projects. Access is limited by token permissions. Clone URLs are constructed from the validated selected HTTPS host and exact project path, rather than trusting cross-host URLs in provider responses. URL/local-source entry remains available.

Submodule add and initialize previously did not receive GitNova's HTTPS credential bridge: its transport recognizer only accepted direct clone/fetch/push/ls-remote commands. They now explicitly authorize only the validated child source URL through command-local configuration. Relative initialization URLs reuse the existing native-compatible resolver. The helper's exact URL matching, redirect restrictions and refusal to persist tokens remain in effect. SSH continues to use the configured agent. Reviews do not authenticate or download.

Add submodule now orders and explains the source repository and relative folder separately, shows source → destination, and uses explicit labels/descriptions. Existing checks against nonempty destinations, cached child metadata, dirty work and custom update commands remain. No destructive recovery shortcut was added. The team's original initialization error was not available, so this fixes a verified gap without claiming that every observed failure had this cause.

## Performance evidence and limits

Discovery no longer runs parent status or reads its HEAD tree when the index has no gitlinks, including leaf repositories reached during recursive discovery. The outer bare-repository check uses discovery instead of another full status. Index parsing, branch/index snapshot consistency and write-time validation remain.

`read_optimization::submodule_discovery_without_gitlinks_skips_status_and_head_tree_scans` measured these Git subprocess counts on a disposable repository with dirty tracked work:

| Command | Calls |
| --- | ---: |
| `ls-files` | 1 |
| `rev-parse` | 4 |
| `status` | 0 |
| `ls-tree` | 0 |

The test also verifies that the index bytes remain unchanged. This is a command-count regression check, not a statistically controlled latency or RAM benchmark. No whole-app speedup percentage, large-repository certification, native foreground soak or live private-host network result is claimed. Reading a large index still has a cost; this change does not remove that bound or cache dirty working state across write previews.

## Validation

All Git writes below use disposable repositories. Provider HTTP behavior is tested with loopback synthetic responses, not real accounts/tokens.

- Rust Git library: 11 passed.
- Rust hosting adapter: 41 passed, one existing ignored test.
- Core submodule updates: 22 passed, including relative initialization and nested relations.
- Core related maintenance: 16 passed.
- Core read optimization: three passed, plus a targeted measurement with captured command counts.
- Frontend unit tests: 64 passed.
- Browser checks: URL clone with real Git; connected list with emulated provider responses, network pagination through 107 rows, five-row display, search, cache reuse and source selection; registration add/remove retaining child files; initialize/recorded restore; registration stage/unstage retaining unrelated staged work.
- TypeScript check passed.

Browser transport is emulated around the real Rust core and temporary repositories; it does not certify native credential prompts or company-specific provider permissions. Full hosted Windows/Linux and long-running RAM acceptance were not rerun for this patch.

Final checks: Clippy (`gitnova-hosting`/`gitnova-core`, all targets, warnings denied), frontend TypeScript/Vite build, and macOS `build:local` passed. The ad-hoc signed debug bundle was verified and installed over the existing Desktop app; no notarization or release signing is claimed. Installed executable SHA-256: `64c86ecb108a022cf014a6ffd2bf5690fe9daf6c9662878513f2ce34d0bdad49`. No repository commit, push or release was performed.
