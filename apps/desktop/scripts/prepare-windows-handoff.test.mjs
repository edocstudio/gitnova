import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,writeFileSync,readFileSync,rmSync,mkdirSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import {prepareWindowsHandoff} from './prepare-windows-handoff.mjs';
const sha=body=>createHash('sha256').update(body).digest('hex');
function fixture(t){
 const root=mkdtempSync(join(tmpdir(),'gitnova-win-handoff-'));t.after(()=>rmSync(root,{recursive:true,force:true}));const input=join(root,'input'),output=join(root,'output');mkdirSync(input);
 const body=Buffer.from('MZ synthetic installer bytes'),file='GitNova_1.3.1_x64-setup.exe',version='1.3.1',sourceCommit='a'.repeat(40),hash=sha(body);writeFileSync(join(input,file),body);
 const acceptance={schemaVersion:1,kind:'gitnova-installed-acceptance',passed:true,version,platform:'win32',arch:'x64',artifact:file,artifactSha256:hash,installedBinarySha256:'b'.repeat(64),checks:['embedded-ui','native-catalog-ipc','real-git-graph-ipc','single-instance','fixture-head-and-working-tree-preserved']};
 const report={schemaVersion:1,kind:'gitnova-alpha-artifact',platform:'win32',arch:'x64',target:'x86_64-pc-windows-msvc',buildProfile:'release',version,sourceCommit,artifact:file,bytes:body.length,sha256:hash,installedAcceptance:acceptance,attribution:{resourcesChecked:5,unresolved:67,auditComplete:false},publicationGatePassed:false,signing:'unsigned; no trusted platform signature is asserted'};
 const lifecycle={schemaVersion:1,kind:'gitnova-installer-lifecycle',passed:true,artifact:file,artifactSha256:hash,checks:['nsis-install','installed-native-acceptance','same-version-reinstall','nsis-uninstall']};
 const save=()=>{writeFileSync(join(input,'windows-artifact.json'),JSON.stringify(report));writeFileSync(join(input,'windows-lifecycle.json'),JSON.stringify(lifecycle));};save();return {input,output,file,report,lifecycle,save,options:{version,sourceCommit}};
}
test('handoff preserves exact artifact bytes, source and unsigned unpublished scope',t=>{
 const f=fixture(t),result=prepareWindowsHandoff(f.input,f.output,f.options);assert.equal(result.status,'candidate');assert.equal(result.publicationGatePassed,false);assert.equal(result.assets[0].url,null);assert.equal(sha(readFileSync(join(f.output,f.file))),f.report.sha256);assert.match(readFileSync(join(f.output,'SHA256SUMS'),'utf8'),new RegExp(f.report.sha256));
});
test('tampered installer bytes are refused',t=>{const f=fixture(t);writeFileSync(join(f.input,f.file),'tampered');assert.throws(()=>prepareWindowsHandoff(f.input,f.output,f.options),/bytes/);});
test('other version, source or platform cannot be presented as this Windows release',t=>{const f=fixture(t);for(const changes of [{version:'1.3.0'},{sourceCommit:'c'.repeat(40)},{platform:'darwin'}]){const original={...f.report};Object.assign(f.report,changes);f.save();assert.throws(()=>prepareWindowsHandoff(f.input,f.output,f.options),/mismatch/);Object.assign(f.report,original);}});
test('missing uninstall or native-IPC acceptance blocks a handoff',t=>{const f=fixture(t);f.lifecycle.checks.pop();f.save();assert.throws(()=>prepareWindowsHandoff(f.input,f.output,f.options),/lifecycle/);f.lifecycle.checks.push('nsis-uninstall');f.report.installedAcceptance.checks=[];f.save();assert.throws(()=>prepareWindowsHandoff(f.input,f.output,f.options),/acceptance/);});
test('publication/signature claims are not silently promoted',t=>{const f=fixture(t);f.report.publicationGatePassed=true;f.save();assert.throws(()=>prepareWindowsHandoff(f.input,f.output,f.options),/scope/);});
