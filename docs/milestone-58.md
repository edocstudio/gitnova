# M58 — saved host accounts authenticate HTTPS Git

## Defect

Reconnect verified and saved a GitHub/GitLab API token, while native Git still
used only external credential helpers/SSH. A public destination could pass
anonymous inspection and then fail publication with `could not read Username …
terminal prompts disabled`. Reconnecting the API account could not fix that gap.

## Change

The macOS app registers its own executable as a **process-local Git credential
helper** for eligible HTTPS transports. It handles the credential protocol before
starting Tauri, opening a catalog or creating a window. Push destination reads,
Fetch/Pull, Push (including tags/deletion) and Clone share the native read/write
transport plumbing. No additional dialog, setting or graph control is needed.

- Git asks for credentials through pipes; the helper reads the same
  `gitnova.hosting.v1` Keychain item used by Settings accounts. Token/password
  values are never passed in argv, URLs, environment variables or new files.
- The parent passes only credential-free destination URLs in a temporary scope.
  A helper request must match the selected HTTPS scheme, authority/port and
  repository path. Redirects are disabled for bridged commands. Out-of-scope
  requests receive no credentials, even if another company account is saved.
- Account matching uses exact normalized origin and segment-aligned base paths,
  choosing the most specific saved host. Ambiguous provider records, malformed tokens and denied Keychain access fail
  closed. The selected saved account supplies its own login, overriding a legacy
  Git-config username for that credential-free destination. Only a
  genuinely missing item permits fallback; a denied item does not.
- When no app account exists, a clean bounded `git credential fill` consults
  existing Git helpers. It strips inherited process-scoped Git configuration and
  carries a recursion guard. The native helper does not persist tokens in other
  helpers or erase saved app accounts on remote rejection.
- SSH agents, local/file transports and offline Git reads/writes retain their
  existing behavior. Unresolved submodule-only transport scopes retain ordinary
  Git credential handling; no broad cross-repository credential scope is invented.
- Authentication failures receive a translated action-oriented message. A failed
  Push still retains conservative uncertainty and inspection guidance; this does
  not relax remote-effect reporting, write previews or fresh validation.

Settings now describes the host account as Git authentication / PR/MR / CI. The
Push hint and bilingual FAQ explain the shared HTTPS account and separate SSH
agent. A token still needs the provider/repository permissions for each operation;
an API reconnect alone does not grant write access.

## Verification

Tests use disposable fixtures only. Protocol tests cover exact host/port/path
isolation, longer hosting base paths, HTTP refusal, duplicate/malformed requests,
ambiguous accounts, denied access, saved-login precedence and secret-free invocation.
A real Git HTTPS loopback server challenges for Basic authentication; selected
Push preview, confirmed publication and Fetch succeed through a fixture helper
with a quoted path. The published remote OID matches the reviewed commit. A
URL-scoped plaintext store receives no token, and an authenticated same-host
redirect is refused even when a less-specific Git setting enables redirects.
Repository config remains unchanged during the operations; no credential file
is persisted. No personal
Keychain account, GitHub server or developer project is used as a test write.

The macOS Keychain permission prompt may appear when the rebuilt helper first
reads the saved account. Actual user account permissions and live publication
remain user-driven; automated checks never push the developer's repository.

Reference: [Git credential protocol](https://git-scm.com/docs/git-credential),
[custom credential helpers](https://git-scm.com/docs/gitcredentials).


Verification: [2026-10-29 authentication regression](verification/2026-10-29-https-host-auth.md).
