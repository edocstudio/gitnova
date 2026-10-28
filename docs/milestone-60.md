# M60 — Explicit bounded HTTPS upload compatibility

An HTTPS push reported RPC HTTP 400 and a disconnected sideband. A read-only inspection of the selected public destination showed no branch heads at that moment. The current local HEAD's estimated upload pack was about 10.9 MiB. There was no evidence proving the actual GitHub rejection was caused by chunked transfer; a successful authenticated write against that server was deliberately not attempted.

The existing Push form now offers **Upload compatibility → Use buffered HTTPS upload (16 MiB)**. It defaults off, belongs to the reviewed operation, and is not a persistent preference. It is included in the preview token. Confirmation uses only a process-local, exact-URL-scoped `http.postBuffer=16777216`; it never rewrites repository/global Git configuration or changes the destination/credentials. Packs exceeding that bound can still stream. This is a targeted transport workaround, not a promise that every HTTP 400 will be fixed.

The existing form expands compatibility guidance after a reported HTTP 400. It does not select the option or replay a failed write. Users must inspect and freshly review the live destination before confirming another push. HTTPS-only validation, non-force scope, exact source/destination checks, submodule publication checks, hooks, tracking/upstream handling and partial-failure reporting remain intact.

No new screen, global navigation or permanent setting is introduced. Existing input, review and confirmation surfaces carry this per-operation choice. English and Thai labels explain its memory and retry semantics.

A disposable authenticated TLS Git-http-backend test reproduces a >1 MiB request rejected as HTTP 400 when chunked. Exactly one rejection occurs, the remote stays at the previous commit, switching the option invalidates the old token, a fresh preview sees the unchanged remote, and explicitly confirming the buffered push publishes the exact new commit. Git config and credential-store files remain untouched. A local-path browser flow checks the option defaults off, refuses incompatible transport before writing, and normal Push still succeeds when disabled.

Git documentation: <https://git-scm.com/docs/git-config#Documentation/git-config.txt-httppostBuffer>. Raising postBuffer is useful only for chunked-transfer compatibility, increases memory consumption, and is not a general solution to push errors. GitNova therefore limits this opt-in to 16 MiB instead of imposing a large global buffer.
