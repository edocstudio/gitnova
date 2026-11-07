#!/usr/bin/env bash
# Real package-manager acceptance, confined to a disposable GitHub-hosted runner.
set -euo pipefail
if [[ ${CI:-} != true || ${RUNNER_ENVIRONMENT:-} != github-hosted || ${RUNNER_OS:-} != Linux ]]; then
  echo 'Debian installation checks require a disposable GitHub-hosted Linux runner.' >&2
  exit 1
fi
if [[ $# != 3 ]]; then
  echo 'Usage: check-debian-install.sh PACKAGE.deb BRIDGE EVIDENCE_DIRECTORY' >&2
  exit 1
fi
artifact=$(realpath "$1")
bridge=$(realpath "$2")
mkdir -p "$3"
evidence=$(realpath "$3")
package=$(dpkg-deb -f "$artifact" Package)
case "$package" in git-nova) ;; *) echo 'Unexpected Debian package identity.' >&2; exit 1 ;; esac
if dpkg-query -W -f='${Status}' "$package" 2>/dev/null | grep -q 'install ok installed'; then
  echo 'Refusing to replace an already installed package.' >&2
  exit 1
fi
cleanup() { sudo dpkg --purge "$package" >/dev/null; }
trap cleanup EXIT
sudo apt-get install -y "$artifact"
mapfile -t binaries < <(dpkg-query -L "$package" | grep '/bin/gitnova-desktop$')
mapfile -t texts < <(dpkg-query -L "$package" | grep '/licenses/LICENSE$')
[[ ${#binaries[@]} -eq 1 && ${#texts[@]} -eq 1 ]]
binary=${binaries[0]}
resources=$(dirname "$(dirname "${texts[0]}")")
node apps/desktop/scripts/verify-platform-resources.mjs "$resources"
dbus-run-session -- xvfb-run -a node apps/desktop/scripts/smoke-local.mjs --executable "$binary" --bridge "$bridge" --artifact "$artifact" --report "$evidence/deb-acceptance.json"
# Reinstall the exact candidate to exercise the upgrade/repair path. This does
# not establish compatibility with a previous release's user data migration.
before=$(sha256sum "$binary")
sudo apt-get install --reinstall -y "$artifact"
[[ $(sha256sum "$binary") == "$before" ]]
node apps/desktop/scripts/verify-platform-resources.mjs "$resources"
node apps/desktop/scripts/alpha-artifact.mjs --executable "$binary" --artifact "$artifact" --acceptance "$evidence/deb-acceptance.json" --resources "$resources" --output "$evidence/deb-artifact.json"
cleanup
trap - EXIT
[[ ! -e "$binary" && ! -e "${texts[0]}" ]]
node --input-type=module - "$artifact" "$evidence/deb-lifecycle.json" <<'NODE'
import {writeFileSync} from 'node:fs';
import {basename} from 'node:path';
import {hashFile} from './apps/desktop/scripts/hash-file.mjs';
const [artifact, output] = process.argv.slice(2);
writeFileSync(output, JSON.stringify({schemaVersion: 1, kind: 'gitnova-installer-lifecycle', artifact: basename(artifact), artifactSha256: hashFile(artifact), checks: ['apt-install', 'installed-native-acceptance', 'same-version-reinstall', 'dpkg-purge'], passed: true, limitations: ['Previous-version migration and manual GUI acceptance are not covered.']}, null, 2) + '\n');
NODE
