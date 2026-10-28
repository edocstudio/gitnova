import {useEffect,useState} from 'react';
import {api} from './api';
import {connections} from './connection-store';
import {readCi} from './ci-requests';
import type {HostingSelection,PipelineSnapshot} from './contracts';

const cache=new Map<string,PipelineSnapshot>();
const active=(v:PipelineSnapshot)=>v.runs.some(r=>['running','pending','created','preparing','waiting_for_resource'].includes(r.status));
export type GraphCiValue={status:string;checkedAt?:number;stale?:boolean};
export function useGraphCi(id:string,oids:string):Record<string,GraphCiValue>{
 const [version,setVersion]=useState(0),[result,setResult]=useState<Record<string,GraphCiValue>>({});
 useEffect(()=>{const change=()=>{cache.clear();setVersion(n=>n+1);};window.addEventListener('gitnova-ci-config',change);return()=>window.removeEventListener('gitnova-ci-config',change);},[]);
 useEffect(()=>{
  const profiles=connections().filter(v=>v.enabled).map(({provider,origin})=>({provider,origin}));
  setResult({});if(!profiles.length)return;
  const commits=oids.split(',').filter(Boolean).slice(0,40);if(!commits.length)return;
  let config:HostingSelection|null=null;let scope='';
  let live=true,reading=false;let timer:ReturnType<typeof setTimeout>|undefined;
  const publish=(oid:string,v:PipelineSnapshot,stale=false)=>{if(live)setResult(old=>({...old,[oid]:{status:v.runs[0]?.status??'No pipeline',checkedAt:v.checkedAt,stale}}));};
  const read=async()=>{
   if(!live||reading||document.hidden)return;reading=true;let retry=false;let failures=0;
   try{
    const resolved=await api.ciConnection(id,commits[0],profiles);
    if(!live||document.hidden)return;
    if(!resolved){setResult({});return;}
    config=resolved;const nextScope=JSON.stringify([id,config.remote,config.provider,config.origin]);if(nextScope!==scope)setResult({});scope=nextScope;
    for(const oid of commits){const v=cache.get(`${scope}:${oid}`);if(v)publish(oid,v);}
    for(const oid of commits){
    if(!live||document.hidden)break;
    const key=`${scope}:${oid}`,old=cache.get(key),ttl=old&&active(old)?15:60;
    if(old&&Date.now()/1000-old.checkedAt<ttl){publish(oid,old);continue;}
    if(!old)setResult(v=>({...v,[oid]:{status:'Reading CI…'}}));
    try{
     const value=await readCi(`summary:${key}`,()=>live&&!document.hidden,()=>api.ciSummary(id,{...config!,oid}));
     if(value.oid!==oid)throw new Error('CI scope changed');
     if(!live)return;
     cache.delete(key);while(cache.size>=128)cache.delete(cache.keys().next().value!);cache.set(key,value);publish(oid,value);failures=0;
    }catch{
     if(live&&!document.hidden){if(old)publish(oid,old,true);else setResult(v=>({...v,[oid]:{status:'CI unavailable'} }));retry=true;}
     if(++failures>=3)break; // Bound repeated auth failures; skip an unpublished local tip.
    }
   }}catch{if(live&&!document.hidden){setResult(Object.fromEntries(commits.map(oid=>[oid,{status:'CI unavailable'}])));retry=true;}}finally{
    reading=false;
    if(live&&!document.hidden){const polling=commits.some(oid=>{const v=cache.get(`${scope}:${oid}`);return v&&(active(v)||!v.runs.length);});if(commits.length)timer=setTimeout(()=>void read(),!retry&&polling?20000:60000);}
   }
  };
  const visibility=()=>{clearTimeout(timer);if(!document.hidden)timer=setTimeout(()=>void read(),1000);};
  timer=setTimeout(()=>void read(),350);document.addEventListener('visibilitychange',visibility);
  return()=>{live=false;clearTimeout(timer);document.removeEventListener('visibilitychange',visibility);};
 },[id,oids,version]);
 return result;
}
