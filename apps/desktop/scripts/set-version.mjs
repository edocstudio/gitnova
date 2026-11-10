// Synchronize first-party version metadata and current README status; never resolve dependencies.
import {readFileSync,writeFileSync} from 'node:fs';
import {dirname,resolve,join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';

export function versionUpdates(root,version){
  if(version.trim()!==version||!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version))throw new Error('Use a stable version such as 1.0.1.');
  const updates=new Map();
  const cargo=readFileSync(join(root,'Cargo.toml'),'utf8');
  if(!/\[workspace\.package\]\r?\nversion = "[^"]+"/.test(cargo))throw new Error('Missing workspace version.');
  updates.set('Cargo.toml',cargo.replace(/(\[workspace\.package\]\r?\nversion = ")[^"]+/,`$1${version}`));
  const lock=readFileSync(join(root,'Cargo.lock'),'utf8');
  updates.set('Cargo.lock',lock.replace(/(\[\[package\]\]\r?\nname = "gitnova-[^"]+"\r?\nversion = ")[^"]+/g,`$1${version}`));
  for(const file of ['apps/desktop/package.json','apps/desktop/package-lock.json','apps/desktop/src-tauri/tauri.conf.json']){
    const data=JSON.parse(readFileSync(join(root,file),'utf8'));
    data.version=version;
    if(file.endsWith('package-lock.json'))data.packages[''].version=version;
    updates.set(file,JSON.stringify(data,null,2)+'\n');
  }
  let readme=readFileSync(join(root,'README.md'),'utf8');
  const currentVersionFields=[
    /(https:\/\/img\.shields\.io\/badge\/Version-)\d+\.\d+\.\d+(?=-)/,
    /(alt="Version: )\d+\.\d+\.\d+(?=")/,
    /(The following workflows are implemented in version )\d+\.\d+\.\d+(?=\.)/,
    /(\*\*GitNova )\d+\.\d+\.\d+(?=\*\* is the owner-selected version)/,
  ];
  for(const field of currentVersionFields){
    if(!field.test(readme))throw new Error('Missing current README version field.');
    readme=readme.replace(field,(_,prefix)=>`${prefix}${version}`);
  }
  updates.set('README.md',readme);
  return updates;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  if(process.argv.length!==3)throw new Error('Usage: npm run version:set -- 1.0.1');
  const root=resolve(dirname(fileURLToPath(import.meta.url)),'../../..');
  const updates=versionUpdates(root,process.argv[2]); // Validate every input before writing.
  for(const [file,text] of updates)writeFileSync(join(root,file),text);
  console.log(`Version ${process.argv[2]} synchronized in ${updates.size} files. Regenerate license inventory before packaging. No commit, tag or publication performed.`);
}
