import {api,errorMessage} from './api';
import {releaseProfile} from './release-profile';
import type {Workspace} from './contracts';

export interface Mapping {
  deployId:string; reference:string; overlays:Record<string,string>; container:string;
}
export interface ImageResult {configUpdatedAt:string|null;image:string|null;error:string|null}
export interface Images {revision:string;overlays:Record<string,ImageResult>}
export interface ConfigEvidence {mapping:Mapping;deployName:string;images?:Images;error?:string}
const key=(repo:string)=>'gitnova.release.kustomize.'+repo;
const scopedKey=(workspace:string,repo:string)=>'gitnova.release.mapping.v2.'+JSON.stringify([workspace,repo]);
function parseMapping(raw:string|null):Mapping|null {
  try {
    const m=JSON.parse(raw??'null');
    if(!m || !['deployId','reference','container'].every(k=>typeof m[k]==='string'&&m[k].length>0))return null;
    const overlays=m.overlays??{production:m.production,uat:m.uat,sit:'overlays/sit'};
    if(!overlays||typeof overlays!=='object'||Array.isArray(overlays)||!Object.values(overlays).every(p=>typeof p==='string'))return null;
    return {deployId:m.deployId,reference:m.reference,container:m.container,overlays:Object.assign(Object.create(null),overlays)};
  } catch {return null;}
}
export function storedMapping(repo:string):Mapping|null{return parseMapping(localStorage.getItem(key(repo)));}
export function mappingMode(workspace:string,repo:string){
  const raw=localStorage.getItem(scopedKey(workspace,repo));
  if(raw==='manual'||raw==='inherit')return raw;
  return (raw?parseMapping(raw):storedMapping(repo))?'custom':'inherit';
}
function defaultMapping(workspace:Workspace,repo:string):Mapping|null{
  const profile=releaseProfile(workspace.id);
  const app=workspace.memberships.find(m=>m.repositoryId===repo);
  if(!profile.images||!app)return null;
  return {deployId:profile.deployId||repo,reference:profile.reference,container:app.name,
    overlays:Object.fromEntries(profile.environments.map(e=>[e.id,e.overlay]))};
}
export function effectiveMapping(workspace:Workspace,repo:string):Mapping|null{
  const raw=localStorage.getItem(scopedKey(workspace.id,repo));
  if(raw==='manual')return null;
  const custom=raw===null?storedMapping(repo):parseMapping(raw);
  if(!custom)return defaultMapping(workspace,repo);
  if(!raw)return custom;
  const saved=JSON.parse(raw);
  if(!saved.inheritDefaults)return custom;
  const defaults=defaultMapping(workspace,repo);
  if(!defaults||!Array.isArray(saved.overlayOverrides))return custom;
  const overrides=saved.overlayOverrides.filter((id:unknown)=>typeof id==='string'&&Object.hasOwn(custom.overlays,id)) as string[];
  return {deployId:saved.inheritDeploy?defaults.deployId:custom.deployId,
    reference:saved.inheritRef?defaults.reference:custom.reference,
    container:saved.inheritContainer?defaults.container:custom.container,
    overlays:{...defaults.overlays,...Object.fromEntries(overrides.map(id=>[id,custom.overlays[id]]))}};
}
export const mappingChanged='gitnova-release-mapping-changed';
export function saveMapping(workspace:Workspace,repo:string,value:Mapping|'inherit'|'manual'){
  const mode=mappingMode(workspace.id,repo);
  if(typeof value==='string'? mode===value : mode==='custom'&&JSON.stringify(effectiveMapping(workspace,repo))===JSON.stringify(value))return;
  let serialized:string;
  if(typeof value==='string')serialized=value;
  else{
    const defaults=defaultMapping(workspace,repo);
    serialized=JSON.stringify(defaults?{...value,inheritDefaults:true,
      inheritDeploy:value.deployId===defaults.deployId,inheritRef:value.reference===defaults.reference,
      inheritContainer:value.container===defaults.container,
      overlayOverrides:Object.keys(value.overlays).filter(id=>value.overlays[id]!==defaults.overlays[id])}:value);
  }
  localStorage.setItem(scopedKey(workspace.id,repo),serialized);
  window.dispatchEvent(new CustomEvent(mappingChanged,{detail:repo}));
}
export async function readImages(repo:string,workspace:Workspace,environments:string[]):Promise<ConfigEvidence|undefined>{
  const mapping=effectiveMapping(workspace,repo);
  if(!mapping)return;
  const deploy=workspace.memberships.find(m=>m.repositoryId===mapping.deployId);
  if(!deploy)return {mapping,deployName:'',error:'Add the configured deploy repository to this workspace.'};
  const overlays=Object.fromEntries(environments.map(name=>name.toLowerCase()).filter(id=>Object.hasOwn(mapping.overlays,id)&&mapping.overlays[id]).map(id=>[id,mapping.overlays[id]]));
  if(!Object.keys(overlays).length)return {mapping,deployName:deploy.name};
  try{return {mapping,deployName:deploy.name,images:await api.kustomizeImages(deploy.id,{...mapping,overlays})};}
  catch(e){return {mapping,deployName:deploy.name,error:errorMessage(e)};}
}
export function imageVersion(image:string){
  const at=image.indexOf('@');
  const name=at<0?image:image.slice(0,at);
  const colon=name.lastIndexOf(':');
  const tag=colon>name.lastIndexOf('/')?name.slice(colon+1):'';
  if(at>=0){const digest=image.slice(at+1);return (tag?tag+' @ ':'')+digest.slice(0,19)+(digest.length>19?'…':'');}
  return tag||image;
}
