// Visible installed WebView workflows, exclusively against disposable Git fixtures.
import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, openSync, closeSync, rmSync, realpathSync } from 'node:fs';
import { hashFile as hash } from './hash-file.mjs';
import { dirname, join, resolve, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import { once } from 'node:events';
const project=resolve(dirname(fileURLToPath(import.meta.url)), '..');
const workspace=resolve(project,'../..');
const args=process.argv.slice(2),options={};
for(let i=0;i<args.length;i+=2){if(!['--executable','--bridge','--report','--artifact','--ci-only'].includes(args[i])||!args[i+1]||options[args[i]])throw new Error('Use --executable, --bridge, --report and --artifact with one value each.');options[args[i]]=args[i+1];}
if(options['--ci-only']&&options['--ci-only']!=='true')throw new Error('--ci-only must be true when supplied.');
const ciOnly=options['--ci-only']==='true';
const explicit=!!options['--executable']&&!!options['--bridge'];
if((args.length&&!explicit)||!!options['--report']!==!!options['--artifact'])throw new Error('Pass both executable/bridge; report and artifact must also be paired.');
if(process.platform!=='darwin'&&!explicit)throw new Error('Pass installed native executable and bridge on this platform.');
const env={...Object.fromEntries(Object.entries(process.env).filter(([key])=>!key.startsWith('GIT_')&&!key.startsWith('GITNOVA_'))),CARGO_BUILD_JOBS:'1'};
const metadata=explicit?null:JSON.parse(execFileSync('cargo',['metadata','--no-deps','--locked','--offline','--format-version','1'],{cwd:workspace,env,encoding:'utf8',timeout:30000,maxBuffer:2*1024*1024}));
const bundleName='GitNova';
const executable=explicit?resolve(options['--executable']):join(metadata.target_directory,`debug/bundle/macos/${bundleName}.app/Contents/MacOS/gitnova-desktop`);
const bridge=explicit?resolve(options['--bridge']):join(metadata.target_directory,'debug/examples/ui_bridge');
const check=execFileSync(executable,['--verify-bundle'],{encoding:'utf8',timeout:30000});
if(!check.includes('native-acceptance=enabled')) throw new Error('Rebuild the current debug app before running the isolated native smoke.');
const version=check.match(/GitNova ([0-9]+\.[0-9]+\.[0-9]+)/)?.[1];if(!version)throw new Error('Missing installed app version.');
if(version!==JSON.parse(readFileSync(join(project,'package.json'),'utf8')).version)throw new Error('Build the current source version before native workflows.');
const probe=process.platform==='win32'?execFileSync('powershell.exe',['-NoLogo','-NoProfile','-NonInteractive','-Command',"Get-Process | Where-Object { $_.ProcessName -in @('gitnova-desktop','GitNova') } | ForEach-Object { $_.ProcessName }"],{encoding:'utf8',timeout:10000}):execFileSync('/bin/ps',['-A','-o','comm='],{encoding:'utf8',timeout:5000});
if(process.platform==='win32'?probe.trim():probe.split('\n').some(line=>line.trim().endsWith('/gitnova-desktop')))throw new Error('Close existing GitNova windows before the isolated native smoke.');
if(!explicit)execFileSync('cargo',['build','-p','gitnova-core','--example','ui_bridge','--locked','--offline','-j','1'],{cwd:workspace,env,stdio:'inherit'});
const fixture=realpathSync.native(mkdtempSync(join(tmpdir(),'gitnova-native-smoke-')));
const repo=join(fixture,'workflow-parent'),source=join(fixture,'child-source'),childRemote=join(fixture,'child.git'),parentRemote=join(fixture,'parent.git'),movedRemote=join(fixture,'moved-parent.git');
const git=(path,args)=>execFileSync('git',['-C',path,'-c','user.name=Native Workflow','-c','user.email=workflow@example.invalid','-c','core.hooksPath=/dev/null','-c','commit.gpgsign=false',...args],{encoding:'utf8',timeout:30000,env:{...env,GIT_CONFIG_NOSYSTEM:'1',GIT_CONFIG_GLOBAL:process.platform==='win32'?'NUL':'/dev/null'}}).trim();
const identity=path=>{for(const [key,value] of [['user.name','Native Workflow'],['user.email','workflow@example.invalid'],['commit.gpgsign','false']])git(path,['config','--local',key,value]);};
mkdirSync(source);git(source,['init','-b','main']);writeFileSync(join(source,'child.txt'),'base\n');git(source,['add','.']);git(source,['commit','-m','Native child base']);git(fixture,['init','--bare',childRemote]);git(childRemote,['symbolic-ref','HEAD','refs/heads/main']);git(source,['remote','add','origin',childRemote]);git(source,['push','origin','main']);const childBase=git(source,['rev-parse','HEAD']);
mkdirSync(repo);git(repo,['init','-b','main']);identity(repo);git(repo,['-c','protocol.file.allow=always','submodule','add',childRemote,'child']);const lines=Array.from({length:6000},(_,i)=>`setting_${i+1}: "value ${i+1}"`);writeFileSync(join(repo,'settings.yaml'),lines.join('\n')+'\n');git(repo,['add','.']);git(repo,['commit','-m','Native parent base']);git(fixture,['init','--bare',parentRemote]);git(fixture,['init','--bare',movedRemote]);git(repo,['remote','add','origin',parentRemote]);git(repo,['push','-u','origin','main']);const base=git(repo,['rev-parse','HEAD']);
const child=join(repo,'child');identity(child);git(child,['switch','--detach']);writeFileSync(join(child,'child.txt'),'unpublished\n');git(child,['commit','-am','Native unpublished child']);const childTip=git(child,['rev-parse','HEAD']);git(repo,['add','child']);git(repo,['commit','-m','Native child pointer']);lines[5101]='setting_5102: "native change"';writeFileSync(join(repo,'settings.yaml'),lines.join('\n')+'\n');git(repo,['commit','-am','Native distant change']);const head=git(repo,['rev-parse','HEAD']),index=git(repo,['write-tree']);writeFileSync(join(repo,'settings.yaml'),'# shifted\n'.repeat(150)+lines.join('\n')+'\n');
const state=join(fixture,'state.json');const add=JSON.parse(execFileSync(bridge,[state],{input:JSON.stringify({command:'add_repository',args:{path:repo}}),encoding:'utf8',timeout:30000}));if(add.error)throw new Error(add.error.message);
const log=join(fixture,'native.log');const fd=openSync(log,'a',0o600);
const isolatedHome=join(fixture,'home');mkdirSync(isolatedHome);
const launchEnv={...env,HOME:isolatedHome,USERPROFILE:isolatedHome,XDG_CONFIG_HOME:join(isolatedHome,'config'),XDG_DATA_HOME:join(isolatedHome,'data'),XDG_CACHE_HOME:join(isolatedHome,'cache'),APPDATA:join(isolatedHome,'AppData','Roaming'),LOCALAPPDATA:join(isolatedHome,'AppData','Local'),GITNOVA_NATIVE_SMOKE:'1',GITNOVA_PERF:'1',GITNOVA_NATIVE_WORKFLOWS:'1',GITNOVA_NATIVE_CI_WORKFLOW:ciOnly?'1':'0',GITNOVA_NATIVE_SOAK_FOREGROUND:'1',GITNOVA_SMOKE_STATE:state};
let first;let passed=false;let launchError;
const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const marker=async name=>{const end=Date.now()+30000;while(Date.now()<end){if(launchError)throw launchError;if(first.exitCode!==null||first.signalCode!==null) throw new Error(`Native app exited before ${name} (exit=${first.exitCode}, signal=${first.signalCode})`);if(readFileSync(log,'utf8').includes(`GITNOVA_NATIVE_SMOKE:${name}`))return;await wait(100);}throw new Error(`Timed out waiting for ${name}`);};
const stop=async child=>{if(!child||child.exitCode!==null||child.signalCode!==null)return;child.kill('SIGTERM');await Promise.race([once(child,'exit'),wait(3000)]);if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await Promise.race([once(child,'exit'),wait(3000)]);}};
try {
  first=spawn(executable,['--native-acceptance'],{cwd:fixture,env:launchEnv,stdio:['ignore',fd,fd]});
  first.on('error',error=>{launchError=error;});
  await marker('CATALOG_OK');await marker('GRAPH_OK');
  const end=Date.now()+240000;let result;
  while(Date.now()<end){
    if(first.exitCode!==null||first.signalCode!==null)throw new Error('Native workflow process exited');
    const file=join(fixture,'ui-soak.jsonl');let rows=[];try{rows=readFileSync(file,'utf8').trim().split('\n').filter(Boolean).map(line=>JSON.parse(line));}catch(error){if(error.code!=='ENOENT')throw error;}
    result=rows.find(row=>row.kind==='workflows-complete'||row.kind==='workflows-failed');if(result)break;await wait(250);
  }
  if(!result||result.kind!=='workflows-complete')throw new Error(JSON.stringify(result??{error:'Native workflow timeout'}));
  if(result.visibility!=='visible')throw new Error('WebView was not visible at completion');
  if(ciOnly){
    if(git(childRemote,['rev-parse','main'])!==childBase||git(parentRemote,['rev-parse','main'])!==base||git(repo,['remote','get-url','origin'])!==parentRemote||git(repo,['rev-parse','HEAD'])!==head||git(repo,['write-tree'])!==index||git(child,['rev-parse','HEAD'])!==childTip||git(child,['branch','--show-current'])!=='')throw new Error('Read-only CI workflow changed fixture identity or Git state');
    const expected='# shifted\n'.repeat(150)+lines.join('\n')+'\n';if(readFileSync(join(repo,'settings.yaml'),'utf8')!==expected||git(repo,['status','--porcelain'])!=='M settings.yaml'||git(child,['status','--porcelain'])!=='')throw new Error('CI workflow changed fixture content');
    if(git(childRemote,['for-each-ref','--format=%(refname)'])!=='refs/heads/main'||git(parentRemote,['for-each-ref','--format=%(refname)'])!=='refs/heads/main'||git(child,['for-each-ref','--format=%(refname)','refs/heads/'])!=='refs/heads/main')throw new Error('CI workflow published or created an unexpected ref');
  }else{
  if(git(childRemote,['rev-parse','review/native-child'])!==childTip||git(childRemote,['rev-parse','main'])!==childBase)throw new Error('Incorrect child publication');
  if(git(parentRemote,['rev-parse','review/native'])!==head||git(parentRemote,['rev-parse','main'])!==base)throw new Error('Incorrect parent publication');
  if(git(repo,['remote','get-url','origin'])!==movedRemote||git(repo,['rev-parse','HEAD'])!==head||git(repo,['write-tree'])!==index)throw new Error('Endpoint or history/index invariant failed');
  if(git(movedRemote,['for-each-ref'])!==''||git(child,['status','--porcelain'])!=='')throw new Error('Unexpected endpoint publication or child worktree edits');
  if(git(repo,['status','--porcelain'])!=='M settings.yaml')throw new Error('Unexpected parent working changes');
  const expected='# shifted\n'.repeat(150)+lines.join('\n').replace('native change','native corrected')+'\n';if(readFileSync(join(repo,'settings.yaml'),'utf8')!==expected)throw new Error('Editor did not save expected content');
  }
  if(options['--report'])writeFileSync(resolve(options['--report']),JSON.stringify({...result,schemaVersion:1,kind:'gitnova-native-workflows',version,passed:true,platform:process.platform,arch:process.arch,artifact:basename(options['--artifact']),artifactSha256:hash(resolve(options['--artifact'])),installedBinarySha256:hash(executable),checks:[...result.checks,'fixture-remote-refs-content-index-and-head-verified']},null,2)+'\n');
  passed=true;console.log('PASS: visible native workflow completed; provider CI data simulated, real Git invariants preserved.');
} finally {
  await stop(first);closeSync(fd);
  // This log belongs only to the synthetic catalog and contains no owner data.
  if(!passed)console.error(readFileSync(log,'utf8').slice(-16000));
  const stopped=[first].every(child=>!child||child.exitCode!==null||child.signalCode!==null);
  if(passed && stopped)rmSync(fixture,{recursive:true,force:true});else console.error(`Native smoke fixture retained for inspection: ${fixture}`);
  if(!stopped){if(options['--report'])rmSync(resolve(options['--report']),{force:true});throw new Error('Native acceptance processes did not stop; successful evidence was removed.');}
}
