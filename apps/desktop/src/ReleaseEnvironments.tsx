import {useState} from 'react';
import {t} from './i18n';
import {releaseProfile,saveProfile,validProfile,type ReleaseEnvironment} from './release-profile';
import type {Workspace} from './contracts';

export function ReleaseEnvironments({workspace}:{workspace:Workspace}){
  const [draft,setDraft]=useState(()=>releaseProfile(workspace.id));
  const [message,setMessage]=useState('');
  const rows=draft.environments;
  function update(rows:ReleaseEnvironment[]){
    setMessage('');
    setDraft({...draft,environments:rows,production:rows.some(e=>e.id===draft.production)?draft.production:''});
  }
  function move(index:number,offset:number){
    const next=[...rows];
    [next[index],next[index+offset]]=[next[index+offset],next[index]];
    update(next);
  }
  return <section className="settings-general"><h3 tabIndex={-1} id="workspace-release-settings">{t('Releases')} · {workspace.name}</h3>
    <form className="kube-settings" onChange={()=>setMessage('')} onSubmit={e=>{e.preventDefault();try{saveProfile(workspace.id,draft);setMessage('Saved');}catch{setMessage('Could not save release settings.');}}}>
    <fieldset><legend>{t('Visible environments')}</legend>
    {rows.map((env,i)=><div className="release-env-editor" key={env.id}>
      <input aria-label={t('Environment {0}',[i+1])} maxLength={40} value={env.name} onChange={e=>update(rows.map(r=>r.id===env.id?{...r,name:e.target.value}:r))}/>
      <button type="button" aria-label={t('Move {0} up',[env.name])} disabled={!i} onClick={()=>move(i,-1)}>↑</button>
      <button type="button" aria-label={t('Move {0} down',[env.name])} disabled={i===rows.length-1} onClick={()=>move(i,1)}>↓</button>
      <button type="button" aria-label={t('Remove {0}',[env.name])} disabled={rows.length===1} onClick={()=>update(rows.filter(r=>r.id!==env.id))}>×</button>
    </div>)}
    <button type="button" disabled={rows.length>=32} onClick={()=>update([...rows,{id:crypto.randomUUID(),name:'',overlay:''}])}>{t('Add environment')}</button>
    </fieldset>
    <label>{t('GitLab production deployments')}<select value={draft.production} onChange={e=>setDraft({...draft,production:e.target.value})}>
      <option value="">{t('Off')}</option>{rows.map(r=><option key={r.id} value={r.id}>{r.name}</option>)}</select></label>
    <label>{t('Default release source')}<select value={draft.images?'images':'manual'} onChange={e=>setDraft({...draft,images:e.target.value==='images'})}>
      <option value="manual">{t('Manual confirmation')}</option><option value="images">Kustomize</option></select></label>
    {draft.images&&<>
      <label>{t('Deploy repository')}<select value={draft.deployId} onChange={e=>setDraft({...draft,deployId:e.target.value})}>
        <option value="">{t('Each application repository')}</option>{workspace.memberships.map(m=><option key={m.id} value={m.repositoryId}>{m.name}</option>)}</select></label>
      <label>{t('Deploy branch')}<input value={draft.reference} placeholder="refs/remotes/origin/main" onChange={e=>setDraft({...draft,reference:e.target.value.trim()})}/></label>
      {rows.map(r=><label key={r.id}>{t('{0} overlay',[r.name])}<input value={r.overlay} placeholder={'overlays/'+r.name.toLowerCase()} onChange={e=>update(rows.map(row=>row.id===r.id?{...row,overlay:e.target.value.trim()}:row))}/></label>)}
      <small>{t('The container defaults to the application repository name. Override it in Releases for individual repositories.')}</small>
    </>}
    <button className="primary" disabled={!validProfile(draft)}>{t('Save environments')}</button>
    {message&&<p role="status">{t(message)}</p>}
    </form></section>;
}
