# Rebase, hunk staging and related metadata verification

Date: 2026-10-28. Platform: local macOS / Apple Silicon. All Git writes use disposable fixtures. Compilation and test runners are serialized, Cargo uses one job, native tests use one thread and browser tests use one worker.

## Results

- Hunk integration: 5 passed, including exact index-only Stage/Unstage, unrelated staging, stale file/index/config/native locks, quoted/tab paths, missing final newline, unsupported mode/binary/new/deleted shapes and active filter refusal without executing the filter.
- Rebase integration: 11 passed across the completed 10-case suite and the additional focused submodule-range regression. Covers author/date preservation, unchanged target, recovery refs, valid paused restart, two separate conflict/Continue stops, explicit empty Skip, edited-result Abort backup, original empty commits, binary/rename/delete/raw paths, topology refusal and tampered/interrupted journal refusal. The added regression proves a first predicted conflict cannot hide later gitlink changes, even when their net tree change is zero.
- Related maintenance: 6 passed. Lock/Unlock preserve dirty work and refs; native Repair restores a genuinely broken gitfile; a locator redirected to another repository is refused; stale state is refused; selected submodule URL sync preserves dirty files, parent index and child HEAD.
- Remaining core integration suites (remote_delete, remotes, search, stashes, submodule_updates, tags, tracking, worktree_creation, worktree_removal and writes): 99 passed. Earlier workspace suites through ref_and_push passed, including merge, amend, clone, branch switching, discard and cherry-pick/revert regression.
- Desktop/native-command, Git adapter and graph unit suites passed. Hosting, IPC, search, storage, submodule, workspace and worktree suites and their doctests passed in the follow-up run.
- Frontend TypeScript check and 43 unit tests passed.
- Browser: 4 real-core flows passed (hunk Stage/Unstage in split/unified views, Rebase conflict/Continue onto a merge-commit target, worktree Lock/Unlock, selected submodule URL sync). Rebase and URL sync passed again after their final visibility improvements. Screenshots were inspected locally; no fixture screenshots with local paths were added to documentation.
- Strict workspace Clippy with all targets and warnings denied passed. Formatting check and tracked-diff whitespace check passed.
- Canonical local GitNova.app rebuilt successfully (83.57 MiB), ad-hoc signature verified and embedded UI assets verified. No alternate Updated bundle was created; no GUI or user repository was opened.

## Failures found and corrected

The initial workspace run stopped at the new real worktree-repair fixture. Passing a broken locator as an explicit native repair path triggers reverse discovery and fails; native repair of registered checkout metadata fixes it. The implementation now reviews/binds all affected checkout locators, refuses missing/symlink/foreign destinations and uses that native repair mode. All six maintenance cases then passed.

A new Rebase regression initially expected a thrown error where the core deliberately returns a failed durable receipt. Its assertion now verifies failure plus unchanged HEAD and absence of native rebase metadata. The full-range pointer preflight passed.

An existing Jenkins mock-server test exposed inherited nonblocking accepted sockets on macOS, racing the first request bytes. The fixture now explicitly restores blocking mode with its existing bounded read timeout. All 26 hosting tests passed afterward; product transport behavior was not changed.

The initial full-workspace command therefore did not complete as one uninterrupted green run. The failed suites and all remaining suites were verified separately after fixes; this report does not represent that initial command as passing. Broader crash fault injection, long-session resource measurements and unsupported repository shapes remain separate acceptance work.
