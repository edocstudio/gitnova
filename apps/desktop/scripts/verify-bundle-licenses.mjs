// Inclusion/integrity checks only; this does not certify license compliance.
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const sha = data => createHash('sha256').update(data).digest('hex');
export function verifyLicenseInventory(sourceRoot = root) {
  const report = JSON.parse(readFileSync(join(sourceRoot, 'docs/licenses/dependency-inventory.json'), 'utf8'));
  if (report.schemaVersion !== 1 || report.kind !== 'gitnova-license-inventory' || report.auditComplete !== false)
    throw new Error('Unexpected inventory schema/audit claim; review the inventory and verification policy.');
  for (const [file, field] of [['Cargo.lock', 'cargoLockSha256'], ['apps/desktop/package-lock.json', 'npmLockSha256']]) {
    if (sha(readFileSync(join(sourceRoot, file))) !== report[field])
      throw new Error(`License inventory is stale for ${file}; regenerate and review it before packaging.`);
  }
  return report;
}
export function verifyBundleLicenses(app, sourceRoot = root) {
  return verifyResourceDirectory(join(app, 'Contents/Resources'), sourceRoot);
}
export function verifyResourceDirectory(resources, sourceRoot = root) {
  const report = verifyLicenseInventory(sourceRoot);
  const config = JSON.parse(readFileSync(join(sourceRoot, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'));
  let checked = 0;
  for (const [source, destination] of Object.entries(config.bundle.resources)) {
    if (!destination.startsWith('licenses/')) continue;
    if (!/^licenses\/[A-Za-z0-9._-]+$/.test(destination)) throw new Error('Unexpected license resource destination.');
    const expected = readFileSync(resolve(sourceRoot, 'apps/desktop/src-tauri', source));
    const actual = readFileSync(join(resources, destination));
    if (!expected.equals(actual)) throw new Error(`Bundled license resource differs: ${destination}`);
    checked++;
  }
  if (checked !== 5) throw new Error('Expected all five project/dependency license resources.');
  return { resourcesChecked: checked, auditComplete: report.auditComplete, unresolved: report.unresolved.length };
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.argv.length !== 3) throw new Error('Usage: node verify-bundle-licenses.mjs /path/to/GitNova.app');
  console.log(JSON.stringify(verifyBundleLicenses(resolve(process.argv[2]))));
}
