// Native-host packaging only. No upload, signing credentials or app installation.
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {dirname,join} from 'node:path';
import {reportPrerequisites} from './check-prerequisites.mjs';
import {verifyLicenseInventory} from './verify-bundle-licenses.mjs';
const project=join(dirname(fileURLToPath(import.meta.url)),'..');
if(!['win32','linux','darwin'].includes(process.platform))throw new Error('Unsupported desktop platform.');
if(!reportPrerequisites())process.exit(1);
const report=verifyLicenseInventory();
const probe=spawnSync('rustc',['-vV'],{encoding:'utf8',timeout:8000});
const host=probe.stdout?.match(/^host: (.+)$/m)?.[1];
if(probe.status!==0||!host||report.target!==host)throw new Error('Regenerate the license inventory for this native host before packaging; see docs/platforms.md.');
const args=process.platform==='darwin'?[join(project,'scripts/build-local.mjs')]:[join(project,'scripts/tauri.mjs'),'build','--ci','--','--locked'];
const result=spawnSync(process.execPath,args,{cwd:project,stdio:'inherit',env:{...process.env,CARGO_BUILD_JOBS:process.env.CARGO_BUILD_JOBS??'2'}});
if(result.error)throw result.error;
process.exitCode=result.status??1;
