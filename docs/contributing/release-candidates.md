# Release candidates, Linux signing and download handoff

This workflow prepares alpha packages and can attach them to a draft GitHub
prerelease; it does not publish a release, certify redistribution, deploy the
website or configure signing accounts.
macOS and Windows trusted publisher signing remain unavailable. Keep the
[binary distribution checklist](binary-distribution.md) and unresolved
target-specific dependency/asset terms visible.

## 1. Build and test exact candidates

Commit the reviewed changes through the normal protected-branch PR flow. Run
**Actions → release candidate → Run workflow** on that exact reviewed ref.
Keep `validation_scope` at `full` to prepare installers. The optional
`windows-regressions` scope runs focused native path/worktree/submodule and
stash UI diagnostics on Windows; it skips full validation, assembly and draft
release creation. Passing diagnostics is not release approval. Run `full`
again at the exact intended source revision after fixing any diagnostic failures.

Leave `sign_linux` false until the owner key is configured. Linux can be
distributed as unsigned alpha without GPG; a missing key is not itself a Linux
installation blocker. Unsigned downloads must retain their publisher-authenticity
warning. License review and native acceptance remain separate requirements.
The workflow reuses
the full three-platform checks without weakening any existing checks, then uses
release-profile packaging on all targets. Ordinary CI retains its macOS debug
candidate path. `npm run build:release` is the native-host candidate build command;
collect the matching target license inventory first as described in
[platform prerequisites](../platforms.md).

Outputs currently cover the macOS runner's architecture as an ad-hoc-signed
`.app` ZIP, Windows x64 unsigned NSIS `.exe`, and Linux x64 `.deb`/`.AppImage`.
This does not add Intel macOS/ARM Windows/ARM Linux artifacts automatically.
The macOS ZIP can be extracted and the app moved into Applications; no DMG or
notarization claim is made. Users of built apps do not need Rust/Cargo/Node;
Git and platform runtime libraries are still required.

The assembly step refuses debug profiles, changed installer bytes, mixed source
commits/versions, missing package types and incomplete native startup evidence.
`release-candidate.json` records the exact files, SHA-256, size, source, target,
signature limitations and unresolved license counts. `SHA256SUMS` also binds the
manifest. Candidate reports and target-specific license evidence accompany the
CI artifact. These are integrity records, not approval or publisher authenticity.

## 2. Owner creates the Linux publisher key

GPG does not require purchasing a commercial certificate. This provides detached
publisher signatures; it does not establish OS trust, create a signed apt
repository or embed a signature inside an AppImage.

Install GnuPG on your own machine. Choose and securely back up a passphrase,
private key and revocation certificate. Do not paste them into chat, Git, issues
or release assets. The commands below are for the owner to run, not CI:

```bash
umask 077
export GNUPGHOME="$HOME/.gnupg-gitnova-release"
mkdir -p "$GNUPGHOME"
chmod 700 "$GNUPGHOME"
gpg --quick-generate-key 'GitNova Releases (edocstudio)' ed25519 cert 1y
gpg --with-colons --fingerprint --list-keys
# Copy the full primary fingerprint into this variable; never use a short ID.
read -r GITNOVA_RELEASE_FINGERPRINT
gpg --quick-add-key "$GITNOVA_RELEASE_FINGERPRINT" ed25519 sign 1y
gpg --armor --export-secret-subkeys "$GITNOVA_RELEASE_FINGERPRINT" > "$GNUPGHOME/ci-signing-subkey.asc"
chmod 600 "$GNUPGHOME/ci-signing-subkey.asc"
gpg --armor --export "$GITNOVA_RELEASE_FINGERPRINT" > "$GNUPGHOME/release-key.asc"
```

Keep the certification-capable primary secret key out of CI. In GitHub, the owner
creates the `release-signing` environment and configures:

| Name | Type | Value |
| --- | --- | --- |
| `LINUX_GPG_PRIVATE_KEY` | Environment secret | Armored signing-subkey export |
| `LINUX_GPG_PASSPHRASE` | Environment secret | Signing key passphrase |
| `LINUX_GPG_FINGERPRINT` | Environment variable | Full primary fingerprint |

