import {useEffect,useMemo,useRef,useState} from 'react';
import {api,errorMessage} from './api';
import {readConfirmation,readRelease,type Confirmation,type ReleaseEvidence} from './release-evidence';
import {mappingChanged,readImages,imageVersion,type ConfigEvidence} from './kustomize-images';
import {cachedConfig,cacheConfig,observedEvents,rememberEvents,configEvents,type ReleaseEvent} from './release-cache';
import {storedMapping} from './kustomize-images';
import {fetchWorkspace,type FetchReport} from './workspace-fetch';
import {MappingForm} from './KustomizeSettings';
import {releaseEnvironments} from './ReleaseEnvironments';
import {Icon} from './Icons';
import {t,dateLocale} from './i18n';
import type {Membership,Workspace} from './contracts';

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
function ConfigDate({value}:{value?:string|null}){
  if(!value||!Number.isFinite(Date.parse(value)))return <small>{t('Config date unavailable')}</small>;
  return <small>{t('Config updated')} · <time dateTime={value}>{stamp(value)}</time></small>;
}
function ConfigImage({config,env}:{config:ConfigEvidence;env:string}){
  const result=config.images?.overlays[env];
  return <div className="config-image"><small>{t('Image from config')}</small>
    {result?.image?<strong><code>{result.image}</code></strong>:<p role="status">{t(config.error||result?.error||'Image unavailable')}</p>}
    {result?.image&&<ConfigDate value={result.configUpdatedAt}/>}
    <small title={config.images?.revision}>{config.deployName} · {config.mapping.reference.replace(/^refs\/(heads|remotes)\//,'')} {config.images?.revision.slice(0,12)}</small>
    <small>{config.mapping.overlays[env]} · {config.mapping.container}</small></div>;
}
function ReleaseDetails({member,info,config,onConfirmed,environments,members,onOpenRepository,navigationBusy}:RepositoryNavigation & {config?:ConfigEvidence;members:Membership[];environments:string[];member:Membership;info?:ReleaseEvidence;onConfirmed:()=>void}){
  const [form,setForm]=useState(false),[error,setError]=useState('');
  const [editing,setEditing]=useState(false);
  const open=(url:string)=>{setError('');void api.openCiUrl(url).catch(e=>setError(errorMessage(e)));};
  const manual=readConfirmation(info?.scope),prod=info?.production;
  return <aside className="release-detail"><h2><Icon name="folder" size={18}/>{member.name}</h2>
    <button disabled={navigationBusy} onClick={()=>onOpenRepository(member.id)}>{t('Open repository in History')}</button>
    <button aria-expanded={editing} onClick={()=>setEditing(v=>!v)}>{t('Configure release images')}</button>
    {editing&&<MappingForm app={member} members={members} environments={environments} onDone={()=>setEditing(false)}/>}
    {environments.map(name=>{
      const id=name.toLowerCase();
      const mapped=config&&Object.hasOwn(config.mapping.overlays,id)&&config.mapping.overlays[id];
      return <section key={id}><h3>{name}</h3>
      {mapped&&config&&<ConfigImage config={config} env={id}/>}
      {id==='production'?<>{mapped&&(prod||manual)&&<small>{t('Deployment evidence')}</small>}
      {prod?<><strong>{prod.tag||prod.sha.slice(0,12)}</strong><p><code>{prod.sha}</code></p>
        {prod.finished&&<small>{stamp(prod.finished)}</small>}
        <button onClick={()=>open(prod.url)}>{t('GitLab deployment #{0}',[prod.id])} ↗</button></>
      :manual?<><strong>{manual.version}</strong><p><code>{manual.sha}</code></p>
        <small>{t('Confirmed by {0}',[manual.by])} · {stamp(manual.time)}</small>
        <button onClick={()=>open(manual.url)}>{t('Open deployment evidence')} ↗</button></>:!mapped&&<small>{t(info?'No overlay configured':'Checking…')}</small>}
      {info?.productionError&&<p className="notice">{t('Deployment history unavailable')} · {t(info.productionError)}</p>}
      {info?.scope&&!prod&&(form?<ConfirmProduction scope={info.scope} onDone={()=>{setForm(false);onConfirmed();}}/>:<button onClick={()=>setForm(true)}>{t('Record production confirmation')}</button>)}
      </>:!mapped&&<small>{t('No overlay configured')}</small>}
      </section>;
    })}{info&&<small>{t('Last checked')} · {stamp(info.checked)}</small>}{error&&<p role="alert">{t(error)}</p>}
  </aside>;
}
function localDay(date:Date){
  return [date.getFullYear(),String(date.getMonth()+1).padStart(2,'0'),String(date.getDate()).padStart(2,'0')].join('-');
}
function ReleaseCalendar({members,workspace,environments,configs,states}:{members:Membership[];workspace:Workspace;environments:string[];configs:Record<string,ConfigEvidence|undefined>;states:Record<string,ReleaseEvidence>}){
  const workspaceId=workspace.id;
  const locale=dateLocale();
  const [environment,setEnvironment]=useState('');
  const [source,setSource]=useState<ReleaseEvent['source']>('Config updated');
  const [picked,setPicked]=useState('');
  const [viewMonth,setViewMonth]=useState('');
  const env=environments.includes(environment)?environment:'';
  const observedDays=useMemo(()=>{
    const groups=new Map<string,{repo:string;id:string;event:ReleaseEvent;commit?:string}[]>();
    const names=new Set((env?[env]:environments).map(n=>n.toLowerCase()));
    for(const member of members){
      const repo=member.repositoryId;
      const records=[...observedEvents(workspaceId,repo),...configEvents(configs[repo]),...deploymentEvents(states[member.id])];
      const seen=new Set<string>();
      for(const event of records){
        if(!names.has(event.environment)||event.source!==source)continue;
        const signature=JSON.stringify(event);
        if(seen.has(signature))continue;
        seen.add(signature);
        const when=new Date(event.at);
        if(!Number.isFinite(when.getTime()))continue;
        const day=localDay(when);
        const rows=groups.get(day)??[];
        rows.push({repo:member.name,id:repo,event});groups.set(day,rows);
      }
    }
    for(const rows of groups.values())rows.sort((a,b)=>Date.parse(b.event.at)-Date.parse(a.event.at));
    return groups;
  },[members,workspaceId,environments,configs,states,source,env]);

  const today=localDay(new Date());
  const newest=[...observedDays.keys()].sort().at(-1);
  const month=viewMonth||(newest??today).slice(0,7);
  const [year,monthNumber]=month.split('-').map(Number);
  const historyRequests=useMemo(()=>members.flatMap(member=>{
    const config=configs[member.repositoryId];
    const deploy=workspace.memberships.find(m=>m.repositoryId===config?.mapping.deployId);
    if(source!=='Config updated'||!config?.images||!deploy)return [];
    const overlays=Object.fromEntries(Object.entries(config.mapping.overlays).filter(([id,path])=>
      path && (env?[env]:environments).some(name=>name.toLowerCase()===id)));
    if(!Object.keys(overlays).length)return [];
    const mapping={...config.mapping,overlays};
    const key=JSON.stringify([workspaceId,deploy.id,mapping,config.images.revision,month]);
    return [{key,member,deploy,mapping}];
  }),[members,workspace,workspaceId,configs,source,env,environments,month]);
  type HistoryResult=Awaited<ReturnType<typeof api.kustomizeHistory>>;
  const cache=useRef(new Map<string,Promise<HistoryResult>>());
  const [results,setResults]=useState(new Map<string,HistoryResult>());
  useEffect(()=>{
    let live=true;
    const start=new Date(year,monthNumber-1,1).getTime()/1000;
    const end=new Date(year,monthNumber,1).getTime()/1000;
    const wanted=new Set(historyRequests.map(r=>r.key));
    setResults(previous=>{
      if([...previous.keys()].every(key=>wanted.has(key)))return previous;
      return new Map([...previous].filter(([key])=>wanted.has(key)));
    });
    // Keep a bounded set of immutable month reads and share in-flight work.
    async function load(){
      for(const request of historyRequests){
        if(!live)break;
        let task=cache.current.get(request.key);
        if(!task){
          task=api.kustomizeHistory(request.deploy.id,request.mapping,start,end);
          cache.current.set(request.key,task);
          if(cache.current.size>256)cache.current.delete(cache.current.keys().next().value!);
        }
        let result:HistoryResult;
        try{result=await task;}catch{
          cache.current.delete(request.key);
          result={partial:true,records:[]};
        }
        if(live)setResults(previous=>{
          if(previous.get(request.key)===result)return previous;
          const next=new Map(previous);
          next.set(request.key,result);
          return next;
        });
      }
    }
    void load();
    return ()=>{live=false;};
  },[historyRequests,year,monthNumber]);
  const days=useMemo(()=>{
    const merged=new Map([...observedDays].map(([day,rows])=>[day,[...rows]]));
    for(const {key,member} of historyRequests){
      const result=results.get(key);
      if(!result)continue;
      if(!result.partial){
        for(const [day,rows] of merged)if(day.startsWith(month+'-'))
          merged.set(day,rows.filter(row=>row.id!==member.repositoryId));
      }
      for(const record of result.records){
        const day=localDay(new Date(record.at));
        const rows=merged.get(day)??[];
        const event:ReleaseEvent={source:'Config updated',environment:record.environment,version:imageVersion(record.image),at:record.at};
        if(result.partial && rows.some(row=>row.id===member.repositoryId && JSON.stringify(row.event)===JSON.stringify(event)))continue;
        rows.push({repo:member.name,id:member.repositoryId,event,commit:record.commit});
        merged.set(day,rows);
      }
    }
    for(const [day,rows] of merged){
      if(!rows.length)merged.delete(day);
      else rows.sort((a,b)=>Date.parse(b.event.at)-Date.parse(a.event.at));
    }
    return merged;
  },[observedDays,results,historyRequests,month]);
  const selected=picked.startsWith(month+'-')?picked:[...days.keys()].filter(d=>d.startsWith(month+'-')).sort().at(-1)??(today.startsWith(month+'-')?today:month+'-01');
  const rows=days.get(selected)??[];
  const first=new Date(year,monthNumber-1,1,12);
  const count=new Date(year,monthNumber,0,12).getDate();
  const moveMonth=(offset:number)=>setViewMonth(localDay(new Date(year,monthNumber-1+offset,1,12)).slice(0,7));
  const fullDate=(day:string)=>new Date(day+'T12:00:00').toLocaleDateString(locale,{dateStyle:'long'});
  return <section className="release-calendar">
    <div className="calendar-controls">
      <h2>{t('Release calendar')}</h2>
      <label>{t('Activity')}<select aria-label={t('Activity')} value={source} onChange={e=>setSource(e.target.value as ReleaseEvent['source'])}>
        <option value="Config updated">{t('Config updates')}</option><option value="Deployment">{t('Deployment')}</option><option value="Manual confirmation">{t('Manual confirmation')}</option>
      </select></label>
      <label>{t('Environment')}<select aria-label={t('Environment')} value={env} onChange={e=>setEnvironment(e.target.value)}>
        <option value="">{t('All environments')}</option>{environments.map(name=><option key={name}>{name}</option>)}
      </select></label>
    </div>
    <div className="calendar-layout"><div className="calendar-month">
      <div className="calendar-month-heading"><h3 aria-live="polite">{first.toLocaleDateString(locale,{month:'long',year:'numeric'})}</h3>
        <button aria-label={t('Previous month')} onClick={()=>moveMonth(-1)}>←</button>
        <button onClick={()=>{setViewMonth(today.slice(0,7));setPicked(today);}}>{t('Today')}</button>
        <button aria-label={t('Next month')} onClick={()=>moveMonth(1)}>→</button>
      </div>
      <div className="calendar-grid">
        {Array.from({length:7},(_,i)=><span className="calendar-weekday" key={'w'+i}>{new Date(2024,0,1+i,12).toLocaleDateString(locale,{weekday:'short'})}</span>)}
        {Array.from({length:count},(_,i)=>{
          const day=month+'-'+String(i+1).padStart(2,'0'),n=days.get(day)?.length??0;
          return <button key={day} data-day={day} data-heat={n>=10?4:n>=5?3:n>=2?2:n?1:0}
            aria-label={fullDate(day)+' · '+t('{0} records',[n])} aria-pressed={day===selected} aria-current={day===today?'date':undefined}
            style={i===0?{gridColumnStart:(first.getDay()+6)%7+1}:undefined}
            onClick={()=>{setViewMonth(month);setPicked(day);}}><span>{i+1}</span>{n>0&&<b>{n}</b>}</button>;
        })}
      </div>
      <div className="calendar-scale">{t('Records per day')}{['0','1','2–4','5–9','10+'].map((band,i)=><span key={band}><i data-heat={i}/>{band}</span>)}</div>
    </div><section className="calendar-detail"><h3>{fullDate(selected)}</h3>
      {rows.length>0?<><p>{t('{0} records',[rows.length])} · {t('{0} repositories',[new Set(rows.map(r=>r.id)).size])}</p>
        <ul>{rows.map(({repo,id,event,commit})=><li key={JSON.stringify([id,event,commit])}><strong>{repo}</strong>
          <time dateTime={event.at}>{new Date(event.at).toLocaleTimeString(locale)}</time>
          <span>{environments.find(n=>n.toLowerCase()===event.environment)}</span><code>{event.version}</code></li>)}</ul></>:<p>{t('No matching records yet.')}</p>}
    </section></div><small role="status">{source==='Config updated'?
      historyRequests.some(r=>!results.has(r.key))?t('Reading Git configuration history…'):
      historyRequests.some(r=>results.get(r.key)?.partial)?t('Some configuration history could not be read.'):
      t('Configuration updates from local deploy branch history.'):
      t('Shows dated records observed by GitNova on this device.')}</small>
  </section>;
}
function deploymentEvents(info?:ReleaseEvidence):ReleaseEvent[]{
  const prod=info?.production,manual=readConfirmation(info?.scope);
  const records:ReleaseEvent[]=[];
  if(prod?.finished)records.push({source:'Deployment',environment:'production',at:prod.finished,version:prod.tag||prod.sha.slice(0,12)});
  if(manual&&Number.isFinite(new Date(manual.time).getTime()))records.push({source:'Manual confirmation',environment:'production',at:new Date(manual.time).toISOString(),version:manual.version});
  return records;
}

const PAGE_SIZE=10;
type RepositoryNavigation = {
  onOpenRepository: (membershipId: string) => void;
  navigationBusy: boolean;
};
export function ReleaseOverview({workspace,onOpenRepository,navigationBusy}:{workspace:Workspace} & RepositoryNavigation){
  const members=useMemo(()=>{
    const seen=new Set<string>();
    return workspace.memberships.filter(m=>{
      if(seen.has(m.repositoryId))return false;
      seen.add(m.repositoryId);return true;
    });
  },[workspace.memberships]);
  const [states,setStates]=useState<Record<string,ReleaseEvidence>>({});
  const [configs,setConfigs]=useState<Record<string,ConfigEvidence|undefined>>({});
  const [focus,setFocus]=useState(workspace.selectedId??''),[revision,refresh]=useState(0),[loading,setLoading]=useState(true);
  const environments=useMemo(releaseEnvironments,[revision]);
  const [daily,setDaily]=useState(false);
  const [query,setQuery]=useState('');
  const [page,setPage]=useState(()=>Math.floor(Math.max(0,members.findIndex(m=>m.id===workspace.selectedId))/PAGE_SIZE));
  const matches=useMemo(()=>{
    const term=query.trim().normalize('NFC').toLowerCase();
    return members.filter(m=>m.name.normalize('NFC').toLowerCase().includes(term));
  },[members,query]);
  const pages=Math.max(1,Math.ceil(matches.length/PAGE_SIZE));
  const currentPage=Math.min(page,pages-1);
  const rows=useMemo(()=>matches.slice(currentPage*PAGE_SIZE,(currentPage+1)*PAGE_SIZE),[matches,currentPage]);
  const requested=daily?matches:rows;
  const schedule=useRef<(next:Membership[])=>void>(()=>{});
  const [report,setReport]=useState<FetchReport|null>(null);
  const [fetching,setFetching]=useState(false);
  const fetchToken=useRef<symbol|null>(null);
  const afterFetch=useRef<(repo:string,reference:string,oid:string|null)=>Promise<void>>(async()=>{});
  useEffect(()=>()=>{fetchToken.current=null;},[]);
  async function startFetch(retry=false){
    if(fetchToken.current||navigationBusy)return;
    const token=Symbol();
    const active=()=>fetchToken.current===token;
    fetchToken.current=token;
    setFetching(true);
    try{
      await fetchWorkspace(members,retry?(report?.failures.map(f=>f.member)??[]):members,active,setReport,
        (repo,reference,oid)=>afterFetch.current(repo,reference,oid));
    }finally{
      if(active()){fetchToken.current=null;setFetching(false);}
    }
  }
  const [,confirmed]=useState(0);
  useEffect(()=>{
    let live=true;setStates({});setLoading(true);
    const snapshots:Record<string,ConfigEvidence>={};
    for(const member of members){
      const config=cachedConfig(workspace,member.repositoryId,environments);
      if(config)snapshots[member.repositoryId]=config;
    }
    setConfigs(snapshots);
    const imageReads=new Map<string,Promise<ConfigEvidence|undefined>>();
    if(revision===0)for(const [repo,config] of Object.entries(snapshots))imageReads.set(repo,Promise.resolve(config));
    function updateImages(repo:string){
      const task=readImages(repo,workspace,environments).then(config=>{
        if(live&&imageReads.get(repo)===task){
          cacheConfig(workspace,repo,environments,config);
          rememberEvents(workspace.id,repo,configEvents(config));
          setConfigs(prior=>({...prior,[repo]:config}));
        }
        return config;
      });
      imageReads.set(repo,task);
      return task;
    }
    afterFetch.current=async(repo,reference,oid)=>{
      const cached=imageReads.get(repo);
      if(!cached){
        if(storedMapping(repo)?.reference===reference)await updateImages(repo);
        return;
      }
      const config=await cached;
      if(live&&imageReads.get(repo)===cached&&config?.mapping.reference===reference&&(!oid||config.images?.revision!==oid))
        await updateImages(repo);
    };
    const onMapping=(event:Event)=>{
      const repo=(event as CustomEvent<string>).detail;
      if(members.some(m=>m.repositoryId===repo))void updateImages(repo);
    };
    const evidence=new Map<string,ReleaseEvidence>();
    let pending:Membership[]=[];
    let wanted=new Set<string>();
    const showProduction=environments.some(name=>name.toLowerCase()==='production');
    let running=false;
    async function drain(){
      running=true;
      try{
        while(live&&pending.length){
          const member=pending.pop()!;
          await (imageReads.get(member.repositoryId)??updateImages(member.repositoryId));
          if(!live||!wanted.has(member.id))continue;
          let info=evidence.get(member.id);
          if(!info){
            try{info=showProduction?await readRelease(member,()=>live):{checked:Date.now()};}
            catch(e){info={checked:Date.now(),productionError:errorMessage(e)};}
            if(!live)break;
            rememberEvents(workspace.id,member.repositoryId,deploymentEvents(info));
            evidence.set(member.id,info);
            const result=info;
            setStates(prior=>({...prior,[member.id]:result}));
          }

        }
      }finally{running=false;if(live)setLoading(false);}
    }
    schedule.current=next=>{
      wanted=new Set(next.map(m=>m.id));
      pending=next.filter(m=>!evidence.has(m.id)||!imageReads.has(m.repositoryId)).reverse();
      if(!running){
        setLoading(pending.length>0);
        queueMicrotask(()=>{if(live&&!running&&pending.length)void drain();});
      }
    };
    const changed=()=>refresh(v=>v+1);
    window.addEventListener(mappingChanged,onMapping);
    window.addEventListener('gitnova-ci-config',changed);
    return()=>{live=false;window.removeEventListener(mappingChanged,onMapping);window.removeEventListener('gitnova-ci-config',changed);};
  },[members,revision,workspace,environments]);
  useEffect(()=>{schedule.current(requested);},[requested,workspace,revision]);
  const selected=rows.find(m=>m.id===focus)??rows[0];
  return <div className="releases-page"><header><div><h1>{t('Releases')}</h1><small>{workspace.name} · {t('Workspace')}</small></div>
    <button aria-pressed={daily} onClick={()=>setDaily(value=>!value)}>{t('Release calendar')}</button>
    <input type="search" aria-label={t('Search repositories')} placeholder={t('Search repositories')} maxLength={512} value={query}
      onChange={e=>{setQuery(e.target.value);setPage(0);}}/>
    <button disabled={fetching||navigationBusy||!members.length} onClick={()=>void startFetch()}><Icon name="pull" size={15}/>{t('Fetch workspace')}</button>
    <button disabled={loading||fetching} onClick={()=>refresh(v=>v+1)}><Icon name="refresh" size={15}/>{t(loading?'Checking…':'Refresh')}</button></header>
    {report&&<div className="workspace-fetch-result">
      <div className="fetch-meter" role="progressbar" aria-label={t('Fetch workspace')} aria-valuemin={0} aria-valuemax={report.total} aria-valuenow={report.done}>
        <span style={{width:(report.total?100*report.done/report.total:0)+'%'}}/></div>
      <span role="status"><strong>{t(fetching?'Fetching…':'Fetch complete')}</strong> · {t('Processed {0} of {1} repositories',[report.done,report.total])} · {t('Fetched {0}',[report.done-report.skipped-report.failures.length])}
        {report.skipped>0&&<> · {t('Skipped {0} without remotes',[report.skipped])}</>}</span>
      {report.failures.length>0&&<><details><summary>{t('Failed {0}',[report.failures.length])}</summary>
        <ul>{report.failures.map(({member,reason})=><li key={member.id}>{member.name} · {t(reason)}</li>)}</ul></details>
        <button disabled={fetching||navigationBusy} onClick={()=>void startFetch(true)}>{t('Retry failed')}</button></>}
    </div>}
    {daily?<ReleaseCalendar members={matches} workspace={workspace} environments={environments} configs={configs} states={states}/>:<div className="release-layout"><div className="release-list" style={{'--env-count':environments.length} as React.CSSProperties}><div className="release-head"><span>{t('Repository')}</span>{environments.map(name=><span key={name}>{name}</span>)}</div>
      {rows.map(m=>{const config=configs[m.repositoryId],info=states[m.id],prod=info?.production,manual=readConfirmation(info?.scope);
        return <button key={m.id} className="release-row" aria-pressed={selected?.id===m.id} onClick={()=>setFocus(m.id)}>
          <b>{m.name}</b>{environments.map(name=>{
            const id=name.toLowerCase(),image=config?.images?.overlays[id]?.image;
            const failed=config?.images?.overlays[id]?.error || (config?.mapping.overlays[id]&&config.error);
            const label=id==='production'?(prod?.tag||prod?.sha.slice(0,12)||manual?.version||t(info?'No overlay configured':'Checking…')):t(info?'No overlay configured':'Checking…');
            return <span key={id}>
              {image?<><b>{imageVersion(image)}</b><ConfigDate value={config?.images?.overlays[id]?.configUpdatedAt}/></>:failed?<small>{t('Image unavailable')}</small>:null}
              {(!image&&!failed)&&label}
              {id==='production'&&!image&&!failed&&<small>{prod?'GitLab':manual?t('Manual confirmation'):''}{prod?.finished?' · '+stamp(prod.finished):!prod&&manual?' · '+stamp(manual.time):''}</small>}
            </span>;
          })}</button>;})}
      {!rows.length&&<p role="status">{t(!members.length?'No repositories in this workspace.':'No matching repositories.')}</p>}
      {matches.length>0&&<div className="repo-pagination">
        <span role="status">{t('{0}–{1} of {2} repositories',[currentPage*PAGE_SIZE+1,currentPage*PAGE_SIZE+rows.length,matches.length])}</span>
        {pages>1&&<nav aria-label={t('Repository pages')}>
          <button aria-label={t('Previous repository page')} disabled={!currentPage} onClick={()=>setPage(currentPage-1)}>←</button>
          <span>{t('Page {0} of {1}',[currentPage+1,pages])}</span>
          <button aria-label={t('Next repository page')} disabled={currentPage+1===pages} onClick={()=>setPage(currentPage+1)}>→</button>
        </nav>}
      </div>}
    </div>{selected&&<div><ReleaseDetails navigationBusy={navigationBusy} onOpenRepository={onOpenRepository} config={configs[selected.repositoryId]} members={members} environments={environments} onConfirmed={()=>{rememberEvents(workspace.id,selected.repositoryId,deploymentEvents(states[selected.id]));confirmed(n=>n+1);}} key={selected.id} member={selected} info={states[selected.id]}/></div>}</div>}
  </div>;
}
