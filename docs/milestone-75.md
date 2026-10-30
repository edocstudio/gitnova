# M75 — Repository terminal dock

Status: implemented; local macOS verification is recorded separately.

The Terminal action beside Pull/Push opens an interactive PTY in the selected
membership's canonical root, including linked worktrees and bare repository
folders. This is an on-demand graph dock, not a navigation destination.

- Desktop-only `portable-pty` adapter; normal user shell on Unix, PowerShell on
  Windows. Platform implementation availability is not certification: this change
  was exercised locally on macOS. No arbitrary path is accepted over terminal IPC:
  initial roots resolve through core workspace membership.
- `@xterm/xterm` and its fit addon load on demand. Shell input is forwarded as user
  input, never constructed from commit/ref names or paths. Paths use process cwd.
- Hide and switching retain separate sessions; End session confirms shell closure.
  Window destruction/reload closes registered sessions. Detached programs may
  outlive a shell. Native input/output is bound to membership plus session ID.
- At most four sessions; 256 KiB native output retention per session, 64 KiB read
  batches, 2,000 renderer scrollback lines, bounded paste queue and native input
  worker admission. Polling/render writes use backpressure. Hidden sessions stop
  UI output polling and retain bounded native output with an explicit trim note.
- Auto fetch pauses while terminal sessions exist. Local status checks and manual
  Refresh still inspect external changes. GitNova cannot infer completion or lock
  ownership for arbitrary user shell commands; Git's own locks remain relevant.
- Commands use native shell/Git configuration, not GitNova's reviewed-write or
  connection author overrides. App-internal `GIT_*` routing is removed. Shell
  startup files and shell history remain user-owned behavior; GitNova does not
  persist terminal transcripts or inject saved provider credentials.

No terminal settings page, standalone dashboard, plugin shell access or network
transport was added. Existing non-terminal Git operations retain their previews.
Upstream MIT texts for the newly added terminal components are bundled as a
resource; this is not a full transitive license audit.
