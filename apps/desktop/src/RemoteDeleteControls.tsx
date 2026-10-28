import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import { t } from './i18n';
import type { GitRef, OperationReceipt, RemoteDeletePreview } from './contracts';
export function RemoteDeleteControls({membershipId,selectedRef,blocked,onBack,onClose,onDone,onBusyChange,onFailure}:{membershipId:string;selectedRef:GitRef;blocked:boolean;onBack:()=>void;onClose:()=>void;onDone:(receipt:OperationReceipt)=>Promise<void>;onBusyChange:(busy:boolean)=>void;onFailure:(id:string,message:string)=>void}) {
 const [preview,setPreview]=useState<RemoteDeletePreview|null>(null),[loading,setLoading]=useState(true),[writing,setWriting]=useState(false),[error,setError]=useState('');const inFlight=useRef(false);
 useEffect(()=>{let cancelled=false;setPreview(null);setLoading(true);setError('');void (async()=>{
   const listing=await api.remoteBranches(membershipId);
   const matches=listing.branches.filter(b=>`refs/remotes/${b.remote}/${b.name}`===selectedRef.name&&b.oid===selectedRef.oid);
   if(matches.length!==1)throw new Error(t('The selected remote branch changed. Refresh and select it again.'));
   const branch=matches[0];const value=await api.previewRemoteDelete(membershipId,{remote:branch.remote,branch:branch.name,oid:branch.oid});if(!cancelled)setPreview(value);
 })().catch(e=>{if(!cancelled)setError(errorMessage(e));}).finally(()=>{if(!cancelled)setLoading(false);});return()=>{cancelled=true;};},[membershipId,selectedRef.name,selectedRef.oid]);
 const apply=async()=>{if(!preview||preview.blockedReason||blocked||inFlight.current)return;inFlight.current=true;setWriting(true);onBusyChange(true);setError('');try{const receipt=await api.deleteRemoteBranch(membershipId,preview.request,preview.token);await onDone(receipt);if(receipt.status==='succeeded')onClose();else{setPreview(null);setError(`${t(receipt.summary)} ${receipt.recovery??''}`);}}catch(e){setPreview(null);setError(errorMessage(e));onFailure(membershipId,errorMessage(e));}finally{inFlight.current=false;setWriting(false);onBusyChange(false);}};
 return <div className="remote-delete-review" aria-label={t('Delete remote branch')} aria-busy={loading||writing}>
 <strong>{t('Delete remote branch')} · {selectedRef.name.slice(13)}</strong>
 {loading&&<p role="status"><span className="loading-spin">↻</span> {t('Checking the live branch and local recovery…')}</p>}
 {preview&&<><p className="push-destination">{preview.destination}</p><p>{t('Removes only {0} on this remote.',[`refs/heads/${preview.request.branch}`])}</p><p>{t('Commit')} <code>{preview.request.oid}</code></p><p className="notice">{t('Local branches, working files and staging stay in place. Other developers may use this remote branch.')}</p><p>{t('The deleted tip is retained locally for recovery:')}</p><code className="recovery-ref">{preview.recoveryRef}</code><p className="hint">{preview.trackingRef?t('Removes the matching cached remote branch after server verification. Upstream configuration stays in place.'):t('Fetch/push URLs or mappings differ; cached fetch refs stay unchanged.')}</p><p className="hint">{t('Server permissions and protected-branch rules still apply. Configured push hooks run only after confirmation.')}</p>{preview.blockedReason&&<p className="notice" role="status">{t(preview.blockedReason)}</p>}</>}
 {error&&<div className="inline-error" role="alert">{t(error)}</div>}
 {writing&&<p role="status"><span className="loading-spin">↻</span> {t('Deleting remote branch…')}</p>}
 <div className="graph-action-footer"><button disabled={writing} onClick={onBack}>{t('Back')}</button><button className="danger-action" disabled={blocked||loading||writing||!preview||Boolean(preview.blockedReason)} onClick={()=>void apply()}>{t('Delete remote branch')}</button></div>
 </div>;
}
