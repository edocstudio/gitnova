// Local macOS artifact only: no release upload, account signing or notarization.
import { spawnSync, execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, resolve, basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const project = resolve(dirname(fileURLToPath(import.meta.url)), '..');
if (process.platform !== 'darwin') throw new Error('build:local currently creates a macOS app; use the normal Tauri build for other platforms.');
const env = { ...process.env, CARGO_BUILD_JOBS: '1', APPLE_SIGNING_IDENTITY: '-' };
for (const key of ['APPLE_CERTIFICATE', 'APPLE_CERTIFICATE_PASSWORD', 'APPLE_ID', 'APPLE_PASSWORD', 'APPLE_TEAM_ID', 'APPLE_API_ISSUER', 'APPLE_API_KEY', 'APPLE_API_KEY_PATH', 'APPLE_PROVIDER_SHORT_NAME']) delete env[key];
// Do not overwrite executable/assets used by an open native window.
const processes=execFileSync('/bin/ps',['-A','-o','comm='],{encoding:'utf8',timeout:5000});
const bundleName='GitNova';
if(processes.split('\n').some(line=>line.trim().endsWith('/gitnova-desktop'))) throw new Error('Close GitNova before building. Local builds update GitNova.app; no alternate app is created.');
const config=JSON.parse(readFileSync(join(project,'src-tauri/tauri.local.conf.json'),'utf8'));
config.productName=bundleName;
const build = spawnSync(process.execPath, [join(project, 'node_modules/@tauri-apps/cli/tauri.js'), 'build', '--debug', '--features', 'custom-protocol', '--bundles', 'app', '--config', JSON.stringify(config), '--ci', '--', '--locked'], { cwd: project, env, stdio: 'inherit' });
if (build.error) throw build.error;
if (build.status !== 0) process.exit(build.status ?? 1);
const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps', '--locked', '--offline', '--manifest-path', join(project, 'src-tauri/Cargo.toml')], { cwd: project, env, encoding: 'utf8', timeout: 30000, maxBuffer: 2 * 1024 * 1024 }));
const app = join(metadata.target_directory, `debug/bundle/macos/${bundleName}.app`);
const executable = execFileSync('/usr/libexec/PlistBuddy', ['-c', 'Print :CFBundleExecutable', join(app, 'Contents/Info.plist')], { encoding: 'utf8', timeout: 10000 }).trim();
if (!executable || basename(executable) !== executable) throw new Error('Unexpected bundle executable name.');
execFileSync('/usr/bin/codesign', ['--verify', '--deep', '--strict', app], { stdio: 'inherit', timeout: 30000 });
execFileSync(join(app, 'Contents/MacOS', executable), ['--verify-bundle'], { cwd: '/tmp', env: { ...env, PATH: '/usr/bin:/bin:/usr/sbin:/sbin' }, stdio: 'inherit', timeout: 30000 });
console.log(`Local app verified: ${app}`);
