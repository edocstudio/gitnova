import {api,errorMessage} from './api';
import type {Workspace} from './contracts';

export interface Mapping {
  deployId:string; reference:string; production:string; uat:string; container:string;
}
export interface ImageResult {image:string|null;error:string|null}
export interface Images {revision:string;production:ImageResult;uat:ImageResult}
export interface ConfigEvidence {mapping:Mapping;deployName:string;images?:Images;error?:string}
const key=(repo:string)=>'gitnova.release.kustomize.'+repo;
export function storedMapping(repo:string):Mapping|null {
  try {
    const m=JSON.parse(localStorage.getItem(key(repo))??'null');
    return m && ['deployId','reference','production','uat','container'].every(k=>typeof m[k]==='string'&&m[k].length>0)?m:null;
  } catch {return null;}
}
export function saveMapping(repo:string,m:Mapping|null){
  if(m)localStorage.setItem(key(repo),JSON.stringify(m));else localStorage.removeItem(key(repo));
  window.dispatchEvent(new Event('gitnova-ci-config'));
}
export async function readImages(repo:string,workspace:Workspace):Promise<ConfigEvidence|undefined>{
  const mapping=storedMapping(repo);
  if(!mapping)return;
  const deploy=workspace.memberships.find(m=>m.repositoryId===mapping.deployId);
  if(!deploy)return {mapping,deployName:'',error:'Add the configured deploy repository to this workspace.'};
  try{return {mapping,deployName:deploy.name,images:await api.kustomizeImages(deploy.id,mapping)};}
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
