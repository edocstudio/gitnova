import {test} from 'node:test';
import {strict as assert} from 'node:assert';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {versionUpdates} from './set-version.mjs';
test('version updates preserve third-party locks and do not write during preparation',()=>{
  const root=mkdtempSync(join(tmpdir(),'gitnova-version-'));
  try{
    mkdirSync(join(root,'apps/desktop/src-tauri'),{recursive:true});
    writeFileSync(join(root,'Cargo.toml'),'[workspace.package]\nversion = "1.0.0"\n');
    const lock='[[package]]\nname = "gitnova-core"\nversion = "1.0.0"\n\n[[package]]\nname = "serde"\nversion = "1.0.0"\n';
    writeFileSync(join(root,'Cargo.lock'),lock);
    const readme='https://img.shields.io/badge/Version-1.0.0-blue alt="Version: 1.0.0"\nThe following workflows are implemented in version 1.0.0.\n**GitNova 1.0.0** is the owner-selected version\nHistorical release: 1.0.0\n';
    writeFileSync(join(root,'README.md'),readme);
    for(const file of ['apps/desktop/package.json','apps/desktop/package-lock.json','apps/desktop/src-tauri/tauri.conf.json'])writeFileSync(join(root,file),JSON.stringify({version:'1.0.0',packages:{'':{version:'1.0.0'},'node_modules/example':{version:'1.0.0'}}}));
    const updates=versionUpdates(root,'1.0.1');assert.equal(updates.size,6);
    assert.equal(updates.get('README.md'),readme.replaceAll('1.0.0','1.0.1').replace('Historical release: 1.0.1','Historical release: 1.0.0'));
    assert.equal(readFileSync(join(root,'README.md'),'utf8'),readme);
    assert.match(updates.get('Cargo.lock'),/name = "gitnova-core"\nversion = "1.0.1"/);
    assert.match(updates.get('Cargo.lock'),/name = "serde"\nversion = "1.0.0"/);
    const npm=JSON.parse(updates.get('apps/desktop/package-lock.json'));
    assert.equal(npm.packages[''].version,'1.0.1');assert.equal(npm.packages['node_modules/example'].version,'1.0.0');
    assert.equal(readFileSync(join(root,'Cargo.lock'),'utf8'),lock);
    for(const invalid of ['01.0.1','v1.0.1','1.0','1.0.1\n','1.0.1-alpha'])assert.throws(()=>versionUpdates(root,invalid));
    writeFileSync(join(root,'README.md'),'Missing version fields');
    assert.throws(()=>versionUpdates(root,'1.0.2'),/Missing current README version field/);
    assert.equal(readFileSync(join(root,'Cargo.lock'),'utf8'),lock);
  }finally{rmSync(root,{recursive:true,force:true});}
});
