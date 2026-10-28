import type {HostProfile} from './contracts';
export interface Connection extends HostProfile {id:string;label:string;enabled:boolean;jenkins?:{controller:string;username:string}}
export const CONNECTIONS_KEY='gitnova.connections.v1';
export function normalizeHost(value:string):string {
 if(value.length>4096||/[\u0000-\u0020]/.test(value))throw new Error('Enter a credential-free HTTPS host address.');
 const url=new URL(value);
 if(url.protocol!=='https:'||!url.hostname||url.username||url.password||url.search||url.hash)throw new Error('Enter a credential-free HTTPS host address.');
 return url.href.replace(/\/+$/,'');
}
export function parseConnections(raw:string|null):Connection[]{
 if(!raw||raw.length>131072)return [];
 try{const rows=JSON.parse(raw);if(!Array.isArray(rows)||rows.length>20)return [];return rows.flatMap(v=>{
  try{if(!v||typeof v.id!=='string'||v.id.length>80||typeof v.label!=='string'||v.label.length>80||!['gitlab','github'].includes(v.provider)||typeof v.origin!=='string')return [];
   const profile:Connection={id:v.id,label:v.label,provider:v.provider,origin:normalizeHost(v.origin),enabled:v.enabled===true};
   if(v.jenkins&&typeof v.jenkins.controller==='string'&&typeof v.jenkins.username==='string'&&v.jenkins.username.length<=256)profile.jenkins={controller:normalizeHost(v.jenkins.controller),username:v.jenkins.username};return [profile];
  }catch{return [];}
 });}catch{return [];}
}
export function connections():Connection[]{
 try{
  const stored=localStorage.getItem(CONNECTIONS_KEY);if(stored!==null)return parseConnections(stored);
  // Promote old explicit GitLab host preferences, retaining original keys and Keychain entries.
  const migrated:Connection[]=[];
  for(const key of Object.keys(localStorage).filter(k=>k.startsWith('gitnova.ci.gitlab.')).slice(0,128)){
   try{const old=JSON.parse(localStorage.getItem(key)??'null');if(!old?.origin)continue;const origin=normalizeHost(old.origin);let item=migrated.find(v=>v.origin===origin);if(!item){if(migrated.length>=20)break;item={id:crypto.randomUUID(),label:new URL(origin).hostname,provider:'gitlab',origin,enabled:true};migrated.push(item);}
    const j=JSON.parse(localStorage.getItem(`gitnova.ci.jenkins.${key.slice('gitnova.ci.gitlab.'.length)}`)??'null');if(j?.controller&&j?.username&&!item.jenkins)item.jenkins={controller:normalizeHost(j.controller),username:j.username};
   }catch{/* do not guess an unknown host */}
  }
  if(migrated.length)localStorage.setItem(CONNECTIONS_KEY,JSON.stringify(migrated));return migrated;
 }catch{return [];}
}
export function saveConnections(values:Connection[]){
 if(values.length>20)throw new Error('At most 20 host connections are supported.');
 const identities=values.map(v=>`${v.provider}:${normalizeHost(v.origin)}`);
 if(new Set(identities).size!==identities.length)throw new Error('This host connection already exists.');
 localStorage.setItem(CONNECTIONS_KEY,JSON.stringify(values));window.dispatchEvent(new Event('gitnova-ci-config'));
}
