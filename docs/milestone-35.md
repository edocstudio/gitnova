# M35 — Submodules in the active branch graph

User correction: clean submodules must remain visible for the checked-out parent
branch, rather than appearing only as Working changes or hidden related entries.
The graph remains the primary surface; see ADR 0029.

- The collapsible Submodules group sits immediately below Working changes. It
  names the parent branch, shows the discovered count, and lists clean, changed,
  dirty, detached, uninitialized and nested relations. The list has bounded height.
- Click a module name to open its actual repository graph. Manage or right click
  opens the existing reviewed controls in the group. Open, Stage/Unstage pointer,
  Initialize and Restore recorded share their existing native implementations.
- Parent HEAD, parent index and child checkout remain separate. Switching parent
  branches refreshes the display without moving the child. Restore targets the
  explicitly reviewed index version, never an implicit newer remote revision.
- A typed native display snapshot brackets discovery with live branch/HEAD and
  bounded index fingerprints. A mismatch fails explicitly. This read remains
  available during pending Git operations and cannot authorize a mutation.
- Frontend source checks, context-keyed mounting and cancellation prevent late
  results from replacing a different branch or repository. Read one active list,
  single flight; refresh on focus, writes and 15-second visible expanded polling.
  Pause discovery while collapsed, hidden, managing a module or writing.
- Loading/error/empty states and all new controls have English/Thai catalogs.

Existing limitations remain: these controls do not add/remove submodules or update
from a configured remote branch. Dirty child files are managed inside their own
repository. Commit differences are described by actual SHAs/status, without
invented ahead/behind numbers. Large-repository/long-session performance remains
a separate planned measurement, not established by these functional checks.

Verification is serialized with one Cargo job and one Playwright worker, using
only temporary real repositories. Core coverage checks uninitialized discovery,
per-branch recorded revisions, pending-operation reads, index preservation,
removed relations and unchanged child checkout. Browser checks cover clean
visibility, management, parent switching, child navigation, late result rejection,
existing pointer actions and repository pagination. The native IPC check includes
this snapshot and rejects remote origins. Final artifact verification is recorded
in the local build log; no user repository mutation or native GUI launch is used.

Validated results: 26 frontend tests, 1 core snapshot regression, 1 native IPC
flow and 6 distinct browser scenarios passed. The final layout rerun confirms
Stage/Restore are visible and the graph keeps more than 100 px at 1360×900.
An additional run applies Stage then Unstage from the graph group itself and
checks refreshed Pointer staged/Commit changed states plus preserved child HEAD.
Screenshots: [clean branch list](screenshots/branch-submodules-clean.png) and
[management in graph context](screenshots/branch-submodules-manage.png).

Local macOS artifact: `target/debug/bundle/macos/GitNova Updated.app` (63.55 MiB).
Build, ad-hoc signature verification and embedded asset verification passed; see
`/tmp/gitnova-branch-submodules-build.log`. Quit the existing app before opening
this bundle so single-instance handling does not return to an older window.
