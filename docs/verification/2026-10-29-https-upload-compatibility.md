# HTTPS upload compatibility — 2026-10-29

## Observed user failure

Native Git reported `RPC failed; HTTP 400 curl 22`, followed by a sideband disconnect. Read-only anonymous `ls-remote --heads` on the selected public destination returned no branch heads at inspection time. Local HEAD's generated pack estimate was 11,447,099 bytes; no oversized individual tracked file explained the error. The user reported no VPN/proxy. These observations do not establish the cause of the actual GitHub rejection.

No upload, branch write, token inspection or Git configuration change was performed against the user's destination. A fresh explicit user-confirmed attempt is required to establish whether the workaround applies there.

## Reproduction and verification

- Authenticated TLS loopback test with real Git-http-backend: **passed**. The unchanged normal small push and fetch authenticate correctly.
- A new incompressible 2 MiB staged payload causes a chunked POST. A disposable server configured to reject chunked bodies returns HTTP 400. Exactly one rejected attempt is observed; the remote branch remains at its previous commit.
- Switching to the explicit 16 MiB buffered option invalidates the old review token before any upload. A fresh preview reads the current destination; explicit confirmation publishes the exact reviewed commit.
- Repository config bytes remain unchanged; app credentials are not written to the configured plaintext helper. Existing authenticated redirect-isolation checks still pass.
- Native Push/ref integration: **20 passed**. Initial-publication integration: **10 passed**. Includes stale destination, divergence, remote races, non-force/single-branch scope, submodule publication checks, tracking/upstream, hooks and dirty-work preservation.
- Frontend TypeScript check and **45 unit tests** passed.
- Workspace all-target Clippy with warnings denied and diff whitespace checks passed.
- Canonical debug GitNova.app rebuilt (70.99 MiB); embedded UI and local bundle verification passed. No second app or remote publication was created.
- Real-core browser Push flows: **2 passed**. Per-operation option defaults off; non-HTTPS selection is refused before writing; disabling it preserves normal Push; divergent remote remains blocked. Browser transport is emulated; Git results use real Rust and disposable repositories.

Only one verification runner was active at a time. All test writes used temporary repositories and loopback endpoints.

## Scope

Compatibility is an opt-in operation input in the existing Push form, not persistent configuration. The selected exact HTTPS URL receives a process-only 16 MiB `http.postBuffer`; larger packs can still stream. This mode does not bypass authentication, branch rules, TLS checks or host isolation. It never automatically retries a write. Runtime success on the user's actual GitHub destination has not been established by these tests.

Machine-local logs: `/tmp/gitnova-m60-https.log`, `/tmp/gitnova-m60-push.log`, `/tmp/gitnova-m60-ui.log`, `/tmp/gitnova-m60-clippy.log`.
