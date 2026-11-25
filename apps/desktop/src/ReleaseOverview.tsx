import {useEffect,useMemo,useState} from 'react';
import {api,errorMessage} from './api';
import {readConfirmation,readRelease,releaseAction,type Confirmation,type ReleaseEvidence} from './release-evidence';
import {readImages,imageVersion,type ConfigEvidence} from './kustomize-images';
import {Icon} from './Icons';
import {t,dateLocale} from './i18n';
import type {Membership,Workspace} from './contracts';

function status(info?:ReleaseEvidence){
  if(!info)return 'Checking…';
  if(releaseAction(info.run))return 'Manual action required';
  if(info.run)return info.run.status==='success'?'Pipeline passed':info.run.status;
  if(info.build)return info.build.status;
  if(info.jenkins)return 'Jenkins job';
  return info.pipelineError?'CI unavailable':info.run===null?'No release pipeline':'CI not connected';
}
function stamp(when:number|string){return new Date(when).toLocaleString(dateLocale());}
function ConfirmProduction({scope,onDone}:{scope:string;onDone:()=>void}){
  const [error,setError]=useState('');
  return <form onSubmit={e=>{
    e.preventDefault();const data=new FormData(e.currentTarget);
    const value=(key:string)=>String(data.get(key)??'').trim();
    try{
      const link=new URL(value('url'));
      if(link.protocol!=='https:'||link.username||link.password)throw new Error('Enter a credential-free HTTPS build link.');
      const evidence:Confirmation={sha:value('sha').toLowerCase(),version:value('version'),by:value('by'),url:link.href,time:Date.now()};
      if(!evidence.by||!evidence.version)throw new Error('Complete the confirmation details.');
      localStorage.setItem('gitnova.release.confirmed.'+scope,JSON.stringify(evidence));onDone();
    }catch(err){setError(errorMessage(err));}
  }}>
    <p>{t('Record the production version you verified on this device.')}</p>
    <label>{t('Deployed version')}<input name="version" required maxLength={128}/></label>
    <label>{t('Deployed commit SHA')}<input name="sha" required pattern="([A-Fa-f0-9]{40}|[A-Fa-f0-9]{64})" maxLength={64}/></label>
    <label>{t('Build or deployment URL')}<input name="url" required type="url" maxLength={4096}/></label>
    <label>{t('Confirmed by')}<input name="by" required maxLength={100}/></label>
    {error&&<p role="alert">{t(error)}</p>}
    <button>{t('Save confirmation')}</button><button type="button" onClick={onDone}>{t('Cancel')}</button>
  </form>;
}
function ConfigImage({config,env}:{config:ConfigEvidence;env:'production'|'uat'}){
  const result=config.images?.[env];
  return <div className="config-image"><small>{t('Image from config')}</small>
    {result?.image?<strong><code>{result.image}</code></strong>:<p role="status">{t(config.error||result?.error||'Image unavailable')}</p>}
    <small title={config.images?.revision}>{config.deployName} · {config.mapping.reference.replace(/^refs\/(heads|remotes)\//,'')} {config.images?.revision.slice(0,12)}</small>
    <small>{config.mapping[env]} · {config.mapping.container}</small></div>;
}
function ReleaseDetails({member,info,onConfirmed}:{member:Membership;info?:ReleaseEvidence;onConfirmed:()=>void}){
  const [form,setForm]=useState(false),[error,setError]=useState('');
  const open=(url:string)=>{setError('');void api.openCiUrl(url).catch(e=>setError(errorMessage(e)));};
  const manual=readConfirmation(info?.scope),prod=info?.production,run=info?.run,action=releaseAction(run);
  return <aside className="release-detail"><h2><Icon name="folder" size={18}/>{member.name}</h2>
    <section><h3>{t('Production')}</h3>
      {info?.config&&<><ConfigImage config={info.config} env="production"/><small>{t('Deployment evidence')}</small></>}
      {prod?<><strong>{prod.tag||prod.sha.slice(0,12)}</strong><p><code>{prod.sha}</code></p>
        {prod.finished&&<small>{stamp(prod.finished)}</small>}
        <button onClick={()=>open(prod.url)}>{t('GitLab deployment #{0}',[prod.id])} ↗</button></>
      :manual?<><strong>{manual.version}</strong><p><code>{manual.sha}</code></p>
        <small>{t('Confirmed by {0}',[manual.by])} · {stamp(manual.time)}</small>
        <button onClick={()=>open(manual.url)}>{t('Open deployment evidence')} ↗</button></>:<p>{t(info?'No verified deployment':'Checking…')}</p>}
      {info?.productionError&&<p className="notice">{t('Deployment history unavailable')} · {t(info.productionError)}</p>}
      {info?.scope&&!prod&&(form?<ConfirmProduction scope={info.scope} onDone={()=>{setForm(false);onConfirmed();}}/>:<button onClick={()=>setForm(true)}>{t('Record production confirmation')}</button>)}
    </section>
    <section><h3>{t('UAT')} · <code>release</code></h3>{info?.config&&<ConfigImage config={info.config} env="uat"/>}<strong>{t(status(info))}</strong>
      {run&&<><p><code>{run.sha}</code></p><button onClick={()=>open(run.url)}>{t('Pipeline #{0}',[run.id])} ↗</button>
      {run.jobs.length>0&&<ul className="release-stages">{run.jobs.map(j=><li key={j.id}><span>{j.stage} / {j.name}</span><b>{t(j.status)}</b></li>)}</ul>}
      {run.jobsPartial&&<small>{t('More jobs are available in the pipeline.')}</small>}</>}
      {action&&<button className="primary" onClick={()=>open(action.url)}>{t(action.label)} ↗</button>}
      {info?.build&&<p><code>{info.build.sha.slice(0,12)}</code> <button onClick={()=>open(info.build!.url)}>{t('Jenkins build #{0}',[info.build.id])} ↗</button></p>}
      {info?.buildError&&<p className="notice">{t(info.buildError)}</p>}
      {info?.jenkins&&<button onClick={()=>open(info.jenkins!)}>{t('Open Jenkins job')} ↗</button>}
      {info?.pipelineError&&<p className="notice">{t(info.pipelineError)}</p>}
    </section>{info&&<small>{t('Last checked')} · {stamp(info.checked)}</small>}{error&&<p role="alert">{t(error)}</p>}
  </aside>;
}
export function ReleaseOverview({workspace}:{workspace:Workspace}){
  const members=useMemo(()=>workspace.memberships.filter((m,i,all)=>all.findIndex(v=>v.repositoryId===m.repositoryId)===i),[workspace.memberships]);
  const [states,setStates]=useState<Record<string,ReleaseEvidence>>({});
  const [focus,setFocus]=useState(workspace.selectedId??''),[attention,setAttention]=useState(false),[revision,refresh]=useState(0),[loading,setLoading]=useState(true);
  const [,confirmed]=useState(0);
  useEffect(()=>{
    let live=true;setStates({});setLoading(true);
    async function read(){
      for(const m of members){
        if(!live)break;
        const configured=readImages(m.repositoryId,workspace);
        let info:ReleaseEvidence;
        try{info=await readRelease(m,()=>live);}catch(e){info={checked:Date.now(),productionError:errorMessage(e),pipelineError:errorMessage(e)};}
        info.config=await configured;
        if(live)setStates(prior=>({...prior,[m.id]:info}));
      }
      if(live)setLoading(false);
    }
    void read();
    const changed=()=>refresh(v=>v+1);
    window.addEventListener('gitnova-ci-config',changed);
    return()=>{live=false;window.removeEventListener('gitnova-ci-config',changed);};
  },[members,revision,workspace]);
  const rows=members.filter(m=>!attention||releaseAction(states[m.id]?.run));
  const selected=rows.find(m=>m.id===focus)??rows[0];
  return <div className="releases-page"><header><div><h1>{t('Releases')}</h1><small>{workspace.name} · {t('Workspace')}</small></div>
    <select aria-label={t('Release filter')} value={attention?'action':'all'} onChange={e=>setAttention(e.target.value==='action')}>
      <option value="all">{t('All repositories')}</option><option value="action">{t('Needs action')}</option></select>
    <button disabled={loading} onClick={()=>refresh(v=>v+1)}><Icon name="refresh" size={15}/>{t(loading?'Checking…':'Refresh')}</button></header>
    <div className="release-layout"><div className="release-list"><div className="release-head"><span>{t('Repository')}</span><span>{t('Production')}</span><span>{t('UAT')}</span></div>
      {rows.map(m=>{const info=states[m.id],prod=info?.production,manual=readConfirmation(info?.scope);
        const p=info?.config?.images?.production.image,u=info?.config?.images?.uat.image;
        return <button key={m.id} className="release-row" aria-pressed={selected?.id===m.id} onClick={()=>setFocus(m.id)}>
          <b>{m.name}</b><span>{p?imageVersion(p):prod?.tag||prod?.sha.slice(0,12)||manual?.version||t(info?'Unverified':'Checking…')}
          <small>{p?t('From config'):prod?'GitLab':manual?t('Manual confirmation'):''}</small></span>
          <span className={releaseAction(info?.run)?'release-wait':''}>{u&&<><b>{imageVersion(u)}</b><small>{t('From config')}</small></>}{releaseAction(info?.run)?'Ⅱ · ':''}{t(status(info))}</span></button>;})}
      {!rows.length&&<p role="status">{t(loading?'Checking…':attention?'No manual actions found.':'No repositories in this workspace.')}</p>}
    </div>{selected&&<div><ReleaseDetails onConfirmed={()=>confirmed(n=>n+1)} key={selected.id} member={selected} info={states[selected.id]}/></div>}</div>
  </div>;
}
