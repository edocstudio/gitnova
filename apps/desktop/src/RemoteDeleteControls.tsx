import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import { t } from './i18n';
import type { GitRef, OperationReceipt, RemoteDeletePreview, RemoteDeleteRequest } from './contracts';

const canRefreshLabel = (message: string) =>
  message === 'This branch no longer exists on the remote. Refresh to remove its outdated local label.'
  || message === 'This branch points to a different commit on the remote. Refresh and review the new tip before deleting.';

export function RemoteDeleteControls({membershipId,selectedRef,blocked,onBack,onClose,onDone,onBusyChange,onFailure}:{membershipId:string;selectedRef:GitRef;blocked:boolean;onBack:()=>void;onClose:()=>void;onDone:(receipt:OperationReceipt)=>Promise<void>;onBusyChange:(busy:boolean)=>void;onFailure:(id:string,message:string)=>void}) {
 const [preview, setPreview] = useState<RemoteDeletePreview | null>(null);
 const [refreshRequest, setRefreshRequest] = useState<RemoteDeleteRequest | null>(null);
 const [loading, setLoading] = useState(true);
 const [action, setAction] = useState<'delete' | 'refresh' | null>(null);
 const [error, setError] = useState('');
 const inFlight = useRef(false);
 const writing = action !== null;

 useEffect(() => {
   let cancelled = false;
   let request: RemoteDeleteRequest | null = null;
   setPreview(null);
   setRefreshRequest(null);
   setLoading(true);
   setError('');
   void (async () => {
     const listing = await api.remoteBranches(membershipId);
     if (cancelled) return;
     const matches = listing.branches.filter(branch =>
       `refs/remotes/${branch.remote}/${branch.name}` === selectedRef.name && branch.oid === selectedRef.oid);
     if (matches.length !== 1) throw new Error('The selected remote branch changed. Refresh and select it again.');
     const branch = matches[0];
     request = {remote: branch.remote, branch: branch.name, oid: branch.oid};
     const value = await api.previewRemoteDelete(membershipId, request);
     if (!cancelled) setPreview(value);
   })().catch(e => {
     if (cancelled) return;
     const message = errorMessage(e);
     setError(message);
     if (request && canRefreshLabel(message)) setRefreshRequest(request);
   }).finally(() => { if (!cancelled) setLoading(false); });
   return () => { cancelled = true; };
 }, [membershipId, selectedRef.name, selectedRef.oid]);

 const submit = async (kind: 'delete' | 'refresh') => {
   if (blocked || loading || inFlight.current) return;
   if (kind === 'delete' && (!preview || preview.blockedReason)) return;
   if (kind === 'refresh' && !refreshRequest) return;
   inFlight.current = true;
   setAction(kind);
   setError('');
   onBusyChange(true);
   try {
     const receipt = kind === 'refresh'
       ? await api.refreshRemoteDeleteTracking(membershipId, refreshRequest!)
       : await api.deleteRemoteBranch(membershipId, preview!.request, preview!.token);
     setPreview(null);
     if (receipt.status === 'succeeded') setRefreshRequest(null);
     await onDone(receipt);
     if (receipt.status === 'succeeded') onClose();
     else setError(`${t(receipt.summary)} ${receipt.recovery ?? ''}`);
   } catch (e) {
     const message = errorMessage(e);
     setPreview(null);
     setError(message);
     if (kind === 'delete') {
       if (preview && canRefreshLabel(message)) setRefreshRequest(preview.request);
       else onFailure(membershipId, message);
     }
   } finally {
     inFlight.current = false;
     setAction(null);
     onBusyChange(false);
   }
 };
 return <div className="remote-delete-review" aria-label={t('Delete remote branch')} aria-busy={loading||writing}>
 <strong>{t('Delete remote branch')} · {selectedRef.name.slice(13)}</strong>
 {loading&&<p role="status"><span className="loading-spin" aria-hidden="true">↻</span> {t('Checking the live branch and local recovery…')}</p>}
 {preview&&<><p className="push-destination">{preview.destination}</p><p>{t('Removes only {0} on this remote.',[`refs/heads/${preview.request.branch}`])}</p><p>{t('Commit')} <code>{preview.request.oid}</code></p><p className="notice">{t('Local branches, working files and staging stay in place. Other developers may use this remote branch.')}</p><p>{t('The deleted tip is retained locally for recovery:')}</p><code className="recovery-ref">{preview.recoveryRef}</code><p className="hint">{preview.trackingRef?t('Removes the matching cached remote branch after server verification. Upstream configuration stays in place.'):t('Fetch/push URLs or mappings differ; cached fetch refs stay unchanged.')}</p><p className="hint">{t('Server permissions and protected-branch rules still apply. Configured push hooks run only after confirmation.')}</p>{preview.blockedReason&&<p className="notice" role="status">{t(preview.blockedReason)}</p>}</>}
 {error&&<div className="inline-error" role="alert">{t(error)}</div>}
 {writing&&<p role="status"><span className="loading-spin" aria-hidden="true">↻</span> {t(action === 'refresh' ? 'Refreshing the selected branch label…' : 'Deleting remote branch…')}</p>}
 <div className="graph-action-footer"><button disabled={writing} onClick={onBack}>{t('Back')}</button>{refreshRequest
   ? <button className="primary" disabled={blocked||loading||writing} onClick={()=>void submit('refresh')}>{t('Refresh branch label')}</button>
   : <button className="danger-action" disabled={blocked||loading||writing||!preview||Boolean(preview.blockedReason)} onClick={()=>void submit('delete')}>{t('Delete remote branch')}</button>}</div>
 </div>;
}
