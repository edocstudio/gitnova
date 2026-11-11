# Repository management dialog and Terminal prerequisite

Follow-up to the 1.0.5 workspace membership controls. The disabled removal in the
reported screenshot was caused by an active (possibly hidden) Terminal entry.
Management now opens a native modal dialog from the same row actions/right-click
entry point. It reuses the workspace context rather than adding navigation.
The repository name/path remain visible independently of editable values.

Remove opens its confirmation even with blank name/path inputs or a prerequisite
still pending. The final removal remains blocked by unsaved drafts or an active
Terminal. A button in the dialog exposes the existing Terminal end action and
requires explicit confirmation that shell commands will stop. Cancellation keeps
the shell running. Confirming removes only that Terminal entry and hides its dock
if selected, preventing automatic creation of a replacement session when the
workspace falls back to another repository. Shell unmount uses the existing
native close/session-ownership cleanup. No new process termination API is added.

The confirmation identifies the original membership, not an unsaved display-name
edit. Removal still changes workspace catalog metadata only. Escape closes the
dialog without closing the picker, and focus returns to the triggering action
(or picker if the entry no longer exists). EN/TH copy is included.

Validation on macOS:

- TypeScript check passed.
- Frontend unit suite: 21 files, 79 tests passed.
- Targeted browser suite: 7 passed, including search/pagination/language,
  Terminal opening/repository switching, rename/reconnect/removal persistence,
  draft protection, and hidden Terminal confirmation followed by removal despite
  blank edit fields. Initial name/path values were present in the test fixtures;
  the screenshot alone does not establish why those inputs had been cleared.
- Browser calls use the real Rust catalog bridge and disposable Git repositories;
  PTY transport is emulated. The test checks close is invoked once only after
  confirmation, and verifies HEAD/files remain unchanged after removal.
- Native-modal screenshot inspected. React review covered focus, Escape,
  dialog/effect cleanup, busy state, explicit session confirmation and bounded
  state; no new dependency added.
- Final focused browser rerun: 1 passed, including focus returning to the picker
  after removal of the triggering entry. Destructive-action styling was inspected
  in the rendered modal.

No new Rust behavior was introduced by this follow-up. Native GUI interaction on
Windows/Linux and the full end-to-end suite were not run.

Delivery: version metadata synchronized to 1.0.6 with the version script; its
regression test passed. Attribution inventory regenerated. `build:local` passed,
and Desktop/GitNova.app was replaced in place after the app quit. Installed
Info.plist reports 1.0.6; the executable matches the built bundle. Strict deep
code-signature verification and `--verify-bundle` passed. This is the local debug,
ad-hoc signed bundle; no notarization, publication or release was performed.
