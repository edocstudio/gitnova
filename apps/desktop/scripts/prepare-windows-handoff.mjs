// Exact Windows candidate handoff only; never creates a tag/release or deploys a site.
import {readFileSync,writeFileSync,readdirSync,lstatSync,mkdirSync,copyFileSync} from 'node:fs';
import {join,resolve,basename,sep,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {hashFile} from './hash-file.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../../..');
const json=file=>JSON.parse(readFileSync(file,'utf8').replace(/^\uFEFF/,''));
function walk(folder,depth=0){
 if(depth>8)throw Error('Candidate nesting exceeds budget.');
 return readdirSync(folder,{withFileTypes:true}).flatMap(entry=>{
  const file=join(folder,entry.name);if(entry.isSymbolicLink())throw Error('Candidate symlinks are not accepted.');
  if(entry.isDirectory())return walk(file,depth+1);
  if(!entry.isFile())throw Error('Candidate contains a non-regular file.');return [file];
 });
}
export function prepareWindowsHandoff(input,output,{version,sourceCommit,runUrl=null}={}){
 input=resolve(input);output=resolve(output);
 if(output===input||output.startsWith(input+sep))throw Error('Use a separate handoff output directory.');
 const paths=walk(input);if(paths.length>10000)throw Error('Candidate file budget exceeded.');
 const one=name=>{const matches=paths.filter(file=>basename(file)===name);if(matches.length!==1)throw Error('Missing or duplicate '+name);return matches[0];};
 const report=json(one('windows-artifact.json')),lifecycle=json(one('windows-lifecycle.json'));
 const expectedChecks=['embedded-ui','native-catalog-ipc','real-git-graph-ipc','single-instance','fixture-head-and-working-tree-preserved'];
 if(report.schemaVersion!==1||report.kind!=='gitnova-alpha-artifact'||report.platform!=='win32'||report.arch!=='x64'||report.target!=='x86_64-pc-windows-msvc'||report.buildProfile!=='release'||report.version!==version||report.sourceCommit!==sourceCommit||!/^\d+\.\d+\.\d+$/.test(version??'')||!/^[a-f0-9]{40,64}$/.test(sourceCommit??''))throw Error('Windows candidate version/source/target mismatch.');
 const name=report.artifact;if(typeof name!=='string'||basename(name)!==name||!/^[A-Za-z0-9._+-]+\.exe$/.test(name))throw Error('Invalid installer filename.');
 const installer=one(name),checksum=hashFile(installer),bytes=lstatSync(installer).size;
 if(checksum!==report.sha256||bytes!==report.bytes)throw Error('Installer bytes do not match native evidence.');
 const acceptance=report.installedAcceptance;
 if(acceptance?.schemaVersion!==1||acceptance.passed!==true||acceptance.kind!=='gitnova-installed-acceptance'||acceptance.version!==version||acceptance.platform!=='win32'||acceptance.arch!=='x64'||acceptance.artifact!==name||acceptance.artifactSha256!==checksum||!/^[a-f0-9]{64}$/.test(acceptance.installedBinarySha256??'')||!expectedChecks.every(check=>acceptance.checks?.includes(check)))throw Error('Installed Windows acceptance is incomplete.');
 if(lifecycle.schemaVersion!==1||lifecycle.kind!=='gitnova-installer-lifecycle'||lifecycle.passed!==true||lifecycle.artifact!==name||lifecycle.artifactSha256!==checksum||!['nsis-install','installed-native-acceptance','same-version-reinstall','nsis-uninstall'].every(check=>lifecycle.checks?.includes(check)))throw Error('Windows installer lifecycle is incomplete.');
 if(report.attribution?.resourcesChecked!==5||!Number.isSafeInteger(report.attribution.unresolved)||report.attribution.unresolved<0||report.publicationGatePassed!==false||!report.signing?.startsWith('unsigned'))throw Error('Candidate attribution/signing scope mismatch.');
 const manifest={schemaVersion:1,kind:'gitnova-windows-handoff',status:'candidate',version,sourceCommit,workflowRunUrl:runUrl,publicationGatePassed:false,assets:[{file:name,platform:'Windows',arch:'x64',target:report.target,version,bytes,sha256:checksum,signing:'unsigned',url:null,checksumUrl:null}],verification:{installedAcceptance:acceptance,installerLifecycle:lifecycle,attribution:report.attribution},limitations:report.blockers??[]};
 mkdirSync(output,{recursive:true});if(readdirSync(output).length)throw Error('Handoff output must be empty.');
 copyFileSync(installer,join(output,name));
 for(const file of ['windows-artifact.json','windows-lifecycle.json'])copyFileSync(one(file),join(output,file));
 writeFileSync(join(output,'windows-handoff.json'),JSON.stringify(manifest,null,2)+'\n');
 writeFileSync(join(output,'WINDOWS-DOWNLOAD.md'),`# GitNova ${version} Windows x64 installer\n\nInstaller: ${name}\nSource: ${sourceCommit}\nSHA-256: ${checksum}\nBytes: ${bytes}\n\nBuilt on Windows with MSVC. Native installed startup/IPC, same-version reinstall and uninstall passed. All Windows jobs must pass before this handoff job runs. The executable is unsigned; keep the website's Windows publisher/SmartScreen notice. This is candidate evidence, not a completed license audit or physical GUI acceptance. Git for Windows and WebView2 runtime are required.\n\n## Handoff to gitnova-web\n\nUse windows-handoff.json for exact version, bytes, checksum and source identity. No public URL is claimed yet. Publish the reviewed installer and SHA256SUMS under a matching version tag in edocstudio/gitnova, verify anonymous downloads, then fill the real URL/checksumUrl and update the website manifest following docs/download-publication.md. Do not change the site feed to an unpublished Actions artifact URL or relabel older platform packages. The website's currently published macOS/Linux artifacts remain independent. This preparation does not publish a release, approve licensing or deploy the website.\n`);
 const files=[name,'windows-artifact.json','windows-lifecycle.json','windows-handoff.json','WINDOWS-DOWNLOAD.md'].sort();
 writeFileSync(join(output,'SHA256SUMS'),files.map(file=>`${hashFile(join(output,file))}  ${file}\n`).join(''));
 return manifest;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
 if(process.argv.length!==4)throw Error('Usage: node prepare-windows-handoff.mjs candidate-directory output-directory');
 const version=json(join(root,'apps/desktop/package.json')).version,sourceCommit=process.env.GITHUB_SHA;
 const runUrl=process.env.GITHUB_RUN_ID?`https://github.com/edocstudio/gitnova/actions/runs/${process.env.GITHUB_RUN_ID}`:null;
 const result=prepareWindowsHandoff(process.argv[2],process.argv[3],{version,sourceCommit,runUrl});console.log(`Verified Windows ${result.version} candidate: ${result.assets[0].file}; publication remains separate.`);
}
