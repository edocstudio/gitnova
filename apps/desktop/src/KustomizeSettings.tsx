import {useEffect,useState} from 'react';
import {api,errorMessage} from './api';
import {t} from './i18n';
import type {Membership,Workspace} from './contracts';
import {saveMapping,storedMapping,type Mapping} from './kustomize-images';

export function KustomizeSettings({workspace,initial}:{workspace:Workspace;initial:string}){
  const members=workspace.memberships.filter((m,i,all)=>all.findIndex(v=>v.repositoryId===m.repositoryId)===i);
  const [id,setId]=useState(initial);
  const member=members.find(m=>m.repositoryId===id)??members[0];
  if(!member)return null;
  return <section className="settings-general kube-settings"><h3>{t('Kustomize release images')}</h3>
    <label>{t('Application repository')}<select value={member.repositoryId} onChange={e=>setId(e.target.value)}>
      {members.map(m=><option key={m.repositoryId} value={m.repositoryId}>{m.name}</option>)}</select></label>
    <MappingForm key={member.repositoryId} app={member} members={members}/>
  </section>;
}
function MappingForm({app,members}:{app:Membership;members:Membership[]}){
  const defaults:Mapping={deployId:app.repositoryId,reference:'',production:'overlays/production',uat:'overlays/uat',container:''};
  const [draft,setDraft]=useState(()=>storedMapping(app.repositoryId)??defaults);
  const [refs,setRefs]=useState<string[]>([]),[error,setError]=useState(''),[notice,setNotice]=useState('');
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
  const edit=(v:Partial<Mapping>)=>{setDraft(old=>({...old,...v}));setNotice('');};
  return <form onSubmit={e=>{e.preventDefault();try{
    saveMapping(app.repositoryId,{...draft,production:draft.production.trim(),uat:draft.uat.trim(),container:draft.container.trim()});
    setNotice('Mapping saved. Open Releases to see the images.');
  }catch(e){setError(errorMessage(e));}}}>
    <label>{t('Deploy repository')}<select value={draft.deployId} onChange={e=>edit({deployId:e.target.value,reference:''})}>
      {!deploy&&<option value={draft.deployId}>{t('Repository unavailable')}</option>}{members.map(m=><option key={m.id} value={m.repositoryId}>{m.name}</option>)}
    </select></label>
    <label>{t('Deploy branch')}<select value={draft.reference} onChange={e=>edit({reference:e.target.value})}>
      {!refs.includes(draft.reference)&&<option value={draft.reference}>{draft.reference||t('Select a branch')}</option>}
      {refs.map(r=><option key={r} value={r}>{r.replace(/^refs\/(heads|remotes)\//,'')}{r.startsWith('refs/heads/')?' · '+t('Local branch'):''}</option>)}
    </select></label>
    <p className="hint">{t('Paths are relative to the deploy repository. Fetch it to update remote branch data.')}</p>
    <label>{t('Production overlay')}<input required value={draft.production} onChange={e=>edit({production:e.target.value})}/></label>
    <label>{t('UAT overlay')}<input required value={draft.uat} onChange={e=>edit({uat:e.target.value})}/></label>
    <label>{t('Container name')}<input required value={draft.container} placeholder="api" onChange={e=>edit({container:e.target.value})}/></label>
    <button className="primary" disabled={!deploy||!refs.includes(draft.reference)||![draft.production,draft.uat,draft.container].every(x=>x.trim())}>{t('Save image mapping')}</button>
    {storedMapping(app.repositoryId)&&<button type="button" onClick={()=>{try{saveMapping(app.repositoryId,null);setDraft({...defaults,reference:refs[0]??''});setNotice('Mapping removed.');}catch(e){setError(errorMessage(e));}}}>{t('Remove mapping')}</button>}
    {notice&&<p role="status">{t(notice)}</p>}{error&&<p role="alert">{t(error)}</p>}
  </form>;
}
