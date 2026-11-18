import {useEffect,useId,useLayoutEffect,useRef,useState} from 'react';
import {Icon} from './Icons';
import {CiRunLink} from './CiRunLink';
import {useCiActions} from './useCiActions';
import {formatTimestamp} from './date-format';
import {t,useLocale} from './i18n';
import type {CiActionKind} from './ci-actions';

const labels:Record<CiActionKind,string>={required:'Manual job required',optional:'Optional manual job',unknown:'Manual action available',blocked:'Pipeline blocked'};
export function CiNotifications({membershipId,repositoryName,oid,branch,blocked,onOpen,onInspect}:{membershipId:string;repositoryName:string;oid:string;branch:string|null;blocked:boolean;onOpen:()=>void;onInspect:(oid:string)=>Promise<void>}){
 useLocale();
 const {state,refresh}=useCiActions(membershipId,oid,branch,blocked);
 const [open,setOpen]=useState(false),[navigating,setNavigating]=useState(false),[error,setError]=useState(false);
 const root=useRef<HTMLDivElement>(null),trigger=useRef<HTMLButtonElement>(null),heading=useRef<HTMLHeadingElement>(null),flight=useRef(false);
 const panelId=useId();const items=state.value?.items??[];
 const close=(restore=false)=>{setOpen(false);if(restore)trigger.current?.focus();};
 useLayoutEffect(()=>{if(open)heading.current?.focus();},[open]);
 useEffect(()=>{
  if(!open)return;
  const outside=(event:PointerEvent)=>{if(!root.current?.contains(event.target as Node))setOpen(false);};
  const focusAway=(event:FocusEvent)=>{if(!root.current?.contains(event.target as Node))setOpen(false);};
  const escape=(event:KeyboardEvent)=>{if(event.key==='Escape'&&!document.querySelector('dialog[open]')){event.preventDefault();event.stopPropagation();close(true);}};
  window.addEventListener('pointerdown',outside);window.addEventListener('focusin',focusAway);window.addEventListener('keydown',escape,true);
  return()=>{window.removeEventListener('pointerdown',outside);window.removeEventListener('focusin',focusAway);window.removeEventListener('keydown',escape,true);};
 },[open]);
 const inspect=async()=>{
  if(blocked||flight.current)return;
  flight.current=true;setNavigating(true);setError(false);
  try{await onInspect(oid);close();}catch{setError(true);}finally{flight.current=false;setNavigating(false);}
 };
 return <div className="ci-notifications" ref={root}>
  <button ref={trigger} className="icon-button ci-actions-trigger" aria-label={t('CI actions ({0})',[items.length])} title={t(state.mode==='error'?'CI actions unavailable. Retry refresh to check the current state.':'CI actions for the current HEAD')} aria-description={state.mode==='error'?t('CI actions unavailable. Retry refresh to check the current state.'):state.value?.partial?t('CI action coverage is incomplete. Unseen jobs may still require attention.'):undefined} aria-controls={panelId} aria-expanded={open} disabled={blocked||navigating} onClick={()=>{if(open)close();else{onOpen();setOpen(true);}}}><Icon name="bell" size={19}/>{items.length>0&&<span className="ci-action-count">{items.length}</span>}{(state.mode==='error'||state.value?.partial)&&<span className="ci-action-warning" aria-hidden="true">!</span>}</button>
  {open&&<section id={panelId} className="ci-actions-popover" role="region" aria-label={t('CI actions')}>
   <div className="ci-actions-heading"><h2 ref={heading} tabIndex={-1}>{t('CI actions')}</h2><button className="icon-button" aria-label={t('Close CI actions')} onClick={()=>close(true)}><Icon name="close"/></button></div>
   <p className="ci-actions-context"><strong>{repositoryName}</strong> · {branch??t('Detached HEAD')} · <code>{oid.slice(0,8)}</code></p>
   <p className="hint">{t('Current HEAD only. Checked once a minute while this window is visible. Reading this list does not complete a CI action.')}</p>
   <div className="ci-actions-toolbar"><button disabled={blocked||navigating||state.reading||state.mode==='unconfigured'} onClick={refresh}>{t('Refresh CI actions')}</button>{state.value&&<small>{t('Last checked: {0}',[formatTimestamp(state.value.checkedAt,'time')])}</small>}</div>
   <div role="status" className="ci-actions-status">
    {state.mode==='loading'&&<p>{t('Checking CI actions…')}</p>}
    {state.mode==='unconfigured'&&<p>{t('Manual CI notifications currently support GitLab. Manage host connections in Settings.')}</p>}
    {state.mode==='unmatched'&&<p>{t('No matching GitLab connection for this repository.')}</p>}
    {state.mode==='error'&&<p className="notice">{t('CI actions unavailable. Retry refresh to check the current state.')}</p>}
    {state.stale&&state.value&&<p className="notice">{t('Cached CI evidence. Open the pipeline to verify its current status.')}</p>}
    {state.value?.partial&&<p className="notice">{t('CI action coverage is incomplete. Unseen jobs may still require attention.')}</p>}
    {state.mode==='ready'&&!items.length&&!state.value?.partial&&<p>{t('No manual or blocked CI actions found for this HEAD.')}</p>}
   </div>
   <div className="ci-actions-list">{items.map(item=><article className={`ci-action ci-action-${item.kind}`} key={item.id}>
    <strong>{item.jobId?item.name:t('Pipeline')} <small>#{item.pipelineId}</small></strong>
    {item.stage&&<p>{item.stage}</p>}<p className="ci-action-kind">{t(labels[item.kind])}</p>
    {item.kind==='unknown'&&<p className="hint">{t('Blocking requirements are not reported by this CI response.')}</p>}
    {item.stale&&<p className="notice">{t('Cached CI evidence. Open the pipeline to verify its current status.')}</p>}
    <div className="ci-action-links"><button disabled={blocked||navigating} onClick={()=>void inspect()}>{t('View commit')}</button><CiRunLink url={item.url} label={t('Open pipeline {0}',[item.pipelineId])}>{t('Open pipeline')}</CiRunLink></div>
   </article>)}</div>
   {error&&<p className="inline-error" role="alert">{t('Could not open this commit. Refresh the repository and try again.')}</p>}
  </section>}
 </div>;
}
