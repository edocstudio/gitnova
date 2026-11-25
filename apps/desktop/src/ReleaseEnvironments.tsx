import {useState} from 'react';
import {t} from './i18n';

const key='gitnova.release.environments';
const defaults=['SIT','UAT','Production'];
function valid(names:unknown):names is string[]{
  return Array.isArray(names)&&names.length>0&&names.length<=32&&
    names.every(n=>typeof n==='string'&&/^[\p{L}\p{N}][\p{L}\p{N} ._-]{0,39}$/u.test(n))&&
    new Set(names.map(n=>n.toLowerCase())).size===names.length;
}
export function releaseEnvironments():string[]{
  try{const names=JSON.parse(localStorage.getItem(key)??'null');if(valid(names))return names;}catch{}
  return defaults;
}
export function ReleaseEnvironments(){
  const [draft,setDraft]=useState(()=>releaseEnvironments().join(', '));
  const [saved,setSaved]=useState(false);
  const names=draft.split(',').map(n=>n.trim());
  return <section className="settings-general"><h3>{t('Releases')}</h3><form onSubmit={e=>{
    e.preventDefault();if(!valid(names))return;
    localStorage.setItem(key,JSON.stringify(names));setSaved(true);
    window.dispatchEvent(new Event('gitnova-ci-config'));
  }}>
    <label>{t('Visible environments')}<input value={draft} onChange={e=>{setDraft(e.target.value);setSaved(false);}}/></label>
    <p className="hint">{t('Separate names with commas, in the order you want across all repositories.')}</p>
    {!valid(names)&&<p role="status">{t('Enter 1–32 unique names using letters, numbers, spaces, dots, underscores or hyphens (40 characters each).')}</p>}
    <button disabled={!valid(names)}>{t('Save environments')}</button>
    {saved&&<p role="status">{t('Saved')}</p>}
  </form></section>;
}
