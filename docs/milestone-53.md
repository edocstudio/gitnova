# M53 — First Push destination setup and hosted repository creation

This supersedes the M50/M52 repository-remote section in Settings. Settings now contains General and global Connections only. The existing Push context asks whether to add a missing destination, with Add remote and Cancel actions. Its inline setup offers an existing URL/local path, an existing repository on a saved host, or creation of a new GitHub/GitLab repository. Global account management opens the existing Settings dialog without losing the Push context.

New repository creation uses an actual authenticated account and bounded owner/namespace enumeration (first 100 entries). Name, description, owner, host and visibility are explicit review inputs. Private is the default; Public must be selected and explicitly acknowledged. No provider logo assets or additional screen/sidebar/navigation were introduced.

Creation follows provider contracts: GitHub uses the authenticated user's or selected organization's create-repository endpoint with `auto_init: false`; GitLab supplies the selected numeric namespace ID and `initialize_with_readme: false`. See [GitHub repository API](https://docs.github.com/en/rest/repos/repos#create-a-repository-for-the-authenticated-user) and [GitLab Projects API](https://docs.gitlab.com/api/projects/#create-a-project). Provider accounts require suitable repository creation permissions; CI read access alone is not sufficient. Native Git credentials/SSH for Push remain separate.

Review is read-only and refuses an existing project instead of silently adopting it. The token binds request, host, authenticated login, namespace/project and clone URL. The core revalidates under its operation guard, saves a started receipt, then performs one POST with redirects and retries disabled. Response identity, visibility, clone/web URLs and an empty branch list are verified. Server errors, transport uncertainty or mismatched results do not trigger automatic retries or deletion. The original request and reviewed destination remain in the durable receipt for investigation.

A successful creation returns its verified URL and a separately journaled result. The UI keeps that URL while reviewing local remote configuration. A stale or failed local config write does not recreate the hosted repository. Adding the remote uses the existing native config lock, exact URL review, private config recovery and stale-state checks. It returns to the existing Push review with the selected remote and upstream choice. Actual code publication remains a third explicit Git operation. No atomicity across provider creation, local configuration and push is claimed.

## Scope

GitHub personal accounts/available organizations and GitLab namespaces returned by the provider are supported. Larger namespace lists require using an existing repository URL for other destinations. Other Git hosts remain usable through existing HTTPS/SSH/local URLs; no unsupported creation-provider buttons are displayed. Creation uses a standard HTTPS clone URL and does not configure native Git authentication or force-push. Hosted API actions are currently backed by macOS Keychain credentials, as with existing hosting integrations.

The first screenshot's single root commit is an expected outcome of amending the earlier root after retaining the second commit's staged changes; this milestone does not modify that repository's history.

## Verification

- All 26 provider unit/HTTP contract tests passed, including new GitHub/GitLab empty creation, exact private/public payloads, namespace scope, no POST during preview, existing-project refusal, token-bound visibility, and uncertain mismatched response without retry/deletion.
- TypeScript and all 38 frontend unit/catalog tests passed. Strict Git/core/hosting/desktop Clippy across all targets passed.
- Real-core single-worker browser publication with 383 files passed through the new inline URL setup to a local bare remote and upstream assignment.
- Hosted-create UI uses a fake provider transport while local Git and remote config use the real Rust core: no repository-remotes section in Settings, connected owner selection, default Private, Public acknowledgement, no local remote before confirmation, stale config after hosted success with the created URL retained, successful local retry with exactly one provider creation, and no code push/fetch during setup. Provider contract tests separately exercise actual HTTP request/response handling on loopback servers. These checks do not claim live account creation or native WebView acceptance.

All mutating verification uses disposable repositories and mock provider servers. No user repository was created, configured or pushed by development tools. Screenshots from real company repositories were not copied into project assets.

Local macOS debug bundle rebuilt in place: `target/debug/bundle/macos/GitNova.app` (81.24 MiB). Ad-hoc signing and embedded-UI checks passed. The user closed the running instance before packaging; no alternate app bundle was created. A final browser rerun verified Settings return focus and the stale-local-config recovery path.
