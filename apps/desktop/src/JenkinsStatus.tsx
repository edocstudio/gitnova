import { CiRunLink } from './CiRunLink';
import { useEffect,useRef,useState } from 'react';
import { api,errorMessage } from './api';
import { readCi } from './ci-requests';
import type { JenkinsSelection,JenkinsSnapshot,PipelineSnapshot } from './contracts';
import { t,dateLocale } from './i18n';
type Config=Omit<JenkinsSelection,'oid'>;
const storageKey=(id:string)=>`gitnova.ci.jenkins.${id}`;
function saved(id:string):Config|null{
 try{const v=JSON.parse(localStorage.getItem(storageKey(id))??'null');return v&&['remote','controller','jobUrl','username'].every(k=>typeof v[k]==='string')?v:null;}catch{return null;}
}
const cache=new Map<string,JenkinsSnapshot>();
export function JenkinsStatus({membershipId,oid,onSnapshot}:{membershipId:string;oid:string;onSnapshot:(value:PipelineSnapshot|null,summary?:string)=>void}){
 const [config,setConfig]=useState<Config|null>(()=>saved(membershipId));
 const [draft,setDraft]=useState<Config>(config??{remote:'',controller:'',jobUrl:'',username:''});
 const [editing,setEditing]=useState(false),[remotes,setRemotes]=useState<string[]>([]);
 const [value,setValue]=useState<JenkinsSnapshot|null>(null),[loading,setLoading]=useState(false),[error,setError]=useState(''),[tick,setTick]=useState(0);
 const active=useRef(true),connecting=useRef(false),callback=useRef(onSnapshot);callback.current=onSnapshot;
 useEffect(()=>{active.current=true;return()=>{active.current=false;};},[]);
 useEffect(()=>{if(!editing)return;let live=true;void api.remoteBranches(membershipId).then(v=>{if(live){setRemotes(v.remotes);setDraft(d=>({...d,remote:d.remote|| (v.remotes.includes('origin')?'origin':v.remotes[0]??'')}));}}).catch(e=>{if(live)setError(errorMessage(e));});return()=>{live=false;};},[membershipId,editing]);
 useEffect(()=>{
  if(!config)return;let live=true;let reading=false;let timer:ReturnType<typeof setTimeout>|undefined;
  const key=JSON.stringify(['jenkins',membershipId,config,oid]);const old=cache.get(key);let hasData=Boolean(old);
  if(old){setValue(old);callback.current(old.pipeline,old.pipeline.runs.length?undefined:'No verified build');}
  const read=async(manual=false)=>{
   if(reading||!live||document.hidden)return;reading=true;if(!hasData||manual)setLoading(true);
   try{
    const result=await readCi(key,()=>live&&!document.hidden,()=>api.jenkinsBuilds(membershipId,{...config,oid},manual));
    if(!live)return;if(result.pipeline.oid!==oid)throw new Error('Pipeline revision did not match the selected commit.');
    while(cache.size>=8&&!cache.has(key))cache.delete(cache.keys().next().value!);cache.set(key,result);
    hasData=true;setValue(result);setError('');callback.current(result.pipeline,result.pipeline.runs.length?undefined:'No verified build');
    if(result.activeUnlinked||result.pipeline.runs.some(run=>run.status==='running'))timer=setTimeout(()=>void read(),20000);
   }catch(e){if(live&&!document.hidden){setError(errorMessage(e));timer=setTimeout(()=>void read(),60000);}}
   finally{reading=false;if(live)setLoading(false);}
  };
  const visibility=()=>{clearTimeout(timer);if(!document.hidden)timer=setTimeout(()=>void read(),1000);};
  void read(tick>0);document.addEventListener('visibilitychange',visibility);
  return()=>{live=false;clearTimeout(timer);document.removeEventListener('visibilitychange',visibility);};
 },[membershipId,oid,config,tick]);
 const connect=async()=>{
  if(connecting.current)return;connecting.current=true;setLoading(true);setError('');
  try{await api.connectJenkins(membershipId,{...draft,oid});if(!active.current)return;
   try{localStorage.setItem(storageKey(membershipId),JSON.stringify(draft));}catch{/* preferences only */}
   cache.clear();setValue(null);callback.current(null);setConfig({...draft});setEditing(false);setTick(v=>v+1);
  }catch(e){if(active.current)setError(errorMessage(e));}finally{connecting.current=false;if(active.current)setLoading(false);}
 };
 return <div className="jenkins-status" aria-label={t('Jenkins builds')}>
  <div className="pipeline-heading"><strong>Jenkins</strong>{config&&<button disabled={loading} onClick={()=>setTick(v=>v+1)} aria-label={t('Refresh Jenkins builds')}>{t('Refresh')}</button>}</div>
  {!config&&!editing&&<button onClick={()=>setEditing(true)}>{t('Connect Jenkins…')}</button>}
  {(editing||config)&&<details open={editing}><summary onClick={e=>{if(!editing){e.preventDefault();setEditing(true);}}}>{t('Jenkins connection')}</summary>
   <label htmlFor="jenkins-remote">{t('Remote')}</label><select id="jenkins-remote" disabled={loading} value={draft.remote} onChange={e=>setDraft(d=>({...d,remote:e.target.value}))}><option value="">{t('Choose remote…')}</option>{remotes.map(r=><option key={r}>{r}</option>)}</select>
   <label htmlFor="jenkins-controller">{t('Jenkins controller URL')}</label><input id="jenkins-controller" disabled={loading} value={draft.controller} maxLength={4096} placeholder="https://jenkins.example.com/jenkins/" onChange={e=>setDraft(d=>({...d,controller:e.target.value}))}/>
   <label htmlFor="jenkins-job">{t('Jenkins job URL')}</label><input id="jenkins-job" disabled={loading} value={draft.jobUrl} maxLength={4096} placeholder="https://jenkins.example.com/jenkins/job/team/job/app/job/develop/" onChange={e=>setDraft(d=>({...d,jobUrl:e.target.value}))}/>
   <label htmlFor="jenkins-username">{t('Jenkins username')}</label><input id="jenkins-username" disabled={loading} value={draft.username} maxLength={256} autoComplete="off" onChange={e=>setDraft(d=>({...d,username:e.target.value}))}/>
   <p className="hint">{t('Choose the branch job inside any folders. Builds must expose Git BuildData for this remote and exact SHA.')}</p>
   <p className="hint">{t('The API token is entered in a native secure prompt and saved separately in Keychain. Job read permission is required.')}</p>
   <button disabled={loading||Object.values(draft).some(v=>!v.trim())} onClick={()=>void connect()}>{config?t('Reconnect Jenkins account…'):t('Connect Jenkins account…')}</button>
   {config&&<button disabled={loading} onClick={()=>{try{localStorage.removeItem(storageKey(membershipId));}catch{/* preferences */}cache.clear();setConfig(null);setValue(null);callback.current(null);setEditing(false);setError('');}}>{t('Stop showing Jenkins')}</button>}
  </details>}
  {loading&&!value&&<p role="status" className="operation-wait"><span className="loading-spin">↻</span>{t('Reading Jenkins builds…')}</p>}
  {error&&<p role="alert" className="notice">{editing?t(error):value?t('Showing cached Jenkins status. Refresh failed.'):t('Jenkins status unavailable. Check connection settings and job read permission.')}</p>}
  {value&&<><p className="hint">{value.pipeline.origin}<br/>{value.pipeline.project}<br/><code>{oid.slice(0,8)}</code> · {t('Last checked')}: {new Date(value.pipeline.checkedAt*1000).toLocaleTimeString(dateLocale())}</p>
   {value.pipeline.runs.map(run=><div className="pipeline-run" key={run.id}><span className={`ci-badge ci-${run.status}`}>{t(run.status)}</span> <CiRunLink url={run.url}>{t('Build #{0}',[run.id])} ↗</CiRunLink></div>)}
   {!value.pipeline.runs.length&&<p className="hint">{t('No verified Jenkins build for this commit in the scanned history.')}</p>}
   {value.unlinked>0&&<p className="notice">{t('{0} builds could not be linked to this repository and commit. Their results are not shown on the graph.',[value.unlinked])}</p>}
   <p className="hint">{t('Checked {0} recent builds (maximum 20). Older builds may exist.',[value.scanned])}</p>
   <p className="hint">{t('Open the Jenkins build for stages, agents and logs. No build or deployment is triggered here.')}</p>
  </>}
 </div>;
}
