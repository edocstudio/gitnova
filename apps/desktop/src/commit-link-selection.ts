import type {HostProfile,HostingSelection,RemoteBranches} from './contracts';

// Resolve only from Git's fetch destination and global host profiles. Never reuse
// old per-repository link overrides: they could point to a different project.
export function commitLinkSelection(data:RemoteBranches,upstream:string|null,oid:string,profiles:HostProfile[]):HostingSelection {
 const remote=upstream&&upstream!=='.'&&data.remotes.includes(upstream)?upstream:data.remotes.includes('origin')?'origin':data.remotes.length===1?data.remotes[0]:null;
 if(!data.remotes.length)throw new Error('No configured remotes. Add a remote from Push before copying a commit link.');
 if(!remote)throw new Error('The commit link destination is ambiguous. Configure this branch’s upstream before copying.');
 const raw=data.urls?.[remote];
 if(!raw||raw.length!==1)throw new Error('The remote needs one fetch URL before a commit link can be copied.');
 let url:URL;
 try{url=new URL(raw[0].includes('://')?raw[0]:`ssh://${raw[0].replace(':','/')}`);}catch{throw new Error('Commit links require an HTTPS or SSH Git remote.');}
 if(!['https:','ssh:'].includes(url.protocol)||url.password||url.search||url.hash||(url.protocol==='https:'&&url.username))throw new Error('Commit links require an HTTPS or SSH Git remote.');
 const matches=profiles.filter(profile=>{
  try{const host=new URL(profile.origin);const prefix=host.pathname.replace(/\/+$/,'');return host.protocol==='https:'&&!host.username&&!host.password&&!host.search&&!host.hash&&host.hostname===url.hostname&&(url.protocol==='ssh:'||(host.port===url.port&&(!prefix||url.pathname.startsWith(`${prefix}/`))));}catch{return false;}
 }).sort((a,b)=>b.origin.length-a.origin.length);
 if(matches.length>1&&(url.protocol==='ssh:'||matches[0].origin.replace(/\/+$/,'')===matches[1].origin.replace(/\/+$/,'')))throw new Error('More than one connection matches this remote. Check Connections in Settings.');
 const profile=matches[0];
 const provider=profile?.provider??(url.hostname==='github.com'?'github':url.hostname==='gitlab.com'?'gitlab':null);
 if(!provider)throw new Error('Choose this Git host’s provider in Settings → Connections, then copy again.');
 return {remote,branch:'',remoteBranch:null,oid,provider,origin:profile?.origin??`https://${url.protocol==='ssh:'?url.hostname:url.host}`};
}
