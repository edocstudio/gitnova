// A hidden real WebKit window and a real temporary Git repository; no user catalog.
import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, openSync, closeSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import { once } from 'node:events';
const project=resolve(dirname(fileURLToPath(import.meta.url)), '..');
const workspace=resolve(project,'../..');
if(process.platform!=='darwin') throw new Error('Local native smoke currently targets macOS.');
const env={...process.env,CARGO_BUILD_JOBS:'1'};
const metadata=JSON.parse(execFileSync('cargo',['metadata','--no-deps','--locked','--offline','--format-version','1'],{cwd:workspace,env,encoding:'utf8',timeout:30000,maxBuffer:2*1024*1024}));
const bundleName='GitNova';
const executable=join(metadata.target_directory,`debug/bundle/macos/${bundleName}.app/Contents/MacOS/gitnova-desktop`);
const check=execFileSync(executable,['--verify-bundle'],{encoding:'utf8',timeout:30000});
if(!check.includes('native-smoke=enabled')) throw new Error('Rebuild the current debug app before running the isolated native smoke.');
const probe=execFileSync('/bin/ps',['-A','-o','comm='],{encoding:'utf8',timeout:5000});
if(probe.split('\n').some(line=>line.trim().endsWith('/gitnova-desktop'))) throw new Error('Close existing GitNova windows before the isolated native smoke.');
execFileSync('cargo',['build','-p','gitnova-core','--example','ui_bridge','--locked','--offline','-j','1'],{cwd:workspace,env,stdio:'inherit'});
const fixture=mkdtempSync(join(tmpdir(),'gitnova-native-smoke-'));
const repo=join(fixture,'repository');mkdirSync(repo);
const git=args=>execFileSync('/usr/bin/git',['-C',repo,'-c','user.name=Native Smoke','-c','user.email=smoke@example.invalid','-c','core.hooksPath=/dev/null','-c','commit.gpgsign=false',...args],{encoding:'utf8',timeout:10000,env:{...env,GIT_CONFIG_NOSYSTEM:'1',GIT_CONFIG_GLOBAL:'/dev/null'}}).trim();
git(['init','-b','main']);writeFileSync(join(repo,'file.txt'),'native real Git smoke\n');git(['add','.']);git(['commit','-m','Native smoke base']);
const head=git(['rev-parse','HEAD']);const state=join(fixture,'state.json');
const add=JSON.parse(execFileSync(join(metadata.target_directory,'debug/examples/ui_bridge'),[state],{input:JSON.stringify({command:'add_repository',args:{path:repo}}),encoding:'utf8',timeout:30000}));
if(add.error) throw new Error(add.error.message);
const log=join(fixture,'native.log');const fd=openSync(log,'a',0o600);
const launchEnv={...env,PATH:'/usr/bin:/bin:/usr/sbin:/sbin',GITNOVA_NATIVE_SMOKE:'1',GITNOVA_SMOKE_STATE:state};
let first;let second;let passed=false;let launchError;
const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const marker=async name=>{const end=Date.now()+30000;while(Date.now()<end){if(launchError)throw launchError;if(first.exitCode!==null||first.signalCode!==null) throw new Error(`Native app exited before ${name}`);if(readFileSync(log,'utf8').includes(`GITNOVA_NATIVE_SMOKE:${name}`))return;await wait(100);}throw new Error(`Timed out waiting for ${name}`);};
const stop=async child=>{if(!child||child.exitCode!==null||child.signalCode!==null)return;child.kill('SIGTERM');await Promise.race([once(child,'exit'),wait(3000)]);if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await Promise.race([once(child,'exit'),wait(3000)]);}};
try {
  first=spawn(executable,[],{cwd:'/tmp',env:launchEnv,stdio:['ignore',fd,fd]});
  first.on('error',error=>{launchError=error;});
  await marker('CATALOG_OK');await marker('GRAPH_OK');
  second=spawn(executable,[],{cwd:'/tmp',env:launchEnv,stdio:['ignore',fd,fd]});
  const exit=await Promise.race([once(second,'exit'),wait(10000).then(()=>{throw new Error('Second instance did not exit');})]);
  if(exit[0]!==0)throw new Error(`Second instance exited with ${exit}`);
  await marker('REUSED_WINDOW');
  if(git(['rev-parse','HEAD'])!==head||git(['status','--porcelain'])!=='')throw new Error('Native smoke changed the fixture Git state');
  passed=true;console.log('PASS: packaged WebKit UI → native catalog and real Git graph; duplicate launch reused the first instance.');
} finally {
  await stop(second);await stop(first);closeSync(fd);
  if(passed && [first,second].every(child=>!child||child.exitCode!==null||child.signalCode!==null))rmSync(fixture,{recursive:true,force:true});else console.error(`Native smoke fixture retained for inspection: ${fixture}`);
}
