# Nova command availability after zsh startup

The embedded Terminal starts a login shell. Prepending the launcher directory to
the process environment alone did not protect it from shell startup files that
replace PATH. Earlier command availability coverage used sh rather than zsh.

Session-owned zsh startup wrappers now source existing user startup files and
restore the launcher path after .zshrc and .zlogin. The original startup directory
is restored for the interactive session. No user startup files are edited.

Verification on macOS: the native Terminal suite passed all 5 tests. The new
regression uses a real interactive login zsh, a synthetic startup directory with
spaces/quotes, and .zshrc/.zlogin files that replace PATH. It verifies command
resolution, loading user settings, restoration of ZDOTDIR, and unchanged rc bytes.
Existing bounds, session admission/cleanup and PTY lifecycle tests also passed.

Clippy (desktop, all targets, locked, warnings denied) and diff whitespace checks
passed. The 1.0.2 local bundle was built and installed over Desktop/GitNova.app;
signature, matching built/installed executable hashes, embedded assets and the
installed executable's Nova help were verified. No live remote was written.

This change targets zsh startup; it does not claim equivalent regression coverage
for arbitrary shell configurations or changes to PATH later in a running shell.
