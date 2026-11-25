import {imageVersion,storedMapping,type ConfigEvidence,type Images} from './kustomize-images';
import type {Workspace} from './contracts';

const key=(workspace:string,repo:string)=>'gitnova.release.snapshot.v1.'+JSON.stringify([workspace,repo]);
function read(key:string){
  try{return JSON.parse(localStorage.getItem(key)??'null');}catch{return null;}
}
function write(key:string,value:unknown){
  try{
    if(value===null)localStorage.removeItem(key);
    else localStorage.setItem(key,JSON.stringify(value));
  }catch{/* Storage may be full. Live reads still supply the current view. */}
}
export function cachedConfig(workspace:Workspace,repo:string,envs:string[]):ConfigEvidence|undefined{
  const mapping=storedMapping(repo);
  if(!mapping)return;
  const record=read(key(workspace.id,repo));
  const deploy=workspace.memberships.find(m=>m.repositoryId===mapping.deployId);
  if(!deploy||record?.scope!==JSON.stringify([mapping,envs,deploy.id]))return;
  const images=record?.images as Images|undefined;
  if(!images||typeof images.revision!=='string'||!images.overlays||typeof images.overlays!=='object')return;
  if(!Object.values(images.overlays).every(v=>v&&typeof v.image==='string'&&(v.configUpdatedAt===null||typeof v.configUpdatedAt==='string')))return;
  return {mapping,deployName:deploy.name,images};
}
export function cacheConfig(workspace:Workspace,repo:string,envs:string[],config?:ConfigEvidence){
  const deploy=workspace.memberships.find(m=>m.repositoryId===config?.mapping.deployId);
  const valid=config?.images&&Object.values(config.images.overlays).every(v=>v.image&&!v.error);
  write(key(workspace.id,repo),valid&&deploy?{scope:JSON.stringify([config.mapping,envs,deploy.id]),images:config.images}:null);
}
export interface ReleaseEvent {source:'Deployment'|'Config updated'|'Manual confirmation';environment:string;at:string;version:string}
const eventKey=(workspace:string,repo:string)=>'gitnova.release.events.v1.'+JSON.stringify([workspace,repo]);
export function observedEvents(workspace:string,repo:string):ReleaseEvent[]{
  const list=read(eventKey(workspace,repo));
  return Array.isArray(list)?list.filter(e=>e&&['Deployment','Config updated','Manual confirmation'].includes(e.source)&&typeof e.environment==='string'&&typeof e.version==='string'&&typeof e.at==='string'&&Number.isFinite(Date.parse(e.at))).slice(0,200):[];
}
export function rememberEvents(workspace:string,repo:string,events:ReleaseEvent[]){
  if(!events.length)return;
  const old=observedEvents(workspace,repo);
  const merged=new Map(old.map(e=>[JSON.stringify(e),e]));
  for(const event of events)if(Number.isFinite(Date.parse(event.at)))merged.set(JSON.stringify(event),event);
  if(merged.size!==old.length)write(eventKey(workspace,repo),[...merged.values()].sort((a,b)=>Date.parse(b.at)-Date.parse(a.at)).slice(0,200));
}
export function configEvents(config?:ConfigEvidence):ReleaseEvent[]{
  return Object.entries(config?.images?.overlays??{}).flatMap(([environment,v])=>v.image&&v.configUpdatedAt?
    [{source:'Config updated',environment,at:v.configUpdatedAt,version:imageVersion(v.image)}]:[]);
}
