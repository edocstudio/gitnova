// Explicit opt-in installed-binary performance probe. All Git writes are fixtures.
import {execFileSync,spawn} from 'node:child_process';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,openSync,closeSync,existsSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {tmpdir,release,arch,totalmem,cpus} from 'node:os';
import {once} from 'node:events';
const [binaryArg,bridgeArg,outputArg]=process.argv.slice(2);
if(process.argv.length!==5)throw Error('Usage: node scripts/bench-native.mjs EXECUTABLE BRIDGE REPORT.json');
if(process.platform!=='darwin')throw Error('This local native performance launcher currently supports macOS only.');
const binary=resolve(binaryArg),bridge=resolve(bridgeArg),output=resolve(outputArg);
const env=Object.fromEntries(Object.entries(process.env).filter(([key])=>!key.startsWith('GIT_')));
const processes=execFileSync('/bin/ps',['-A','-o','comm='],{encoding:'utf8'});
if(processes.split('\n').some(line=>line.trim().endsWith('/gitnova-desktop')))throw Error('Quit GitNova before the isolated probe.');
const root=mkdtempSync(join(tmpdir(),'gitnova-native-smoke-')),repo=join(root,'repository'),home=join(root,'home');mkdirSync(repo);mkdirSync(home);
const git=args=>execFileSync('git',['-C',repo,'-c','user.name=Benchmark','-c','user.email=bench@example.invalid','-c','commit.gpgsign=false','-c','core.hooksPath=/dev/null',...args],{encoding:'utf8',env:{...env,GIT_CONFIG_NOSYSTEM:'1',GIT_CONFIG_GLOBAL:'/dev/null'}}).trim();
git(['init','-b','main']);
let stream='';for(let i=0;i<2000;i++){const text=`version ${i}\n`,message=`Performance ${i}\n`;stream+=`commit refs/heads/main\ncommitter Benchmark <bench@example.invalid> ${1700000000+i} +0000\ndata ${message.length}\n${message}M 100644 inline file.txt\ndata ${text.length}\n${text}\n`;}
execFileSync('git',['-C',repo,'fast-import','--quiet'],{input:stream+'done\n',env,stdio:['pipe','pipe','pipe']});git(['reset','--hard','HEAD']);
for(let i=0;i<1000;i++)writeFileSync(join(repo,`untracked-${i}.txt`),'synthetic\n');
const before=git(['rev-parse','HEAD']),state=join(root,'state.json');
const added=JSON.parse(execFileSync(bridge,[state],{input:JSON.stringify({command:'add_repository',args:{path:repo}}),encoding:'utf8',env}));if(added.error)throw Error('Fixture registration failed');
const fd=openSync(join(root,'native.log'),'a',0o600);const telemetry=join(root,'ui-soak.jsonl');
let child,spawnError,passed=false;
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
try{
 child=spawn(binary,['--native-acceptance'],{cwd:root,env:{...env,HOME:home,USERPROFILE:home,XDG_CONFIG_HOME:home,XDG_DATA_HOME:home,XDG_CACHE_HOME:home,GITNOVA_NATIVE_SMOKE:'1',GITNOVA_SMOKE_STATE:state,GITNOVA_PERF:'1',GITNOVA_NATIVE_SOAK_FOREGROUND:'1'},stdio:['ignore',fd,fd]});
 child.on('error',error=>{spawnError=error;});
 const end=Date.now()+180000;let sample;
 while(Date.now()<end){if(spawnError)throw spawnError;if(child.exitCode!==null)throw Error('Probe exited early');if(existsSync(telemetry)){sample=readFileSync(telemetry,'utf8').trim().split('\n').map(line=>JSON.parse(line)).find(row=>row.kind.startsWith('performance'));if(sample)break;}await pause(200);}
 if(sample?.kind!=='performance')throw Error('Native fixture probe failed or timed out');
 if(git(['rev-parse','HEAD'])!==before||git(['status','--porcelain','--untracked-files=no'])!=='')throw Error('Probe changed tracked Git state');
 const native=readFileSync(join(root,'native.log'),'utf8').split('\n').filter(line=>line.startsWith('GITNOVA_PERF:')).map(line=>JSON.parse(line.slice('GITNOVA_PERF:'.length)));
 writeFileSync(output,JSON.stringify({schemaVersion:1,platform:process.platform,osRelease:release(),arch:arch(),ramBytes:totalmem(),logicalCpus:cpus().length,fixture:{commits:2000,trackedFiles:1,untrackedFiles:1000},ui:sample,native,limits:['Warm native application only. Background CI/network accounts absent.','Serialization is an additional diagnostic probe, not the actual Tauri encoding timestamp.','UI timings end at matching DOM content, not physical screen presentation.']},null,2)+'\n');
 passed=true;console.log(`Native probe complete: ${sample.samples.length} UI/IPC samples, ${native.length} native timings. Fixture Git state preserved.`);
}finally{
 if(child?.pid&&child.exitCode===null){child.kill('SIGTERM');await Promise.race([once(child,'exit'),pause(3000)]);if(child.exitCode===null)child.kill('SIGKILL');}
 closeSync(fd);
 // Retain synthetic fixture/logs for reproducibility and failure inspection.
 if(!passed)console.error('Synthetic probe log retained in a temporary gitnova-native-smoke directory.');
}
