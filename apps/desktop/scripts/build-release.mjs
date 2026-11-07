// Native release-profile candidate only; no upload or trusted signing claim.
import {execFileSync, spawnSync} from 'node:child_process';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {reportPrerequisites} from './check-prerequisites.mjs';
import {verifyLicenseInventory, verifyBundleLicenses} from './verify-bundle-licenses.mjs';
const project=join(dirname(fileURLToPath(import.meta.url)),'..');
if(!['darwin','linux','win32'].includes(process.platform))throw new Error('Unsupported native release platform.');
if(!reportPrerequisites())process.exit(1);
const host=execFileSync('rustc',['-vV'],{encoding:'utf8',timeout:8000}).match(/^host: (.+)$/m)?.[1];
if(verifyLicenseInventory().target!==host)throw new Error('Collect license evidence for this native target before packaging.');
const env={...process.env,CARGO_BUILD_JOBS:'1'};
// This candidate path deliberately never uses account certificates or notarizes.
for(const key of Object.keys(env))if(/^(APPLE_|WINDOWS_CERTIFICATE|TAURI_SIGNING_)/.test(key))delete env[key];
const args=[join(project,'scripts/tauri.mjs'),'build','--ci','--features','custom-protocol'];
if(process.platform==='darwin'){
  if(execFileSync('/bin/ps',['-A','-o','comm='],{encoding:'utf8',timeout:5000}).split('\n').some(line=>line.trim().endsWith('/gitnova-desktop')))throw new Error('Close GitNova before building.');
  env.APPLE_SIGNING_IDENTITY='-';
  args.push('--config',JSON.stringify({bundle:{active:true,targets:['app'],icon:['icons/icon.icns'],macOS:{minimumSystemVersion:'13.0',signingIdentity:'-'}}}));
}
args.push('--','--locked');
const result=spawnSync(process.execPath,args,{cwd:project,env,stdio:'inherit'});
if(result.error)throw result.error;
if(result.status!==0)process.exit(result.status??1);
if(process.platform==='darwin'){
  const metadata=JSON.parse(execFileSync('cargo',['metadata','--format-version','1','--no-deps','--locked','--offline'],{cwd:join(project,'src-tauri'),encoding:'utf8',timeout:30000,maxBuffer:2*1024*1024}));
  const app=join(metadata.target_directory,'release/bundle/macos/GitNova.app');
  console.log('Bundled attribution evidence:',verifyBundleLicenses(app));
  execFileSync('/usr/bin/codesign',['--verify','--deep','--strict',app],{stdio:'inherit',timeout:30000});
  execFileSync(join(app,'Contents/MacOS/gitnova-desktop'),['--verify-bundle'],{cwd:'/tmp',env:{...env,PATH:'/usr/bin:/bin:/usr/sbin:/sbin'},stdio:'inherit',timeout:30000});
}
