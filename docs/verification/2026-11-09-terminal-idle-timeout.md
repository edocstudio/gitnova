# Terminal idle timeout

The device preference lives in Settings → General, defaults to disabled, and
allows 30 or 60 minutes. It is persisted in local settings without storing shell
commands/output. It applies to all repository Terminal sessions.

Native expiry checks require session ownership, a supported timeout, a verified
zsh prompt with no background jobs, and elapsed monotonic inactivity. zsh preexec,
precmd and line-editor hooks report busy/idle using session-specific bounded OSC
markers. Input invalidates idle state; output resets elapsed inactivity. Unknown
shells never automatically expire. No process-idle inference is made from an
absence of output. User shell startup files are not edited.

The frontend checks about every 15 seconds, including hidden docks, without
overlapping checks. Disposal clears timers; late responses cannot update an
unmounted view. Verified expiry removes the session through existing cleanup and
announces a status message. OS sleep/browser suspension may delay checking.

Validation on macOS:

- Native Terminal suite: 7 passed. Covers split markers and bounded state,
  unknown/off states, input activity, real silent foreground command, background
  job, unfinished line-editor input, and verified expiry. Elapsed time is
  advanced in the fixture; no claim of a 30/60-minute wall-clock soak is made.
- Terminal preference unit suite: 2 passed, including invalid choices and
  persistence. The storage fixture is synthetic.
- TypeScript check passed.
- Native Clippy check passed with warnings denied.
- Browser suite: 3 passed. Covers existing Terminal behavior, preference
  persistence and hidden-session expiry, and repository switching/session reuse.
  Browser PTY transport is emulated; native shell tests cover actual zsh behavior.
- React review: subscription cleanup, timeout cleanup, bounded session state,
  fresh callback refs, labelled settings controls, EN/TH strings and status
  announcement reviewed. No new navigation surface or runtime dependency added.

The feature currently verifies idle only for zsh. Other shells retain sessions;
this is explicit in Settings and the CLI guide. Windows/Linux GUI behavior and
long-duration wall-clock inactivity were not verified in this run.

Delivery: version metadata synchronized to 1.0.4; version-script test passed.
Local macOS debug/ad-hoc bundle built and passed embedded-asset verification.
Installed over Desktop/GitNova.app; version, binary hash, code signature and
installed `--verify-bundle` check passed. This is not a notarized release build.
Attribution inventory was regenerated: 413 components, 250 license texts and
67 unresolved entries; the license audit remains incomplete.
