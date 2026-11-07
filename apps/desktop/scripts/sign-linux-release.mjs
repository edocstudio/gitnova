// Detached GPG signatures, not embedded AppImage signatures or an apt repository.
import {execFileSync} from 'node:child_process';
import {readFileSync, writeFileSync, existsSync, realpathSync, lstatSync} from 'node:fs';
import {join, resolve, basename, sep} from 'node:path';
import {pathToFileURL} from 'node:url';
import {hashFile} from './hash-file.mjs';
export function signLinuxRelease(folder,homedir,fingerprint,passphrase=''){
  folder=realpathSync(folder);homedir=realpathSync(homedir);
  if(homedir===folder||homedir.startsWith(folder+sep)||!/^(?:[A-Fa-f0-9]{40}|[A-Fa-f0-9]{64})$/.test(fingerprint))throw new Error('Use an isolated GPG home outside release assets and a full publisher fingerprint.');
  const gpg=(args,input)=>{
    try{return execFileSync('gpg',['--homedir',homedir,'--batch','--no-tty',...args],{input,encoding:'utf8',timeout:30000,maxBuffer:1024*1024,stdio:['pipe','pipe','pipe']});}
    catch{throw new Error('GPG signing/verification failed; no verified publisher signature is claimed.');}
  };
  const manifest=JSON.parse(readFileSync(join(folder,'release-candidate.json'),'utf8'));
  if(manifest.kind!=='gitnova-release-candidate'||manifest.status!=='candidate'||manifest.buildProfile!=='release')throw new Error('Expected release candidate manifest.');
  const linux=manifest.assets.filter(asset=>asset.platform==='linux');
  if(linux.length!==2||!linux.some(asset=>asset.kind==='deb')||!linux.some(asset=>asset.kind==='appimage'))throw new Error('Expected Debian and AppImage candidates.');
  const sumLines=readFileSync(join(folder,'SHA256SUMS'),'utf8').trimEnd().split('\n');
  if(sumLines.length!==manifest.assets.length+1)throw new Error('Unexpected checksum inventory.');
  const checksums=new Map();
  for(const line of sumLines){const match=/^([a-f0-9]{64})  ([A-Za-z0-9._+-]+)$/.exec(line);if(!match||checksums.has(match[2])||!lstatSync(join(folder,match[2])).isFile()||lstatSync(join(folder,match[2])).isSymbolicLink()||hashFile(join(folder,match[2]))!==match[1])throw new Error('Candidate checksum mismatch.');checksums.set(match[2],match[1]);}
  if(!checksums.has('release-candidate.json'))throw new Error('Manifest checksum missing.');
  for(const asset of manifest.assets)if(basename(asset.file)!==asset.file||checksums.get(asset.file)!==asset.sha256)throw new Error('Artifact checksum missing.');
  const files=[...linux.map(asset=>asset.file),'SHA256SUMS'];
  if(files.some(file=>existsSync(join(folder,file+'.asc')))||existsSync(join(folder,'release-key.asc')))throw new Error('Signature output already exists.');
  for(const file of files){
    const path=join(folder,file),signature=path+'.asc';
    gpg(['--pinentry-mode','loopback','--passphrase-fd','0','--local-user',fingerprint,'--armor','--detach-sign','--output',signature,path],passphrase+'\n');
    const status=gpg(['--status-fd','1','--verify',signature,path]);
    const valid=status.split('\n').filter(line=>line.startsWith('[GNUPG:] VALIDSIG '));
    if(valid.length!==1||![valid[0].split(' ')[2],valid[0].split(' ').at(-1)].some(field=>field?.toUpperCase()===fingerprint.toUpperCase()))throw new Error('Signature does not match the expected publisher key.');
  }
  const publicKey=gpg(['--armor','--export',fingerprint]);
  if(!publicKey.includes('BEGIN PGP PUBLIC KEY BLOCK')||publicKey.includes('PRIVATE KEY'))throw new Error('Expected a public verification key.');
  writeFileSync(join(folder,'release-key.asc'),publicKey);
  writeFileSync(join(folder,'linux-signing.json'),JSON.stringify({schemaVersion:1,kind:'gitnova-linux-detached-signatures',fingerprint:fingerprint.toUpperCase(),files:files.map(file=>({file,sha256:hashFile(join(folder,file)),signature:file+'.asc'})),scope:'Detached GPG signatures only; no embedded AppImage signature or signed apt repository.',publicationGatePassed:false},null,2)+'\n');
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  if(process.argv.length!==5)throw new Error('Usage: node sign-linux-release.mjs RELEASE_DIRECTORY ISOLATED_GPG_HOME FINGERPRINT');
  signLinuxRelease(...process.argv.slice(2),process.env.LINUX_GPG_PASSPHRASE??'');
  console.log('Linux detached signatures verified against the pinned publisher fingerprint.');
}
