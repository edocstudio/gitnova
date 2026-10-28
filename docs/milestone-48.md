# M48 — Cherry-pick with an unrelated clean submodule checkout

A merge commit selected on `main` was blocked as generic uncommitted work when `_dags` had clean files but its checkout differed from the gitlink recorded by `main`. Selecting a parent is still required for a merge cherry-pick: Git applies the selected parent-to-merge delta, rather than the entire source branch.

The existing graph action review now shows this baseline explicitly and clears the previous preview immediately when the parent changes. No new screen, navigation or action is needed: the current review already contains the source, destination, parent selector and affected file list. An evidence block in that same review identifies preserved child checkouts and the unchanged recorded pointers. New text has English and Thai translations.

The backend narrowly permits cherry-pick/revert when all parent status entries are unstaged gitlink checkout differences, parent HEAD/index gitlinks agree, and every child is clean with an inspectable identity, HEAD, branch and index. Ordinary dirty files, staged gitlinks, dirty/untracked child files, active child operations and selected deltas affecting gitlinks/layout remain blocked. The predicted result must not overlap a preserved child or change `.gitmodules`.

The preview token and owned-operation marker bind this evidence. Start revalidates it; action reviews and recovery capture revalidate it again. Recovery excludes only verified preserved child directories. Nonrecursive native commands never stage, update, snapshot or restore these children. Start, finish and abort verify the child evidence afterwards; unexpected changes retain the recovery/owned state for inspection rather than attempting a rollback. Older operation markers default to no preserved children.

Verification uses only temporary repositories. The fixture reproduces different main/release gitlinks and a SQL-only merge delta; tests cover parent choice, finish, abort, stale child state, ordinary/child dirty refusal, staged pointers, actual incoming gitlink/layout changes and inverse revert. Existing merge, patch and submodule regression suites exercise shared lifecycle/recovery behavior.

## Verified results

- 46 serial real-repository integration tests passed: preservation (7), existing patch operations (13), merges/recovery (22), merge-submodule previews (4).
- TypeScript and all 32 frontend unit tests passed. Strict Clippy passed for Git/core/desktop, all targets.
- Three browser flows backed by the real Rust core passed with one worker: conflicted cherry-pick plus inverse revert; merge-parent revert plus abort; SQL-only merge cherry-pick carrying an unrelated clean child checkout. The last also checks immediate Start disablement after clearing a parent and compares child HEAD, refs and index bytes after finish. Browser transport is emulated; these checks do not certify native WebView interaction or a user's live repository.
- Visual inspection at 1360×900 confirmed the baseline, preserved checkout evidence, affected file and Start action fit the existing graph review.

- Local debug macOS bundle built successfully in 1m 13s: `target/debug/bundle/macos/GitNova.app` (79.02 MiB). Local signing and embedded UI assets verified; no GUI or developer workspace opened.
