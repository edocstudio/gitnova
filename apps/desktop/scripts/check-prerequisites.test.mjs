import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {checkPrerequisites} from './check-prerequisites.mjs';
const versions={rustc:'rustc 1.98.0',cargo:'cargo 1.98.0',git:'git version 2.51.0',xcrun:'/SDK/MacOSX.sdk'};
const inspect=(overrides={},opts={})=>checkPrerequisites({nodeVersion:'20.20.2',platform:'darwin',probe:(name)=>({status:0,stdout:versions[name],...overrides[name]}),...opts});
test('accepts installed basic tools without running builds or installers',()=>assert.ok(inspect().every(v=>v.ok)));
test('missing Cargo gives installation and PATH guidance',()=>{const row=inspect({cargo:{status:null,stdout:'',error:{code:'ENOENT'}}}).find(v=>!v.ok);assert.match(row.label,/cargo/);assert.match(row.help,/tools\/install/);assert.match(row.help,/PATH/);});
test('rejects old Rust and Cargo even when the commands exist',()=>{assert.equal(inspect({rustc:{stdout:'rustc 1.97.0'},cargo:{stdout:'cargo 1.97.0'}}).filter(v=>!v.ok).length,2);});
test('rejects unavailable SDK and bounded probe timeout',()=>{assert.equal(inspect({xcrun:{status:1,stdout:''},cargo:{error:{code:'ETIMEDOUT'},status:null,stdout:''}}).filter(v=>!v.ok).length,2);});
test('matches Node engine boundary and skips macOS SDK probes on other platforms',()=>{for(const version of ['20.19.0','22.12.0','24.0.0'])assert.ok(inspect({}, {nodeVersion:version})[0].ok);for(const version of ['20.18.0','21.7.0','22.11.0'])assert.equal(inspect({}, {nodeVersion:version})[0].ok,false);assert.equal(inspect({}, {platform:'win32'}).length,5);});
test('does not accept nightly Rust as the documented stable toolchain',()=>assert.equal(inspect({rustc:{stdout:'rustc 1.99.0-nightly'}}).find(v=>v.label.startsWith('rustc')).ok,false));

test('tauri dev stops before native CLI when Cargo and other tools are absent',()=>{
 const result=spawnSync(process.execPath,[fileURLToPath(new URL('./tauri.mjs',import.meta.url)),'dev'],{env:{...process.env,PATH:''},encoding:'utf8',timeout:15000});
 assert.equal(result.status,1);assert.match(result.stdout,/cargo: not available in PATH/);assert.match(result.stderr,/tools\/install/);assert.doesNotMatch(result.stdout,/Starting dev server|Running BeforeDevCommand/);
});

test('Linux library probes are read-only and report missing packages',()=>{
 const calls=[]; const rows=checkPrerequisites({nodeVersion:'22.12.0',platform:'linux',probe:(name,args)=>{calls.push([name,args]);return name==='pkg-config'?{status:1}:{status:0,stdout:versions[name]};}});
 assert.equal(rows.filter(r=>!r.ok).length,4);
 assert.equal(calls.filter(([name])=>name==='pkg-config').length,4);
 assert.ok(calls.every(([name])=>!['sudo','apt','curl'].includes(name)));
});

test('rejects Git without modern isolated merge-tree support',()=>{assert.equal(inspect({git:{stdout:'git version 2.34.1'}}).find(v=>v.label.startsWith('git')).ok,false);assert.ok(inspect({git:{stdout:'git version 2.38.0'}}).every(v=>v.ok));});
