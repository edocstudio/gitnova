// A hidden real platform WebView window and a real temporary Git repository; no user catalog.
import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, openSync, closeSync, rmSync } from 'node:fs';
import { hashFile as hash } from './hash-file.mjs';
import { dirname, join, resolve, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import { once } from 'node:events';
const project=resolve(dirname(fileURLToPath(import.meta.url)), '..');
const workspace=resolve(project,'../..');
const args=process.argv.slice(2),options={};
for(let i=0;i<args.length;i+=2){if(!['--executable','--bridge','--report','--artifact'].includes(args[i])||!args[i+1]||options[args[i]])throw new Error('Use --executable, --bridge, --report and --artifact with one value each.');options[args[i]]=args[i+1];}
const explicit=!!options['--executable']&&!!options['--bridge'];
if((args.length&&!explicit)||!!options['--report']!==!!options['--artifact'])throw new Error('Pass both executable/bridge; report and artifact must also be paired.');
if(process.platform!=='darwin'&&!explicit)throw new Error('Pass installed native executable and bridge on this platform.');
const env={...Object.fromEntries(Object.entries(process.env).filter(([key])=>!key.startsWith('GIT_'))),CARGO_BUILD_JOBS:'1'};
const metadata=explicit?null:JSON.parse(execFileSync('cargo',['metadata','--no-deps','--locked','--offline','--format-version','1'],{cwd:workspace,env,encoding:'utf8',timeout:30000,maxBuffer:2*1024*1024}));
const bundleName='GitNova';
const executable=explicit?resolve(options['--executable']):join(metadata.target_directory,`debug/bundle/macos/${bundleName}.app/Contents/MacOS/gitnova-desktop`);
const bridge=explicit?resolve(options['--bridge']):join(metadata.target_directory,'debug/examples/ui_bridge');
const check=execFileSync(executable,['--verify-bundle'],{encoding:'utf8',timeout:30000});
if(!check.includes('native-acceptance=enabled')) throw new Error('Rebuild the current debug app before running the isolated native smoke.');
const version=check.match(/GitNova ([0-9]+\.[0-9]+\.[0-9]+)/)?.[1];if(!version)throw new Error('Missing installed app version.');
const probe=process.platform==='win32'?execFileSync('powershell.exe',['-NoLogo','-NoProfile','-NonInteractive','-Command',"Get-Process | Where-Object { $_.ProcessName -in @('gitnova-desktop','GitNova') } | ForEach-Object { $_.ProcessName }"],{encoding:'utf8',timeout:10000}):execFileSync('/bin/ps',['-A','-o','comm='],{encoding:'utf8',timeout:5000});
if(process.platform==='win32'?probe.trim():probe.split('\n').some(line=>line.trim().endsWith('/gitnova-desktop')))throw new Error('Close existing GitNova windows before the isolated native smoke.');
if(!explicit)execFileSync('cargo',['build','-p','gitnova-core','--example','ui_bridge','--locked','--offline','-j','1'],{cwd:workspace,env,stdio:'inherit'});
const fixture=mkdtempSync(join(tmpdir(),'gitnova-native-smoke-'));
const repo=join(fixture,'repository');mkdirSync(repo);
const git=args=>execFileSync('git',['-C',repo,'-c','user.name=Native Smoke','-c','user.email=smoke@example.invalid','-c','core.hooksPath=/dev/null','-c','commit.gpgsign=false',...args],{encoding:'utf8',timeout:10000,env:{...env,GIT_CONFIG_NOSYSTEM:'1',GIT_CONFIG_GLOBAL:process.platform==='win32'?'NUL':'/dev/null'}}).trim();
git(['init','-b','main']);writeFileSync(join(repo,'file.txt'),'native real Git smoke\n');git(['add','.']);git(['commit','-m','Native smoke base']);
const head=git(['rev-parse','HEAD']);const state=join(fixture,'state.json');
const add=JSON.parse(execFileSync(bridge,[state],{input:JSON.stringify({command:'add_repository',args:{path:repo}}),encoding:'utf8',timeout:30000}));
if(add.error) throw new Error(add.error.message);
const log=join(fixture,'native.log');const fd=openSync(log,'a',0o600);
const isolatedHome=join(fixture,'home');mkdirSync(isolatedHome);
const launchEnv={...env,HOME:isolatedHome,USERPROFILE:isolatedHome,XDG_CONFIG_HOME:join(isolatedHome,'config'),XDG_DATA_HOME:join(isolatedHome,'data'),XDG_CACHE_HOME:join(isolatedHome,'cache'),APPDATA:join(isolatedHome,'AppData','Roaming'),LOCALAPPDATA:join(isolatedHome,'AppData','Local'),GITNOVA_NATIVE_SMOKE:'1',GITNOVA_SMOKE_STATE:state};
let first;let second;let passed=false;let launchError;
const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const marker=async name=>{const end=Date.now()+30000;while(Date.now()<end){if(launchError)throw launchError;if(first.exitCode!==null||first.signalCode!==null) throw new Error(`Native app exited before ${name}`);if(readFileSync(log,'utf8').includes(`GITNOVA_NATIVE_SMOKE:${name}`))return;await wait(100);}throw new Error(`Timed out waiting for ${name}`);};
const stop=async child=>{if(!child||child.exitCode!==null||child.signalCode!==null)return;child.kill('SIGTERM');await Promise.race([once(child,'exit'),wait(3000)]);if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await Promise.race([once(child,'exit'),wait(3000)]);}};
try {
  first=spawn(executable,['--native-acceptance'],{cwd:fixture,env:launchEnv,stdio:['ignore',fd,fd]});
  first.on('error',error=>{launchError=error;});
  await marker('CATALOG_OK');await marker('GRAPH_OK');
  second=spawn(executable,['--native-acceptance'],{cwd:fixture,env:launchEnv,stdio:['ignore',fd,fd]});
  const exit=await Promise.race([once(second,'exit'),wait(10000).then(()=>{throw new Error('Second instance did not exit');})]);
  if(exit[0]!==0)throw new Error(`Second instance exited with ${exit}`);
  await marker('REUSED_WINDOW');
  if(git(['rev-parse','HEAD'])!==head||git(['status','--porcelain'])!=='')throw new Error('Native smoke changed the fixture Git state');
  if(options['--report'])writeFileSync(resolve(options['--report']),JSON.stringify({schemaVersion:1,kind:'gitnova-installed-acceptance',version,passed:true,platform:process.platform,arch:process.arch,artifact:basename(options['--artifact']),artifactSha256:hash(resolve(options['--artifact'])),installedBinarySha256:hash(executable),checks:['embedded-ui','native-catalog-ipc','real-git-graph-ipc','single-instance','fixture-head-and-working-tree-preserved'],scope:'Startup and native IPC smoke only; full browser workflows, OS credentials and manual GUI acceptance are separate.'},null,2)+'\n');
  passed=true;console.log('PASS: installed native WebView UI → native catalog and real Git graph; duplicate launch reused the first instance.');
} finally {
  await stop(second);await stop(first);closeSync(fd);
  const stopped=[first,second].every(child=>!child||child.exitCode!==null||child.signalCode!==null);
  if(passed && stopped)rmSync(fixture,{recursive:true,force:true});else console.error(`Native smoke fixture retained for inspection: ${fixture}`);
  if(!stopped){if(options['--report'])rmSync(resolve(options['--report']),{force:true});throw new Error('Native acceptance processes did not stop; successful evidence was removed.');}
}
