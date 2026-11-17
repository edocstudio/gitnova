import {useEffect,useRef,useState} from 'react';
import {api,errorMessage} from './api';
import type {OperationReceipt,RemoteEndpointPreview,RemoteEndpointSettings} from './contracts';
import {t,useLocale} from './i18n';

export function RemoteEndpointDialog({membershipId,repositoryName,names,initialName,blocked,onBusyChange,onDone,onClose}:{membershipId:string;repositoryName:string;names:string[];initialName:string;blocked:boolean;onBusyChange:(value:boolean)=>void;onDone:(receipt:OperationReceipt)=>Promise<void>;onClose:()=>void}){
  useLocale();
  const dialog=useRef<HTMLDialogElement>(null),flight=useRef(false),sequence=useRef(0);
  const [name,setName]=useState(initialName),[settings,setSettings]=useState<RemoteEndpointSettings|null>(null);
  const [urls,setUrls]=useState(''),[pushUrls,setPushUrls]=useState(''),[samePush,setSamePush]=useState(true);
  const [review,setReview]=useState<RemoteEndpointPreview|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false),[loading,setLoading]=useState(false),[stale,setStale]=useState(false),[reload,setReload]=useState(0);
  const urlInput=useRef<HTMLTextAreaElement>(null);
  useEffect(()=>{const node=dialog.current,previous=document.activeElement as HTMLElement|null;node?.showModal();return()=>{sequence.current++;node?.close();if(previous?.isConnected)previous.focus();};},[]);
  useEffect(()=>{
    const request=++sequence.current;setLoading(true);setSettings(null);setReview(null);setError('');setStale(false);
    api.remoteEndpoint(membershipId,name).then(value=>{if(sequence.current===request){setSettings(value);setUrls(value.request.urls.join('\n'));setPushUrls(value.request.pushUrls.join('\n'));setSamePush(!value.request.pushUrls.length);}}).catch(e=>{if(sequence.current===request)setError(errorMessage(e));}).finally(()=>{if(sequence.current===request)setLoading(false);});
    return()=>{sequence.current++;};
  },[membershipId,name,reload]);
  useEffect(()=>{if(settings&&!loading)urlInput.current?.focus();},[settings,loading]);
  const locked=blocked||busy||loading;
  const urlLines=(value:string)=>value.split('\n').map(line=>line.trim()).filter(Boolean);
  const multiple=urlLines(urls).length>1||!samePush&&urlLines(pushUrls).length>1;
  const close=()=>{if(!flight.current)onClose();};
  const failed=(e:unknown)=>{setError(errorMessage(e));setReview(null);if(e&&typeof e==='object'&&'code' in e&&e.code==='stale_preview')setStale(true);};
  const prepare=async()=>{
    if(!settings||locked||flight.current||stale||multiple)return;
    flight.current=true;setLoading(true);setError('');
    try{setReview(await api.previewRemoteEndpoint(membershipId,{name,urls:urlLines(urls),pushUrls:samePush?[]:urlLines(pushUrls)},settings.token));}
    catch(e){failed(e);}finally{flight.current=false;setLoading(false);}
  };
  const apply=async()=>{
    if(!review||locked||flight.current)return;
    flight.current=true;setBusy(true);onBusyChange(true);setError('');
    try{
      const receipt=await api.changeRemoteEndpoint(membershipId,review.request,review.previous.token,review.token);
      try{await onDone(receipt);}catch{/* The recorded write result remains authoritative. */}
      window.dispatchEvent(new Event('gitnova-remotes-changed'));
      window.dispatchEvent(new Event('gitnova-ci-config'));
      window.dispatchEvent(new Event('gitnova-requests-changed'));
      if(receipt.status==='succeeded')onClose();else{setError(receipt.recovery??receipt.summary);setReview(null);setStale(true);}
    }catch(e){failed(e);setStale(true);}finally{flight.current=false;setBusy(false);onBusyChange(false);}
  };
  const list=(values:string[])=>values.length?values.map((value,index)=><code key={index}>{value}</code>):<span>{t('Use fetch URL for push')}</span>;
  return <dialog ref={dialog} className="settings-dialog remote-endpoint-dialog" aria-labelledby="remote-endpoint-title" onCancel={event=>{event.preventDefault();close();}} onKeyDown={event=>event.stopPropagation()}>
    <div className="settings-header"><h2 id="remote-endpoint-title">{t('Edit remote endpoint')}</h2><button disabled={busy} onClick={close}>{t('Close')}</button></div>
    <p><strong>{repositoryName}</strong></p><p className="hint">{t('Applies to this repository and its linked worktrees. Other repositories and account connections are unchanged.')}</p>
    <label htmlFor="endpoint-remote">{t('Remote name')}</label><select id="endpoint-remote" value={name} disabled={locked||!!review} onChange={event=>setName(event.target.value)}>{names.map(remote=><option key={remote} value={remote}>{remote}</option>)}</select>
    {review?<section aria-label={t('Review remote endpoint')}>
      <table><thead><tr><th>{t('URL type')}</th><th>{t('Current')}</th><th>{t('New')}</th></tr></thead><tbody><tr><th>{t('Fetch')}</th><td>{list(review.previous.request.urls)}</td><td>{list(review.request.urls)}</td></tr><tr><th>{t('Push')}</th><td>{list(review.previous.request.pushUrls)}</td><td>{list(review.request.pushUrls)}</td></tr></tbody></table>
      <p>{t('Updates local remote URLs only. No fetch, push or branch change is performed. Existing remote-tracking refs remain until you fetch.')}</p>
      <p className="hint">{t('Git URL rewrite rules still apply. GitNova uses the connection matching the new host; manage accounts in Settings.')}</p>
      <div className="graph-action-footer"><button disabled={locked} onClick={()=>setReview(null)}>{t('Back')}</button><button className="primary" disabled={locked} onClick={()=>void apply()}>{t(busy?'Applying…':'Save remote URLs')}</button></div>
    </section>:<form onSubmit={event=>{event.preventDefault();void prepare();}}>
      <label htmlFor="endpoint-fetch">{t('Fetch URLs (one per line)')}</label><textarea ref={urlInput} id="endpoint-fetch" rows={Math.min(4,Math.max(1,urls.split('\n').length))} value={urls} maxLength={65543} disabled={locked||!settings} required spellCheck={false} onChange={event=>setUrls(event.target.value)}/>
      <label className="ref-checkbox"><input type="checkbox" checked={samePush} disabled={locked||!settings} onChange={event=>setSamePush(event.target.checked)}/>{t('Use fetch URL for push')}</label>
      {!samePush&&<><label htmlFor="endpoint-push">{t('Push URLs (one per line)')}</label><textarea id="endpoint-push" rows={Math.min(4,Math.max(1,pushUrls.split('\n').length))} value={pushUrls} maxLength={65543} disabled={locked||!settings} required spellCheck={false} onChange={event=>setPushUrls(event.target.value)}/></>}
      {multiple&&<p className="notice" role="status">{t('GitNova Push supports one destination. Choose one fetch URL and at most one separate push URL before reviewing.')}</p>}
      <p className="hint">{t('Use a credential helper or SSH agent. Do not put tokens or passwords in URLs.')}</p>
      <div className="graph-action-footer"><button type="button" disabled={busy} onClick={close}>{t('Cancel')}</button><button className="primary" disabled={locked||!settings||stale||multiple||!urls.trim()||!samePush&&!pushUrls.trim()}>{t('Review remote change')}</button></div>
    </form>}
    {loading&&<p className="loading-status" role="status">{t('Reading remote…')}</p>}{error&&<p className="inline-error" role="alert">{t(error)}</p>}
    {stale&&<button disabled={locked} onClick={()=>setReload(value=>value+1)}>{t('Reload current remote URLs')}</button>}
  </dialog>;
}
