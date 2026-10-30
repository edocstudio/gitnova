# M71 — Submodule URL and hunk correctness

The existing selected-submodule Sync URLs review now resolves supported relative
URLs against the parent branch's configured default remote (origin when no
branch remote is configured). It preserves explicit selected-child scope,
configuration recovery, dirty working files and recorded/checked-out commits.
Standard HTTPS, SSH-with-port and SCP-style sibling/ancestor URL shapes are
compared with native Git in fixtures. Root escape, ambiguous/custom URL shapes
and unsafe sources are refused. No fetch, hook, custom update command or checkout
runs during preview. Native Git remains the confirmed writer.

Post-write checks compare the raw reviewed destination, not its redacted display
form. Previously a normal SSH username could make a successful sync appear
partial. Parent remote/configuration changes invalidate the review before sync.

Hunk parsing now counts actual diff header lines, rather than occurrences of the
text `diff --git ` anywhere in a file. A literal command example inside source
code therefore no longer prevents Stage/Unstage hunk. Existing complete-patch,
index-lock, filter, mode and stale-token protections remain unchanged.

Rebase and worktree maintenance retain their existing supported scope. Regression
coverage includes Rebase author/recovery/conflict/skip behavior and linked
checkout isolation, plus worktree Lock/Unlock/Repair and dirty-file protection.
This milestone does not implement arbitrary merge-preserving Rebase, submodule
add/remove/update-to-newer, or moved-worktree discovery/pruning. Earlier M56's
blanket relative-URL limitation is superseded only for the validated shapes above.

See [verification](verification/2026-10-30-lifecycle-investigation.md).
