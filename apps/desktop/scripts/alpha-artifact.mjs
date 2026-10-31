// Candidate evidence, never a publication/signing or legal-compliance approval.
import {readFileSync,writeFileSync,statSync} from 'node:fs';
import {hashFile as hash} from './hash-file.mjs';
import {basename,dirname,join,resolve} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {verifyLicenseInventory,verifyResourceDirectory} from './verify-bundle-licenses.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../../..');
export function inspectAlphaArtifact({artifact,executable,acceptance,resources,sourceRoot,platform=process.platform,arch=process.arch}) {
  const report=JSON.parse(readFileSync(acceptance,'utf8'));
  const expected=['embedded-ui','native-catalog-ipc','real-git-graph-ipc','single-instance','fixture-head-and-working-tree-preserved'];
  if(!statSync(artifact).isFile()||report.schemaVersion!==1||report.kind!=='gitnova-installed-acceptance'||report.passed!==true
    ||report.platform!==platform||report.arch!==arch||report.artifact!==basename(artifact)||report.artifactSha256!==hash(artifact)
    ||!expected.every(check=>report.checks?.includes(check))||!(/^[a-f0-9]{64}$/).test(report.installedBinarySha256??'')
    ||!statSync(executable).isFile()||hash(executable)!==report.installedBinarySha256)throw new Error('Artifact does not match successful native installed acceptance evidence.');
  const licenses=verifyResourceDirectory(resources,sourceRoot);
  const inventory=verifyLicenseInventory(sourceRoot);
  const target={darwin:{arm64:'aarch64-apple-darwin',x64:'x86_64-apple-darwin'},linux:{x64:'x86_64-unknown-linux-gnu'},win32:{x64:'x86_64-pc-windows-msvc'}}[platform]?.[arch];
  if(!target||inventory.target!==target)throw new Error('License evidence belongs to another native target.');
  const config=JSON.parse(readFileSync(join(sourceRoot??root,'apps/desktop/src-tauri/tauri.conf.json'),'utf8'));
  if(!report.version||report.version!==config.version)throw new Error('Installed app version does not match the packaged source configuration.');
  return {schemaVersion:1,kind:'gitnova-alpha-artifact',version:report.version,sourceCommit:/^[a-f0-9]{40,64}$/.test(process.env.GITHUB_SHA??'')?process.env.GITHUB_SHA:null,target:inventory.target,lockfiles:{cargo:inventory.cargoLockSha256,npm:inventory.npmLockSha256},artifact:basename(artifact),bytes:statSync(artifact).size,sha256:report.artifactSha256,platform,arch,
    installedAcceptance:report,attribution:licenses,signing:'unsigned-alpha; no trusted platform signature is asserted',publicationGatePassed:false,
    blockers:['Native manual GUI acceptance for this exact artifact remains required.','Trusted signing/notarization is not configured.','Target-specific shipped dependency/asset license review remains required; bundled evidence is not an audit.'],
    limits:['Hashes provide integrity, not publisher authenticity.','This manifest does not upload, install, sign or publish anything.']};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const args=process.argv.slice(2),options={};
  for(let i=0;i<args.length;i+=2){if(!['--artifact','--executable','--acceptance','--resources','--output'].includes(args[i])||!args[i+1]||options[args[i]])throw new Error('Unexpected candidate manifest argument.');options[args[i]]=args[i+1];}
  if(Object.keys(options).length!==5)throw new Error('Pass --artifact, --executable, --acceptance, --resources and --output.');
  const report=inspectAlphaArtifact({artifact:resolve(options['--artifact']),executable:resolve(options['--executable']),acceptance:resolve(options['--acceptance']),resources:resolve(options['--resources'])});
  writeFileSync(resolve(options['--output']),JSON.stringify(report,null,2)+'\n');
  console.log(`Verified unsigned alpha evidence: ${report.artifact}; publication gate remains blocked.`);
}