The workflow imports into a disposable keyring outside artifact paths, passes
the passphrase over stdin, signs Linux files plus `SHA256SUMS`, verifies the
expected fingerprint, exports only the public key, then removes the keyring.
Only the separate optional draft job has release write permission. Failed signing does not upload the final
release-candidate artifact. Publish the public fingerprint separately on the
official website/repository so users can authenticate the supplied public key.

With `sign_linux` enabled, outputs add `.asc` sidecars, `release-key.asc` and
`linux-signing.json`. Do not alter/repackage files afterward. Rebuild and rerun
acceptance if any distributed installer bytes change.

## 3. Review licenses and actual native behavior

Review direct/transitive dependencies, native components, included assets,
license choices and source/notice obligations for each target. Existing inventory
gaps are deliberately preserved; collecting texts is not a completed audit.
Record decisions against the exact source and artifact hashes. Include all
required texts/materials and inspect their locations inside the packages.

Complete native manual installation/upgrade/uninstall and workflow acceptance
on clean machines using [the existing checklist](unsigned-alpha.md#native-manual-acceptance-record).
CI now installs, reinstalls and purges the Debian package on a disposable runner;
AppImage startup uses its extracted AppRun entrypoint. Windows exercises NSIS
installation, same-version reinstall and uninstallation. These automated checks
do not replace clean-machine manual acceptance or previous-version migration.
Trusted macOS/Windows signing remains a separate later task. Only publish an
unsigned alpha if the owner explicitly accepts the documented limitations.

## 4. Owner publication and website handoff

For a concrete installer handoff, create an alpha tag such as
`v0.1.0-alpha.1` at the reviewed source commit, run **release candidate** on that
tag, and enter the same tag in `draft_tag`. Leave `sign_linux` false for the
unsigned alpha route. After all three-platform checks and release builds pass,
the optional draft job validates the package hashes and resolves the existing
tag (including annotated tags) to the tested commit before creating a draft.
It never creates/moves tags, overwrites existing release assets or publishes.
Signed Linux sidecars, when present, are independently verified in a temporary
public-key-only keyring before draft upload. No owner private key is needed to
upload unsigned alpha files.

The draft includes the exact
packages, `SHA256SUMS`, signatures/public key, candidate and acceptance/license
evidence contained inside the packages, and honest release notes. Full native
acceptance reports and target-specific inventory remain in the workflow artifact;
review these and attach any required accompanying material before publication.
The candidate manifest deliberately keeps `publicationGatePassed: false` and
all unresolved license counts: creating a draft is not approval of those gaps.
After review, verify every downloaded attachment against the manifest and make
the prerelease public using GitHub's **Publish release** action. The workflow
never performs that final publication automatically. Users download native
installers from Releases; they do not need the developer toolchain.

The separate `gitnova-web` repository has a validated `lib/downloads.json` feed.
Keep it `unpublished` until the exact assets exist on the public prerelease.
Then follow its `docs/download-publication.md`, review the diff and deploy the
website separately. Do not copy private keys or candidate-only URLs into the site.

Linux recipient verification, after independently checking the publisher key's
full fingerprint:

```bash
gpg --import release-key.asc
gpg --fingerprint
gpg --verify SHA256SUMS.asc SHA256SUMS
sha256sum --ignore-missing --check SHA256SUMS
gpg --verify GitNova_VERSION_amd64.deb.asc GitNova_VERSION_amd64.deb
```

Replace the example package name with the exact downloaded name. Check that the
specific package reports `OK`; `--ignore-missing` skips other platforms rather
than asserting that you downloaded all assets. GPG validity only proves control
of the key you trust, not application safety or license compliance.

References: [Tauri Linux signing](https://v2.tauri.app/distribute/sign/linux/),
[Tauri distribution](https://v2.tauri.app/distribute/),
[GitHub Actions secrets](https://docs.github.com/en/actions/security-for-github-actions/security-guides/using-secrets-in-github-actions).
