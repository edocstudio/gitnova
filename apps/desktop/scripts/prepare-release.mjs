// Assemble verified CI candidates. This never approves or publishes a release.
import {mkdirSync, readdirSync, readFileSync, copyFileSync, writeFileSync, lstatSync} from 'node:fs';
import {join, resolve, basename, sep} from 'node:path';
import {pathToFileURL} from 'node:url';
import {hashFile} from './hash-file.mjs';
function walk(folder,depth=0){
  if(depth>8)throw new Error('Candidate directory nesting exceeds limit.');
  return readdirSync(folder,{withFileTypes:true}).flatMap(entry=>{
    const path=join(folder,entry.name);
    if(entry.isSymbolicLink())throw new Error('Candidate symlinks are not accepted.');
    return entry.isDirectory()?walk(path,depth+1):[path];
  });
}
export function prepareRelease(input,output){
  input=resolve(input);output=resolve(output);
  if(output===input||output.startsWith(input+sep))throw new Error('Output must be separate from candidate input.');
  const paths=walk(input);if(paths.length>10000)throw new Error('Too many candidate files.');
  const reports=paths.filter(path=>path.endsWith('-artifact.json')).map(path=>JSON.parse(readFileSync(path,'utf8')));
  if(reports.length!==4)throw new Error('Expected macOS, Windows, Debian and AppImage evidence.');
  const first=reports[0],seen=new Set();
  if(!/^[a-f0-9]{40,64}$/.test(first.sourceCommit??'')||!/^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/.test(first.version??''))throw new Error('Exact source commit and version are required.');
  const assets=reports.map(report=>{
    const filename=report.artifact;
    if(report.kind!=='gitnova-alpha-artifact'||report.schemaVersion!==1||report.buildProfile!=='release'||report.sourceCommit!==first.sourceCommit||report.version!==first.version||!filename||basename(filename)!==filename||!/^[A-Za-z0-9._+-]+$/.test(filename)||seen.has(filename))throw new Error('Mismatched or duplicate release candidate evidence.');
    const matches=paths.filter(path=>basename(path)===filename);
    if(matches.length!==1||!lstatSync(matches[0]).isFile()||hashFile(matches[0])!==report.sha256||lstatSync(matches[0]).size!==report.bytes)throw new Error('Artifact bytes do not match candidate evidence.');
    const acceptance=report.installedAcceptance;
    if(acceptance?.passed!==true||acceptance.artifact!==filename||acceptance.artifactSha256!==report.sha256||acceptance.platform!==report.platform||acceptance.arch!==report.arch||acceptance.version!==report.version||!['embedded-ui','native-catalog-ipc','real-git-graph-ipc','single-instance','fixture-head-and-working-tree-preserved'].every(check=>acceptance.checks?.includes(check))||report.attribution?.resourcesChecked!==5||!Number.isSafeInteger(report.attribution.unresolved)||report.attribution.unresolved<0)throw new Error('Native acceptance or attribution evidence is incomplete.');
    const kind=report.platform==='darwin'&&filename.endsWith('.zip')?'macos':report.platform==='win32'&&filename.endsWith('.exe')?'windows':report.platform==='linux'&&filename.endsWith('.deb')?'deb':report.platform==='linux'&&filename.endsWith('.AppImage')?'appimage':null;
    if(!kind)throw new Error('Unexpected native package type.');
    const target={macos:{arm64:'aarch64-apple-darwin',x64:'x86_64-apple-darwin'},windows:{x64:'x86_64-pc-windows-msvc'},deb:{x64:'x86_64-unknown-linux-gnu'},appimage:{x64:'x86_64-unknown-linux-gnu'}}[kind]?.[report.arch];
    if(!target||report.target!==target)throw new Error('Candidate target does not match its platform.');
    seen.add(filename);
    return {file:filename,kind,platform:report.platform,arch:report.arch,target,bytes:report.bytes,sha256:report.sha256,signing:report.signing,licenseReview:{unresolved:report.attribution.unresolved,auditComplete:false},input:matches[0]};
  }).sort((a,b)=>a.file.localeCompare(b.file,'en'));
  if(new Set(assets.map(asset=>asset.kind)).size!==4)throw new Error('Missing native package type.');
  mkdirSync(output,{recursive:true});
  if(readdirSync(output).length)throw new Error('Release output must be empty.');
  for(const asset of assets)copyFileSync(asset.input,join(output,asset.file));
  const manifest={schemaVersion:1,kind:'gitnova-release-candidate',status:'candidate',version:first.version,sourceCommit:first.sourceCommit,buildProfile:'release',publicationGatePassed:false,assets:assets.map(({input,...asset})=>asset),blockers:['Owner review of exact artifact license/asset obligations and native manual acceptance is required.','macOS Developer ID/notarization and Windows trusted signatures are not configured.','Linux GPG signing requires the owner key; no authenticity claim is made before verification.']};
  writeFileSync(join(output,'release-candidate.json'),JSON.stringify(manifest,null,2)+'\n');
  const files=[...assets.map(asset=>asset.file),'release-candidate.json'].sort();
  writeFileSync(join(output,'SHA256SUMS'),files.map(file=>`${hashFile(join(output,file))}  ${file}\n`).join(''));
  writeFileSync(join(output,'RELEASE-NOTES.md'),`GitNova ${first.version} alpha candidate\n\nSource: ${first.sourceCommit}\n\nRelease-profile native packages; this is not publication approval.\nmacOS is ad-hoc signed, not Developer ID signed or notarized.\nWindows is unsigned. Linux GPG sidecars, when present, are detached publisher signatures, not a trusted apt repository or embedded AppImage signature.\n\nRequires installed Git; Linux also needs compatible desktop libraries and Secret Service.\nRead the candidate manifest and target-specific acceptance/license evidence before publishing.\n`);
  return manifest;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const args=process.argv.slice(2);
  if(args.length!==2)throw new Error('Usage: node prepare-release.mjs CANDIDATES EMPTY_OUTPUT');
  const result=prepareRelease(...args);console.log(`Prepared ${result.assets.length} release-profile candidates; publication remains blocked.`);
}
