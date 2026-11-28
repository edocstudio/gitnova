export interface ReleaseEnvironment {id:string;name:string;overlay:string}
export interface ReleaseProfile {
  environments:ReleaseEnvironment[];
  production:string;
  images:boolean;
  reference:string;
  deployId:string;
}
const key=(workspace:string)=>'gitnova.release.profile.v1.'+workspace;
export const validName=(name:string)=>/^[\p{L}\p{N}][\p{L}\p{N} ._-]{0,39}$/u.test(name);
export function validProfile(p:ReleaseProfile){
  const rows=p.environments;
  return Array.isArray(rows)&&rows.length>0&&rows.length<=32&&rows.every(e=>e&&typeof e.id==='string'&&validName(e.id)&&typeof e.name==='string'&&validName(e.name)&&typeof e.overlay==='string')&&
    new Set(rows.map(e=>e.id)).size===rows.length&&new Set(rows.map(e=>e.name.toLowerCase())).size===rows.length&&
    (p.production===''||rows.some(e=>e.id===p.production))&&typeof p.images==='boolean'&&typeof p.reference==='string'&&typeof p.deployId==='string'&&
    (!p.images||(/^refs\/(remotes|heads)\/.+/.test(p.reference)&&rows.some(e=>e.overlay.trim())));
}
export function releaseProfile(workspace:string):ReleaseProfile {
  try{
    const saved=JSON.parse(localStorage.getItem(key(workspace))??'null');
    if(saved&&validProfile(saved))return saved;
  }catch{}
  let names=['SIT','UAT','Production'];
  try{
    const old=JSON.parse(localStorage.getItem('gitnova.release.environments')??'null');
    if(Array.isArray(old)&&old.length&&old.length<=32&&old.every(n=>typeof n==='string'&&validName(n))&&new Set(old.map(n=>n.toLowerCase())).size===old.length)names=old;
  }catch{}
  const environments=names.map(name=>({name,id:name.toLowerCase(),overlay:'overlays/'+name.toLowerCase()}));
  const production=['production','prd','prod'].find(id=>environments.some(e=>e.id===id))??'';
  return {environments,production,images:false,deployId:'',reference:'refs/remotes/origin/main'};
}
export function saveProfile(workspace:string,profile:ReleaseProfile){
  if(!validProfile(profile))throw new Error('Invalid release settings.');
  localStorage.setItem(key(workspace),JSON.stringify(profile));
  window.dispatchEvent(new Event('gitnova-ci-config'));
}
