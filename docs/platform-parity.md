# Current-feature platform parity

Scope: every feature currently available in the macOS alpha must also work on Windows/Linux. Installer generation alone does not satisfy this scope. New features beyond the current macOS baseline are separate work.

The frontend and command/DTO surface are shared. Platform-specific implementations must preserve preview/confirmation, immutable scope, dirty-work protection, partial-failure reporting and recovery. No Windows/Linux replacement UI, reduced feature mode or mock Git backend is accepted.

## Acceptance map

Each row requires the same functional suite on all three native runners and an installed-app acceptance pass. The listed tests are real Git fixtures, not user repositories. Browser graph tests use the real Rust bridge but do not certify native webview rendering/transport by themselves.

| Current workflow | Shared automated coverage / native acceptance |
| --- | --- |
| Workspace switching, pinned/recent repositories, repository search/pagination, bare repositories and linked checkout identity | `real_git`, `read_optimization`, `related_maintenance`, full graph suite; native app-data and Unicode/space-path checks |
| Init, Clone, first publication and remote setup | `init`, `clone`, `initial_publication`, graph suite; native Git installation/HTTPS/SSH |
| Graph lanes, refs, selection, paging, column resize and cached context | graph/domain crates, graph suite; native scrolling, keyboard and long sessions |
| Working changes, stage/unstage all/files/hunks, discard, commit and amend | `writes`, `hunks`, `discard`, `operation_discard`, `amend`, graph suite; native filesystem permissions and line endings |
| Branch creation/switch/delete, tracking/upstream, remote branch opening, fetch/auto-fetch, Pull and Push | `branches`, `remotes`, `remote_delete`, `ref_and_push`, `tracking`, transport credentials and graph suite |
| Merge, cherry-pick, revert, rebase, edited replay, root/merge topology and branch cleanup | `merges`, `patch_operations`, `rebase`, `merge_branch_cleanup`, graph suite; preserve authors/refs/dirty work and native lock ownership |
| Conflict Current/Incoming/Result editing, per-block choices, staging, continue/abort/restart | merge/rebase/patch suites and graph suite; native input, clipboard and result persistence |
| Stash create/inspect/apply/pop/drop, saved index/untracked content and large bounded diffs | `stashes`, graph suite; cross-branch application and retained stash on conflicts |
| Recursive submodule discovery, recorded/current pointers, initialize/update/restore, lifecycle and URL sync | `submodule_updates`, `submodule_lifecycle`, `merge_submodule_preview`, `patch_submodule_preservation`, graph suite |
| Worktree create/open/move/reattach/repair/unlock/prune/remove, memberships and shared object store | `worktree_creation`, `worktree_removal`, `related_maintenance`, graph suite; native directory identity/reparse-point protection |
| Tags, selected tag publication and ref management | `tags`, `graph_actions`, graph suite |
| Historical files, search, investigation and current cross-repository release readiness | `historical_files`, `search`, `investigation`, graph suite; lazy loading, bounded caches and exact repository attribution |
| Global provider/Jenkins connections, per-connection author identities, CI, PR/MR and commit links | hosting contracts, `ci_connections`, `pipelines`, `commit_identities`, graph suite; native masked entry, OS store isolation/lock/cancel and browser opening |
| Embedded terminal, cwd/session scope, output bounds and session cleanup | native PTY tests on every OS plus graph UI tests; PowerShell/ConPTY on Windows and POSIX PTY elsewhere |
| General Settings, themes, EN/TH, FAQ and accessible keyboard interactions | frontend unit tests and full graph suite; native fonts, shortcuts, dialogs and focus |
| Operation journals, stale-lock recovery and interruption safety | `activity`, `stale_lock`, merge/rebase/stash/discard suites; native process and file identity, never remove live/native Git locks |
| Timeout/process lifetime, memory and long paths | Git runner tests, Windows Job Object descendant cleanup test, long-session profiling; no orphaned helpers after timed-out Git operations |

## Platform differences implemented in this change

- Windows stale-lock recovery uses OS process handles, snapshot process names, volume/file IDs, an exclusive writer/deleter-denying handle and handle-based rename. Shared checks keep the same native Git markers, ref-lock bounds, worktree checks and confirmation token semantics as Unix. It never removes the lock just because its PID is absent from a text listing.
- Interactive replay no longer has a Unix-only rejection. The reviewed todo remains an immutable private file. Git for Windows' shell applies it with safely quoted normalized paths; Unix retains its system shell. Edited-replay tests and stale-lock UI tests run on Windows too.
- Init/Clone/worktree/discard confirmation tokens on Windows use native directory file IDs instead of creation timestamps. New/replaced directories invalidate review even at the same path.
- Git for Windows receives process-local `core.longpaths=true`, never a global/repository configuration edit. Git read/write children run in a Job Object with hidden console windows; timeout cleanup terminates descendants, with a dedicated native test.
- Keychain, Credential Manager and Secret Service provide the same host-scoped account capability. Linux background fetch reads only an already-unlocked exact item and never prompts. Foreground account access may request an unlock, as on macOS.

## Completion gate

**Not yet certified as full parity.** Implementation has been enabled, and regression/native CI coverage has been expanded; the first native CI run exposed failures and the corrections still need a passing Windows/Linux rerun and installed-app checks. See the [native CI findings](verification/2026-10-31/native-ci-corrections.md). The maintainer's Mac cannot execute those native branches. Record the exact commit, OS/version, passed tests and any remaining defects before changing this status.

Windows filesystem restrictions (reserved names, unsupported characters, symlink privileges/Developer Mode and filesystem capabilities) remain real OS constraints. Handle them explicitly without selecting another path or silently changing scope. These constraints must not be used to hide a missing app workflow. Git for Windows must include its shell/tools; an arbitrary minimal Git executable is not enough for hooks or interactive replay.

No claim of completed resource/leak testing, signed releases or third-party license compliance follows from this acceptance map.
