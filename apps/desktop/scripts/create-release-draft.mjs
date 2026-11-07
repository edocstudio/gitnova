// Draft only: never publish, overwrite assets, move a tag or claim audit approval.
import {execFileSync} from 'node:child_process';
import {readFileSync, lstatSync, readdirSync, mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {hashFile} from './hash-file.mjs';
const repository='edocstudio/gitnova';
export function reviewDraftFiles(folder,tag){
  if(!/^v\d+\.\d+\.\d+-alpha(?:\.\d+)?$/.test(tag))throw new Error('Use an explicit versioned alpha tag.');
  const manifest=JSON.parse(readFileSync(join(folder,'release-candidate.json'),'utf8'));
  if(manifest.kind!=='gitnova-release-candidate'||manifest.status!=='candidate'||manifest.buildProfile!=='release'||manifest.publicationGatePassed!==false||!/^([a-f0-9]{40}|[a-f0-9]{64})$/.test(manifest.sourceCommit??'')||!tag.startsWith(`v${manifest.version}-alpha`)||manifest.assets?.length!==4)throw new Error('Expected exact release-profile candidate evidence.');
  const checksums=new Map();
  for(const line of readFileSync(join(folder,'SHA256SUMS'),'utf8').trimEnd().split('\n')){
    const match=/^([a-f0-9]{64})  ([A-Za-z0-9._+-]+)$/.exec(line);
    if(!match||checksums.has(match[2]))throw new Error('Invalid checksum manifest.');
    checksums.set(match[2],match[1]);
  }
  if(checksums.size!==5||!checksums.has('release-candidate.json')||new Set(manifest.assets.map(a=>a.kind)).size!==4||!['macos','windows','deb','appimage'].every(kind=>manifest.assets.some(a=>a.kind===kind)))throw new Error('Incomplete platform/checksum inventory.');
  for(const [file,sha] of checksums){
    const path=join(folder,file),stat=lstatSync(path);
    if(!stat.isFile()||stat.isSymbolicLink()||hashFile(path)!==sha)throw new Error('Candidate bytes changed.');
  }
  for(const asset of manifest.assets)if(typeof asset.file!=='string'||!/^[A-Za-z0-9._+-]+$/.test(asset.file)||!/^[a-f0-9]{64}$/.test(asset.sha256??'')||!Number.isSafeInteger(asset.bytes)||asset.bytes<=0||checksums.get(asset.file)!==asset.sha256||lstatSync(join(folder,asset.file)).size!==asset.bytes)throw new Error('Artifact evidence mismatch.');
  const files=[...checksums.keys(),'SHA256SUMS','RELEASE-NOTES.md'];
  // Absence is honest unsigned alpha. Signed candidates retain exact sidecars.
  if(readdirSync(folder).some(file=>file.endsWith('.asc')||file==='linux-signing.json')){
    const signing=JSON.parse(readFileSync(join(folder,'linux-signing.json'),'utf8'));
    const linux=manifest.assets.filter(a=>a.platform==='linux');
    if(signing.kind!=='gitnova-linux-detached-signatures'||!/^(?:[A-F0-9]{40}|[A-F0-9]{64})$/.test(signing.fingerprint??'')||linux.length!==2||signing.files?.length!==3)throw new Error('Invalid Linux signing evidence.');
    for(const file of [...linux.map(a=>a.file),'SHA256SUMS']){
      const record=signing.files.find(r=>r.file===file);
      if(record?.signature!==file+'.asc'||record.sha256!==hashFile(join(folder,file)))throw new Error('Signing evidence mismatch.');
      files.push(file+'.asc');
    }
    files.push('release-key.asc','linux-signing.json');
  }
  for(const file of files){const stat=lstatSync(join(folder,file));if(!stat.isFile()||stat.isSymbolicLink())throw new Error('Only regular release files are accepted.');}
  return {manifest,files};
}
export function createReleaseDraft(folder,tag){
  folder=resolve(folder);
  const {manifest,files}=reviewDraftFiles(folder,tag);
  if(files.includes('linux-signing.json')){
    const signing=JSON.parse(readFileSync(join(folder,'linux-signing.json'),'utf8'));
    const publicKey=readFileSync(join(folder,'release-key.asc'),'utf8');
    if(!publicKey.includes('BEGIN PGP PUBLIC KEY BLOCK')||publicKey.includes('PRIVATE KEY'))throw new Error('Only a public publisher key may be uploaded.');
    const keyring=mkdtempSync(join(process.platform==='darwin'?'/tmp':tmpdir(),'gn-'));
    const gpg=args=>execFileSync('gpg',['--homedir',keyring,'--batch','--no-tty',...args],{encoding:'utf8',timeout:30000,maxBuffer:1024*1024,stdio:['ignore','pipe','pipe']});
    try{
      gpg(['--import',join(folder,'release-key.asc')]);
      for(const record of signing.files){
        const status=gpg(['--status-fd','1','--verify',join(folder,record.signature),join(folder,record.file)]);
        const valid=status.split('\n').filter(line=>line.startsWith('[GNUPG:] VALIDSIG '));
        if(valid.length!==1||![valid[0].split(' ')[2],valid[0].split(' ').at(-1)].includes(signing.fingerprint))throw new Error('Publisher signature mismatch.');
      }
    }finally{try{execFileSync('gpgconf',['--homedir',keyring,'--kill','all'],{stdio:'ignore',timeout:5000});}finally{rmSync(keyring,{recursive:true,force:true});}}
  }
  const gh=args=>execFileSync('gh',args,{encoding:'utf8',timeout:60000,maxBuffer:2*1024*1024,stdio:['ignore','pipe','pipe']});
  let object=JSON.parse(gh(['api',`repos/${repository}/git/ref/tags/${tag}`])).object;
  for(let depth=0;object?.type==='tag'&&depth<4;depth++)object=JSON.parse(gh(['api',`repos/${repository}/git/tags/${object.sha}`])).object;
  if(object?.type!=='commit'||object.sha!==manifest.sourceCommit)throw new Error('Existing tag must resolve to the tested candidate commit; no tags are created or moved.');
  // gh refuses an existing release. No --clobber or public-release mutation.
  const url=gh(['release','create',tag,'--repo',repository,'--verify-tag','--draft','--prerelease','--title',`GitNova ${tag} — alpha`,'--notes-file',join(folder,'RELEASE-NOTES.md'),...files.map(file=>join(folder,file))]);
  console.log(url.trim());
  console.log('Draft only. License/native acceptance review remains required before public distribution.');
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  if(process.argv.length!==4)throw new Error('Usage: node create-release-draft.mjs RELEASE_DIRECTORY ALPHA_TAG');
  createReleaseDraft(...process.argv.slice(2));
}
