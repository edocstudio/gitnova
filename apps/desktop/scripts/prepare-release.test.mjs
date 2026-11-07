import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync, execFileSync} from 'node:child_process';
import {prepareRelease} from './prepare-release.mjs';
import {signLinuxRelease} from './sign-linux-release.mjs';
import {hashFile} from './hash-file.mjs';
import {reviewDraftFiles} from './create-release-draft.mjs';
function fixture(){
 const root=mkdtempSync(join(tmpdir(),'gitnova-release-test-')),input=join(root,'input'),output=join(root,'output');mkdirSync(input);
 const reports=[];
 for(const [platform,arch,file,target] of [['darwin','arm64','GitNova.zip','aarch64-apple-darwin'],['win32','x64','GitNova.exe','x86_64-pc-windows-msvc'],['linux','x64','GitNova.deb','x86_64-unknown-linux-gnu'],['linux','x64','GitNova.AppImage','x86_64-unknown-linux-gnu']]){
  writeFileSync(join(input,file),'fixture installer');const sha256=hashFile(join(input,file));
  const report={schemaVersion:1,kind:'gitnova-alpha-artifact',sourceCommit:'a'.repeat(40),version:'0.1.0',buildProfile:'release',artifact:file,platform,arch,target,sha256,bytes:17,attribution:{resourcesChecked:5,unresolved:7},signing:'unsigned-alpha',installedAcceptance:{passed:true,version:'0.1.0',platform,arch,artifact:file,artifactSha256:sha256,checks:['embedded-ui','native-catalog-ipc','real-git-graph-ipc','single-instance','fixture-head-and-working-tree-preserved']}};
  const path=join(input,file+'-artifact.json');writeFileSync(path,JSON.stringify(report));reports.push({report,path});
 }
 return {root,input,output,reports,clean:()=>rmSync(root,{recursive:true,force:true})};
}
test('release assembly preserves unresolved license evidence and never approves publication',()=>{
 const f=fixture();try{const manifest=prepareRelease(f.input,f.output);assert.equal(manifest.assets.length,4);assert.equal(manifest.publicationGatePassed,false);assert.ok(manifest.assets.every(asset=>asset.licenseReview.unresolved===7));assert.equal(readFileSync(join(f.output,'SHA256SUMS'),'utf8').trim().split('\n').length,5);assert.throws(()=>prepareRelease(f.input,f.output),/empty/);}finally{f.clean();}
});
test('release assembly refuses tampering, debug builds, mixed commits and missing native evidence',()=>{
 for(const mutate of [f=>writeFileSync(join(f.input,'GitNova.zip'),'changed'),f=>{f.reports[0].report.buildProfile='debug';},f=>{f.reports[0].report.sourceCommit='b'.repeat(40);},f=>{f.reports[0].report.installedAcceptance.checks=[];},f=>{f.reports[0].report.target='x86_64-pc-windows-msvc';}]){
  const f=fixture();try{mutate(f);for(const {path,report} of f.reports)writeFileSync(path,JSON.stringify(report));assert.throws(()=>prepareRelease(f.input,f.output));}finally{f.clean();}
 }
});
test('signing rejects an output-contained keyring or abbreviated key identity before calling GPG',()=>{
 const f=fixture();try{prepareRelease(f.input,f.output);assert.throws(()=>signLinuxRelease(f.output,f.output,'a'.repeat(40)));assert.throws(()=>signLinuxRelease(f.output,f.root,'short-key'));}finally{f.clean();}
});
test('draft upload validates all platform bytes and accepts only matching alpha tags',()=>{
 const f=fixture();try{
  prepareRelease(f.input,f.output);
  assert.equal(reviewDraftFiles(f.output,'v0.1.0-alpha.1').files.length,7);
  for(const tag of ['v0.2.0-alpha.1','v0.1.0','--draft','v0.1.0-alpha.1/other'])assert.throws(()=>reviewDraftFiles(f.output,tag));
  writeFileSync(join(f.output,'GitNova.exe'),'changed');assert.throws(()=>reviewDraftFiles(f.output,'v0.1.0-alpha.1'),/changed/);
 }finally{f.clean();}
});
test('ephemeral GPG signs exact Linux bytes and checksum manifest', {skip:spawnSync('gpg',['--version']).status!==0},()=>{
 // macOS TMPDIR paths can exceed the Unix-domain socket limit for gpg-agent.
 // A separate short, private directory keeps the real signing test isolated.
 const f=fixture(),keyring=mkdtempSync(join(process.platform==='darwin'?'/tmp':tmpdir(),'gn-'));
 try{
  prepareRelease(f.input,f.output);
  const args=['--homedir',keyring,'--batch','--pinentry-mode','loopback','--passphrase',''];
  execFileSync('gpg',[...args,'--quick-generate-key','GitNova Disposable Test <fixture@example.invalid>','ed25519','cert','1d'],{stdio:'pipe'});
  const fingerprint=execFileSync('gpg',[...args,'--with-colons','--list-keys'],{encoding:'utf8',stdio:['pipe','pipe','pipe']}).split('\n').find(line=>line.startsWith('fpr:')).split(':')[9];
  execFileSync('gpg',[...args,'--quick-add-key',fingerprint,'ed25519','sign','1d'],{stdio:'pipe'});
  signLinuxRelease(f.output,keyring,fingerprint);
  assert.match(readFileSync(join(f.output,'release-key.asc'),'utf8'),/PUBLIC KEY BLOCK/);
  assert.equal(JSON.parse(readFileSync(join(f.output,'linux-signing.json'),'utf8')).files.length,3);
  writeFileSync(join(f.output,'GitNova.deb'),'tampered');
  assert.notEqual(spawnSync('gpg',[...args,'--verify',join(f.output,'GitNova.deb.asc'),join(f.output,'GitNova.deb')]).status,0);
 }finally{spawnSync('gpgconf',['--homedir',keyring,'--kill','all']);rmSync(keyring,{recursive:true,force:true});f.clean();}
});
