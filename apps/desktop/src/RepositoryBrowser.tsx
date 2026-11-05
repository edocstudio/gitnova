import {useCallback, useEffect, useId, useMemo, useRef, useState} from 'react';
import {api, errorMessage} from './api';
import {connections} from './connection-store';
import type {HostedRepository} from './contracts';
import {Icon} from './Icons';
import {t} from './i18n';

// Session-only metadata; no credentials. Evict entire account scopes, never grow unbounded.
const cache = new Map<string, {rows:HostedRepository[]; page:number; more:boolean; at:number}>();
export function clearRepositoryBrowserCache(){cache.clear();}
export function RepositoryBrowser({disabled,onSelect}:{disabled:boolean;onSelect:(url:string)=>void}) {
  const accountId=useId(),searchId=useId();
  const [profiles,setProfiles]=useState(connections),[selected,setSelected]=useState('');
  const [rows,setRows]=useState<HostedRepository[]>([]),[page,setPage]=useState(0),[more,setMore]=useState(false);
  const [query,setQuery]=useState(''),[visible,setVisible]=useState(0),[busy,setBusy]=useState(false),[error,setError]=useState('');
  const generation=useRef(0), flight=useRef(false);
  const profile=profiles.find(p=>p.id===selected);
  const key=profile?`${profile.provider}:${profile.origin}`:'';
  useEffect(()=>{const update=()=>{cache.clear();setProfiles(connections());setSelected('');};window.addEventListener('gitnova-ci-config',update);return()=>{generation.current++;window.removeEventListener('gitnova-ci-config',update);};},[]);
  useEffect(()=>{generation.current++;flight.current=false;setBusy(false);setError('');setQuery('');setVisible(0);const saved=cache.get(key);if(saved&&Date.now()-saved.at<60000){setRows(saved.rows);setPage(saved.page);setMore(saved.more);}else{setRows([]);setPage(0);setMore(Boolean(key));}},[key]);
  const load=useCallback(async()=>{
    if(!profile||disabled||flight.current)return;
    const version=generation.current;flight.current=true;setBusy(true);setError('');
    try{const result=await api.hostedRepositories(profile,page+1);if(version!==generation.current)return;
      if(result.repositories.length>100)throw new Error('Repository page exceeds its limit.');
      const next=[...new Map([...rows,...result.repositories].map(row=>[row.project,row])).values()].slice(0,500);
      const entry={rows:next,page:page+1,more:result.hasMore&&page+1<5,at:Date.now()};cache.delete(key);cache.set(key,entry);while(cache.size>8)cache.delete(cache.keys().next().value!);
      setRows(next);setPage(entry.page);setMore(entry.more);
    }catch(e){if(version===generation.current)setError(errorMessage(e));}finally{if(version===generation.current){flight.current=false;setBusy(false);}}
  },[profile,disabled,page,rows,key]);
  useEffect(()=>{if(!key||page!==0||busy||error)return;const timer=setTimeout(()=>void load(),150);return()=>clearTimeout(timer);},[key,page,busy,error,load]);
  const filtered=useMemo(()=>rows.filter(row=>row.project.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())),[rows,query]);
  const pages=Math.max(1,Math.ceil(filtered.length/5)), current=Math.min(visible,pages-1);
  return <div className="repository-browser">
    <label htmlFor={accountId}>{t('Connection / account')}</label><select id={accountId} value={selected} disabled={disabled||busy} onChange={e=>setSelected(e.target.value)}><option value="">{t('URL or local path')}</option>{profiles.filter(p=>p.enabled).map(p=><option key={p.id} value={p.id}>{p.label} · {p.origin}</option>)}</select>
    {profile&&<><div className="repository-browser-search"><label htmlFor={searchId}>{t('Search loaded repositories')}<input id={searchId} type="search" value={query} maxLength={256} disabled={disabled} onChange={e=>{setQuery(e.target.value);setVisible(0);}}/></label><button type="button" disabled={disabled||busy||!more} onClick={()=>void load()}>{busy?<><Icon name="refresh" className="loading-spin" size={14}/>{t('Loading repositories…')}</>:t(page?'Load more repositories':'Load repositories')}</button></div>
      <p className="hint">{t('Lists repositories this account can access. Search covers loaded results; load more to include the next 100. Up to 500 repositories are kept in memory. You can always paste a URL below.')}</p>
      {error&&<p role="alert" className="inline-error">{t(error)}</p>}
      <div aria-busy={busy}>{filtered.slice(current*5,current*5+5).map(row=><button className="repository-browser-row" type="button" key={row.project} disabled={disabled||busy} onClick={()=>onSelect(row.cloneUrl)}><Icon name="folder" size={16}/><span>{row.project}</span><small>{t(row.private?'Private':'Public')}</small></button>)}</div>
      {page>0&&filtered.length===0&&<p role="status">{t('No matching loaded repositories.')}</p>}
      {filtered.length>5&&<div className="repository-browser-pages"><button type="button" disabled={current===0||disabled} onClick={()=>setVisible(current-1)}>{t('Previous')}</button><span>{current+1} / {pages} · {filtered.length}</span><button type="button" disabled={current+1>=pages||disabled} onClick={()=>setVisible(current+1)}>{t('Next')}</button></div>}
    </>}
  </div>;
}
