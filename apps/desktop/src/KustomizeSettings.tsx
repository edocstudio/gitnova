import {useEffect,useState} from 'react';
import {api,errorMessage} from './api';
import {t} from './i18n';
import type {Membership} from './contracts';
import {saveMapping,storedMapping,type Mapping} from './kustomize-images';

export function MappingForm({app,members,environments,onDone}:{app:Membership;members:Membership[];environments:string[];onDone:()=>void}){
  const defaults:Mapping={deployId:app.repositoryId,reference:'',overlays:Object.fromEntries(environments.map(n=>[n.toLowerCase(),'overlays/'+n.toLowerCase()])),container:app.name};
  const [draft,setDraft]=useState(()=>storedMapping(app.repositoryId)??defaults);
  const [refs,setRefs]=useState<string[]>([]),[error,setError]=useState('');
  const deploy=members.find(m=>m.repositoryId===draft.deployId);
  useEffect(()=>{
    let live=true;setRefs([]);setError('');
    if(deploy)void Promise.all([api.remoteBranches(deploy.id),api.branches(deploy.id)]).then(([remote,locals])=>{
      if(!live)return;
      const list=[...remote.branches.map(b=>'refs/remotes/'+b.remote+'/'+b.name),...locals.map(b=>'refs/heads/'+b.name)];
      setRefs(list);
      setDraft(prev=>({...prev,reference:prev.reference||list.find(r=>r==='refs/remotes/origin/main')||list[0]||''}));
    }).catch(e=>{if(live)setError(errorMessage(e));});
    return()=>{live=false;};
  },[deploy?.id]);
  const edit=(v:Partial<Mapping>)=>{setDraft(old=>({...old,...v}));};
  return <form className="kube-settings" onSubmit={e=>{e.preventDefault();try{
    saveMapping(app.repositoryId,{...draft,container:draft.container.trim()});
    onDone();
  }catch(e){setError(errorMessage(e));}}}>
    <label>{t('Deploy repository')}<select value={draft.deployId} onChange={e=>edit({deployId:e.target.value,reference:''})}>
      {!deploy&&<option value={draft.deployId}>{t('Repository unavailable')}</option>}{members.map(m=><option key={m.id} value={m.repositoryId}>{m.name}</option>)}
    </select></label>
    <label>{t('Deploy branch')}<select value={draft.reference} onChange={e=>edit({reference:e.target.value})}>
      {!refs.includes(draft.reference)&&<option value={draft.reference}>{draft.reference||t('Select a branch')}</option>}
      {refs.map(r=><option key={r} value={r}>{r.replace(/^refs\/(heads|remotes)\//,'')}{r.startsWith('refs/heads/')?' · '+t('Local branch'):''}</option>)}
    </select></label>
    <p className="hint">{t('Paths are relative to the deploy repository. Fetch it to update remote branch data.')}</p>
    {environments.map(name=>{
      const id=name.toLowerCase();
      return <label key={id}>{t('{0} overlay',[name])}<input value={draft.overlays[id]??''} placeholder={'overlays/'+id}
        onChange={e=>edit({overlays:{...draft.overlays,[id]:e.target.value.trim()}})}/></label>;
    })}
    <label>{t('Container name')}<input required value={draft.container} placeholder="api" onChange={e=>edit({container:e.target.value})}/></label>
    <button className="primary" disabled={!deploy||!refs.includes(draft.reference)||!draft.container.trim()||!environments.some(n=>draft.overlays[n.toLowerCase()])}>{t('Save image mapping')}</button>
    {storedMapping(app.repositoryId)&&<button type="button" onClick={()=>{try{saveMapping(app.repositoryId,null);onDone();}catch(e){setError(errorMessage(e));}}}>{t('Remove mapping')}</button>}
    <button type="button" onClick={onDone}>{t('Cancel')}</button>
    {error&&<p role="alert">{t(error)}</p>}
  </form>;
}
