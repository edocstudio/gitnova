import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,basename} from 'node:path';
import {createHash} from 'node:crypto';
import {inspectAlphaArtifact} from './alpha-artifact.mjs';
import {hashFile} from './hash-file.mjs';
const sha=data=>createHash('sha256').update(data).digest('hex');
test('artifact hash includes content beyond its bounded read buffer',()=>{
 const root=mkdtempSync(join(tmpdir(),'gitnova-hash-test-'));
 try{const data=Buffer.alloc(3*1024*1024+17,37);data[data.length-1]=91;const file=join(root,'large');writeFileSync(file,data);assert.equal(hashFile(file),sha(data));writeFileSync(file,'');assert.equal(hashFile(file),sha(''));}
 finally{rmSync(root,{recursive:true,force:true});}
});
test('candidate evidence rejects tampered installers, wrong OS, missing checks and changed notices',()=>{
 const root=mkdtempSync(join(tmpdir(),'gitnova-alpha-test-'));
 try{
  const put=(path,value)=>{mkdirSync(join(root,path,'..'),{recursive:true});writeFileSync(join(root,path),value);};
  const cargo='locked cargo',npm='locked npm';put('Cargo.lock',cargo);put('apps/desktop/package-lock.json',npm);
  const inventory={schemaVersion:1,kind:'gitnova-license-inventory',target:{darwin:{arm64:'aarch64-apple-darwin',x64:'x86_64-apple-darwin'},linux:{x64:'x86_64-unknown-linux-gnu'},win32:{x64:'x86_64-pc-windows-msvc'}}[process.platform]?.[process.arch],auditComplete:false,cargoLockSha256:sha(cargo),npmLockSha256:sha(npm),unresolved:[{component:'fixture',reason:'review required'}]};
  put('docs/licenses/dependency-inventory.json',JSON.stringify(inventory));
  const files=['LICENSE','NOTICE','terminal-components.txt','third-party-license-evidence.txt','dependency-inventory.json'];const resources={};
  for(const file of files){const source=file==='LICENSE'||file==='NOTICE'?file:`docs/licenses/${file}`;if(file!=='dependency-inventory.json')put(source,`fixture ${file}`);put(`installed/licenses/${file}`,readFileSync(join(root,source)));resources[`../../../${source}`]=`licenses/${file}`;}
  put('apps/desktop/src-tauri/tauri.conf.json',JSON.stringify({version:"0.1.0",bundle:{resources}}));
  put('candidate.exe','installer fixture');
  put('installed/app','binary fixture');
  const report={schemaVersion:1,kind:'gitnova-installed-acceptance',version:'0.1.0',passed:true,platform:process.platform,arch:process.arch,artifact:'candidate.exe',artifactSha256:sha('installer fixture'),installedBinarySha256:sha('binary fixture'),checks:['embedded-ui','native-catalog-ipc','real-git-graph-ipc','single-instance','fixture-head-and-working-tree-preserved']};
  const save=()=>put('acceptance.json',JSON.stringify(report));save();
  const options={artifact:join(root,'candidate.exe'),executable:join(root,'installed/app'),acceptance:join(root,'acceptance.json'),resources:join(root,'installed'),sourceRoot:root};
  const result=inspectAlphaArtifact(options);assert.equal(result.publicationGatePassed,false);assert.equal(result.attribution.unresolved,1);assert.equal(result.artifact,basename(options.artifact));
  put('candidate.exe','tampered');assert.throws(()=>inspectAlphaArtifact(options));put('candidate.exe','installer fixture');
  put('installed/app','changed binary');assert.throws(()=>inspectAlphaArtifact(options));put('installed/app','binary fixture');
  report.platform='other';save();assert.throws(()=>inspectAlphaArtifact(options));report.platform=process.platform;
  report.version='99.0.0';save();assert.throws(()=>inspectAlphaArtifact(options));report.version='0.1.0';save();
  report.checks.pop();save();assert.throws(()=>inspectAlphaArtifact(options));report.checks.push('fixture-head-and-working-tree-preserved');save();
  put('installed/licenses/NOTICE','changed notice');assert.throws(()=>inspectAlphaArtifact(options));
 }finally{rmSync(root,{recursive:true,force:true});}
});
