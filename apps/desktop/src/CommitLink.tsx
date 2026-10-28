import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import { connections } from './connection-store';
import { commitLinkSelection } from './commit-link-selection';
import { t } from './i18n';
import { Icon } from './Icons';
export function CommitLink({membershipId,oid}:{membershipId:string;oid:string}) {
 const [busy,setBusy]=useState(false),[copied,setCopied]=useState(false),[url,setUrl]=useState(''),[error,setError]=useState('');
 const generation=useRef(0),inFlight=useRef(false);
 useEffect(()=>{generation.current++;inFlight.current=false;setCopied(false);setUrl('');setError('');setBusy(false);return()=>{generation.current++;};},[membershipId,oid]);
 async function copy(){
  if(inFlight.current)return;inFlight.current=true;const request=generation.current;const live=()=>request===generation.current;setBusy(true);setError('');setCopied(false);setUrl('');
  try {
   const [data,tracking]=await Promise.allSettled([api.remoteBranches(membershipId),api.pullUpstream(membershipId)]);
   if(!live())return;if(data.status==='rejected')throw data.reason;
   const selection=commitLinkSelection(data.value,tracking.status==='fulfilled'?tracking.value.remote:null,oid,connections());
   const link=await api.commitLink(membershipId,selection);
   if(!live())return;
   try{await navigator.clipboard.writeText(link);if(live())setCopied(true);}catch{if(live()){setUrl(link);setError('Clipboard unavailable. Copy the commit link below.');}}
  }catch(e){if(live())setError(errorMessage(e));}
  finally{if(live()){inFlight.current=false;setBusy(false);}}
 }
 return <div className="commit-link">
  <button className="copy-sha" disabled={busy} onClick={()=>void copy()}><Icon name={copied?'check':'copy'} size={15}/>{busy?t('Preparing commit link…'):copied?t('Commit link copied'):t('Copy commit link')}</button>
  {url&&<code className="commit-link-url">{url}</code>}
  {error&&<p role="status" className="hint">{t(error)}</p>}
 </div>;
}
