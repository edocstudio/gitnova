import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {reportPrerequisites} from './check-prerequisites.mjs';
const args=process.argv.slice(2);
if(['dev','build'].includes(args[0])&&!reportPrerequisites())process.exit(1);
const cli=fileURLToPath(new URL('../node_modules/@tauri-apps/cli/tauri.js',import.meta.url));
const child=spawnSync(process.execPath,[cli,...args],{stdio:'inherit'});
if(child.error){console.error('Could not start Tauri. Run npm ci in apps/desktop, then retry.');process.exitCode=1;}
else process.exitCode=child.status??(child.signal==='SIGINT'?130:1);
