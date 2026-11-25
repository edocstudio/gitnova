import {api,errorMessage} from './api';
import type {Workspace} from './contracts';

export interface Mapping {
  deployId:string; reference:string; overlays:Record<string,string>; container:string;
}
export interface ImageResult {configUpdatedAt:string|null;image:string|null;error:string|null}
export interface Images {revision:string;overlays:Record<string,ImageResult>}
export interface ConfigEvidence {mapping:Mapping;deployName:string;images?:Images;error?:string}
const key=(repo:string)=>'gitnova.release.kustomize.'+repo;
export function storedMapping(repo:string):Mapping|null {
  try {
    const m=JSON.parse(localStorage.getItem(key(repo))??'null');
    if(!m || !['deployId','reference','container'].every(k=>typeof m[k]==='string'&&m[k].length>0))return null;
    const overlays=m.overlays??{production:m.production,uat:m.uat,sit:'overlays/sit'};
    if(!overlays||typeof overlays!=='object'||Array.isArray(overlays)||!Object.values(overlays).every(p=>typeof p==='string'))return null;
    return {deployId:m.deployId,reference:m.reference,container:m.container,overlays:Object.assign(Object.create(null),overlays)};
  } catch {return null;}
}
export const mappingChanged='gitnova-release-mapping-changed';
export function saveMapping(repo:string,m:Mapping|null){
  const prior=storedMapping(repo);
  if(!m&&!prior)return;
  if(m&&prior&&m.deployId===prior.deployId&&m.reference===prior.reference&&m.container===prior.container){
    const paths=Object.entries(m.overlays);
    if(paths.length===Object.keys(prior.overlays).length&&paths.every(([id,path])=>prior.overlays[id]===path))return;
  }
  if(m)localStorage.setItem(key(repo),JSON.stringify(m));else localStorage.removeItem(key(repo));
  window.dispatchEvent(new CustomEvent(mappingChanged,{detail:repo}));
}
export async function readImages(repo:string,workspace:Workspace,environments:string[]):Promise<ConfigEvidence|undefined>{
  const mapping=storedMapping(repo);
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
