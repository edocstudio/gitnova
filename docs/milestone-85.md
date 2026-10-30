# M85 — stash preview size is separate from write scope

The old stash review rejected an operation when aggregate working/index patch
text exceeded 512 KiB. Large tracked/untracked files or saved stashes could not
be saved/restored even though the complete path list and immutable identities
were available. The text display cap no longer rejects the whole operation.

Preview protocol version 2 keeps complete bounded path metadata and explicit
working/index text-truncation flags. The saved path list includes index-only
changes as well as working and untracked changes. Aggregate strings remain at
most 512 KiB each; untracked aggregate text also stops after 16 files. The
inspector explains that all reviewed files remain in scope. It loads each
selected Working/Index patch independently into the existing diff dock, so a
file omitted from an aggregate text prefix is still inspectable. A selected
large file remains bounded and is clearly labelled partial.

The read-only `stash_patch` command validates membership, request/token and path
scope before reading and rechecks the snapshot afterward. Saved content is read
from the exact stash/index/untracked objects. Current tracked Working views
compare HEAD to working content, including staged changes; they are not silently
changed to an index-to-working comparison. Pathspecs are literal, including
historical paths. No configured external diff or textconv is executed.

Apply still recomputes the full review token. Selected working files are hashed
in full under the existing 16 MiB per-file bound, with index/HEAD/configuration,
stash identity/list and removal metadata retained in the token. A changed tail
beyond the displayed prefix invalidates confirmation. Code clipping never
clips what Git saves/restores. Stored objects themselves are immutable identities.

The 20,000-path/metadata bounds, active-operation and hidden-index guards,
collision checks, complete stash-list/removal limits and recovery behavior
remain explicit. Gitlink changes are checked in working, saved-index and
untracked deltas; index-only pointers cannot bypass separate submodule review.
Pop removes the selected record only after successful application; conflict or
failure keeps saved work and recovery. This does not add automatic stash-conflict
resolution or support every stash shape/backend.

The existing file list, Working/Index switch, loading state, partial-text notice
and Review updates retry are reused. No new screen, stash mode or setting is
introduced. Native/frontend mismatch asks for an app restart rather than treating
old partial data as a complete review.

See [verification](verification/2026-10-30-large-stash.md).
