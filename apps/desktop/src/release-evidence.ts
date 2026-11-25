import {api,errorMessage} from './api';
import {connections} from './connection-store';
import type {Membership} from './contracts';

export interface Deployment {sha:string; tag:string|null; finished:string|null; id:string; url:string}
export interface ReleaseEvidence {
  production?:Deployment|null; scope?:string;
  productionError?:string; checked:number;
}
export async function readRelease(member:Membership, active:()=>boolean):Promise<ReleaseEvidence> {
  const info:ReleaseEvidence={checked:Date.now()};
  const listing=await api.remoteBranches(member.id);
  const preferred=listing.remotes.includes('origin')?'origin':listing.remotes[0];
  let oid=listing.branches.find(b=>b.remote===preferred && b.name==='release')?.oid ?? listing.branches[0]?.oid;
  const profiles=connections().filter(c=>c.enabled).map(({provider,origin})=>({provider,origin}));
  if(!oid && preferred && profiles.length && active()) oid=(await api.branches(member.id))[0]?.oid;
  const selection=oid && profiles.length && active()?await api.ciConnection(member.id,oid,profiles):null;
  const remote=selection?.remote??preferred;
  const urls=remote && listing.urls?.[remote];
  if(urls?.length) info.scope=JSON.stringify([member.repositoryId,remote,urls]);
  if(selection?.provider==='gitlab' && active()){
    try{info.production=await api.productionDeployment(member.id,selection);}
    catch(e){info.productionError=errorMessage(e);}
  }
  info.checked=Date.now();
  return info;
}
export interface Confirmation {sha:string; version:string; by:string; url:string; time:number}
export function readConfirmation(scope?:string):Confirmation|null {
  if(!scope)return null;
  try{const data=JSON.parse(localStorage.getItem('gitnova.release.confirmed.'+scope)??'null');
    return data && [40,64].includes(data.sha.length) && /^[a-f0-9]+$/.test(data.sha) && typeof data.by==='string' && typeof data.version==='string' && typeof data.url==='string' && Number.isFinite(data.time)?data:null;
  }catch{return null;}
}
