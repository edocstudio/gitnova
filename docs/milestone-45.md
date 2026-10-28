# M45 — Central settings

User explicitly requested a Settings icon in the upper-right header and removal of GitLab/Jenkins connection sections from commit details. Persistent preferences describe the application or repository connection, rather than a historical commit, so they cannot naturally live on graph nodes. One global Settings dialog is justified; it reuses the existing CI forms, secure credential flow and status inspection instead of creating a dashboard or duplicate navigation. The graph remains visible underneath and keeps its CI column.

The header gear (and Cmd/Ctrl+,) opens Settings. Application language moves out of the workspace picker into General. Repository connections clearly name the active repository/path and the selected revision (or current HEAD when Working changes is selected). GitLab/Jenkins configuration and existing build evidence move together out of commit details, preserving the available inspection and reconnect/disconnect flows. No Git write is performed. Empty/unborn repositories retain General and explain why CI cannot yet validate a commit. Closing restores focus; Escape, keyboard traversal and narrow/short windows remain usable. Existing stored preferences and Keychain credentials migrate in place without resetting connections.

Future persistent application, workspace and repository preferences belong here, with their scope explicit. Git action inputs such as merge source/target and push selection remain in their contextual reviewed actions. Per-action remote choice is not global state.

Verification follows focused frontend and browser checks, then one serial local app build. No live account connection or developer repository writes are used in tests.

## Verification

- TypeScript and all 29 frontend unit tests passed, including English/Thai catalog coverage.
- Eight focused browser scenarios passed across serial runs: moved Settings UI, empty-app scope, language persistence and draft retention, Thai branch reviews, GitLab/Jenkins cached/error/late-response/hidden behavior and visible graph CI polling. Settings also preserves an unfinished graph branch-name draft when switching language. Underlying graph/picker keyboard handlers yield to the native dialog.
- Local debug app built at `target/debug/bundle/macos/GitNova.app` (77.57 MiB); signature and embedded assets verified without opening a GUI or developer workspace.
- Provider APIs are fixture-backed in these UI tests; Git scope/state checks use real disposable repositories. Existing local preference keys and native credential APIs are unchanged.
