import { CiRunLink } from './CiRunLink';
import { JenkinsStatus } from './JenkinsStatus';
import { readCi } from './ci-requests';
import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import type { HostingSelection, PipelineSnapshot } from './contracts';
import { t, dateLocale } from './i18n';

type Config = {remote:string;origin:string};
const storageKey = (id:string) => `gitnova.ci.gitlab.${id}`;
function saved(id:string):Config|null {
  try { const v=JSON.parse(localStorage.getItem(storageKey(id))??'null'); return v&&typeof v.remote==='string'&&typeof v.origin==='string'?v:null; } catch { return null; }
}
export function pipelineActive(snapshot:PipelineSnapshot) {
  return snapshot.runs.some(run=>['created','waiting_for_resource','preparing','pending','running'].includes(run.status));
}
export function pipelineSummary(snapshot:PipelineSnapshot) {
  const states=[...new Set(snapshot.runs.map(run=>run.status))];
  return states.length===0?'No pipeline':states.length===1?states[0]:'Mixed results';
}
const cache=new Map<string,PipelineSnapshot>();
export function clearPipelineCache() { cache.clear(); }
export function PipelineStatus({membershipId,oid,onSnapshot}:{membershipId:string;oid:string;onSnapshot:(value:PipelineSnapshot|null,source?:string,summary?:string)=>void}) {
  const [config,setConfig]=useState<Config|null>(()=>saved(membershipId));
  const [remote,setRemote]=useState(config?.remote??''); const [origin,setOrigin]=useState(config?.origin??'');
  const [remotes,setRemotes]=useState<string[]>([]); const [editing,setEditing]=useState(false);
  const [snapshot,setSnapshot]=useState<PipelineSnapshot|null>(null),[error,setError]=useState(''),[loading,setLoading]=useState(false);
  const [tick,setTick]=useState(0); const mounted=useRef(true); const connecting=useRef(false);
  const callback=useRef(onSnapshot);callback.current=onSnapshot;
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const selection=(c:Config):HostingSelection=>({remote:c.remote,origin:c.origin.trim()||null,provider:'gitlab',branch:'',oid});
  useEffect(()=>{
    if(!editing)return;let live=true;
    void api.remoteBranches(membershipId).then(v=>{if(live){setRemotes(v.remotes);if(!remote)setRemote(v.remotes.includes('origin')?'origin':v.remotes[0]??'');}}).catch(e=>{if(live)setError(errorMessage(e));});
    return()=>{live=false;};
  },[editing,membershipId]);
  useEffect(()=>{
    if(!config)return;
    let live=true;let reading=false;let timer:ReturnType<typeof setTimeout>|undefined;
    const key=JSON.stringify([membershipId,config.remote,config.origin,oid]);
    const old=cache.get(key); let hasData=Boolean(old); if(old){setSnapshot(old);callback.current(old);}
    const read=async(manual=false)=>{
      if(reading||!live||document.hidden)return;reading=true;
      if(!hasData||manual)setLoading(true);
      try {
        const value=await readCi(`gitlab:${key}`,()=>live&&!document.hidden,()=>api.pipelines(membershipId,selection(config),manual));
        if(value.oid!==oid)throw new Error('Pipeline revision did not match the selected commit.');
        if(!live)return;
        while(cache.size>=16&&!cache.has(key))cache.delete(cache.keys().next().value!);
        cache.set(key,value);hasData=true;setSnapshot(value);callback.current(value);setError('');
        // Only active runs poll. Stable/empty results require explicit refresh or revisiting.
        if(pipelineActive(value))timer=setTimeout(()=>void read(),20000);
      }catch(e){if(live&&!document.hidden){setError(errorMessage(e));timer=setTimeout(()=>void read(),60000);}}
      finally{reading=false;if(live)setLoading(false);}
    };
    const visibility=()=>{if(document.hidden){clearTimeout(timer);}else{clearTimeout(timer);timer=setTimeout(()=>void read(),1000);}};
    void read(tick>0);document.addEventListener('visibilitychange',visibility);
    return()=>{live=false;clearTimeout(timer);document.removeEventListener('visibilitychange',visibility);};
  },[membershipId,oid,config,tick]);
  async function connect(){
    if(connecting.current||!remote)return;connecting.current=true;setLoading(true);setError('');
    try{
      const next={remote,origin};const scope=selection(next);const context=await api.pipelineContext(membershipId,scope);
      if(!context.account||config)await api.connectPipeline(membershipId,scope);
      if(!mounted.current)return;
      const resolved={remote,origin:context.origin};
      try{localStorage.setItem(storageKey(membershipId),JSON.stringify(resolved));}catch{/* session-only settings still work */}
      clearPipelineCache();window.dispatchEvent(new Event('gitnova-ci-config'));setSnapshot(null);callback.current(null);setConfig(resolved);setEditing(false);
    }catch(e){if(mounted.current)setError(errorMessage(e));}finally{connecting.current=false;if(mounted.current)setLoading(false);}
  }
  return <section className="pipeline-status" aria-label={t('CI pipelines')}>
    <div className="pipeline-heading"><strong>{t('CI pipelines')}</strong>{config&&<button disabled={loading} onClick={()=>setTick(v=>v+1)}>{t('Refresh')}</button>}</div>
    {!config&&!editing&&<><p className="hint">{t('Show GitLab builds for this exact commit. Local Git works without connecting CI.')}</p><button onClick={()=>setEditing(true)}>{t('Connect GitLab CI…')}</button></>}
    {(editing||config)&&<details open={editing}><summary onClick={e=>{if(!editing){e.preventDefault();setEditing(true);}}}>{t('CI connection')}</summary>
      <label htmlFor="ci-remote">{t('Remote')}</label><select id="ci-remote" value={remote} disabled={loading} onChange={e=>setRemote(e.target.value)}><option value="">{t('Choose remote…')}</option>{remotes.map(r=><option key={r}>{r}</option>)}</select>
      <label htmlFor="ci-origin">{t('HTTPS web address (optional)')}</label><input id="ci-origin" value={origin} disabled={loading} maxLength={4096} placeholder="https://gitlab.example.com" onChange={e=>setOrigin(e.target.value)}/>
      <p className="hint">{t('GitLab CI needs API read access. Saved account credentials stay in Keychain.')}</p><p className="hint">{t('The saved GitLab account is shared with merge requests.')}</p>
      <button disabled={loading||!remote} onClick={()=>void connect()}>{config?t('Reconnect account…'):t('Connect')}</button>
      {config&&<button disabled={loading} onClick={()=>{try{localStorage.removeItem(storageKey(membershipId));}catch{/* session */}clearPipelineCache();window.dispatchEvent(new Event('gitnova-ci-config'));setConfig(null);setSnapshot(null);callback.current(null);setEditing(false);setError('');}}>{t('Stop showing CI')}</button>}
    </details>}
    {loading&&!snapshot&&<p role="status" className="operation-wait"><span className="loading-spin">↻</span>{t('Reading pipelines…')}</p>}
    {error&&<p role="alert" className="notice">{editing?t(error):snapshot?t('Showing cached CI status. Refresh failed.'):t('CI status unavailable. Check the connection and read permissions.')}</p>}
    {snapshot&&<><p className="hint">{snapshot.project} · <code>{oid.slice(0,8)}</code><br/>{t('Last checked')}: {new Date(snapshot.checkedAt*1000).toLocaleTimeString(dateLocale())}</p>
      {!snapshot.runs.length&&<p className="hint">{t('No pipeline for this exact commit.')}</p>}
      {snapshot.runs.map(run=><details className="pipeline-run" key={run.id}><summary><span className={`ci-badge ci-${run.status}`}>{t(run.status)}</span> #{run.id} · {run.reference}</summary>
        <p className="hint">{t('Pipeline source')}: {run.source||t('Status unavailable')}</p>
        <CiRunLink url={run.url}>{t('View pipeline on GitLab')} ↗</CiRunLink>
        {run.jobs.map(job=><div className="pipeline-job" key={job.id}><div><span>{job.stage} / {job.name}</span><strong>{t(job.status)}</strong></div>{job.runner&&<small>{t('Runner')}: {job.runner}</small>}{job.duration!==null&&<small>{t('Duration')}: {Math.round(job.duration)}s</small>}</div>)}
        {run.jobsPartial&&<p className="hint">{t('Showing the first 50 jobs. Open GitLab for all jobs and child pipelines.')}</p>}
      </details>)}
      <p className="hint">{t('Job list excludes child pipelines and retried attempts. Open GitLab for the complete run.')}</p>
      {snapshot.partial&&<p className="hint">{t('Showing the latest 5 pipelines for this commit.')}</p>}
    </>}
    <JenkinsStatus membershipId={membershipId} oid={oid} onSnapshot={(value,summary)=>onSnapshot(value,'jenkins',summary)}/>
  </section>;
}
