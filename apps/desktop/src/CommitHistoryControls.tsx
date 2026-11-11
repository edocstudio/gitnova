import {useEffect, useRef, useState} from 'react';
import {api, errorMessage} from './api';
import type {CommitHistoryPreview, OperationReceipt} from './contracts';
import {Icon} from './Icons';
import {t, useLocale} from './i18n';

/** Current-HEAD actions live beside branch navigation, independent of row selection. */
export function CommitHistoryControls({membershipId,head,branch,disabled,onBusyChange,onDone,onFailure}:{membershipId:string;head?:string|null;branch?:string|null;disabled:boolean;onBusyChange:(busy:boolean)=>void;onDone:(receipt:OperationReceipt)=>Promise<void>;onFailure:(id:string,message:string)=>void}) {
  const [action,setAction]=useState<boolean|null>(null);
  const [redoAvailable,setRedoAvailable]=useState(false);
  useEffect(()=>{
    let active=true;
    setRedoAvailable(false);
    if(branch&&!disabled) void api.previewCommitHistory(membershipId,true).then(p=>{if(active)setRedoAvailable(!p.blockedReason);}).catch(()=>{});
    return()=>{active=false;};
  },[membershipId,head,branch,disabled]);
  return <>
    <button className="undo-current" disabled={disabled||!branch||!head} title={t('Undo the latest local commit; keep its changes staged.')} onClick={()=>setAction(false)}><Icon name="undo"/>{t('Undo')}</button>
    <button className="redo-current" disabled={disabled||!branch||!redoAvailable} title={t('Restore the commit removed by Undo.')} onClick={()=>setAction(true)}><Icon name="redo"/>{t('Redo')}</button>
    {action!==null&&<CommitHistoryReview key={`${membershipId}-${action}`} membershipId={membershipId} redo={action} onClose={()=>setAction(null)} onBusyChange={onBusyChange} onDone={onDone} onFailure={onFailure}/>}
  </>;
}

function CommitHistoryReview({membershipId,redo,onClose,onBusyChange,onDone,onFailure}:{membershipId:string;redo:boolean;onClose:()=>void;onBusyChange:(busy:boolean)=>void;onDone:(receipt:OperationReceipt)=>Promise<void>;onFailure:(id:string,message:string)=>void}) {
  useLocale();
  const dialog=useRef<HTMLDialogElement>(null);
  const inFlight=useRef(false);
  const [preview,setPreview]=useState<CommitHistoryPreview|null>(null);
  const [error,setError]=useState('');
  const [loading,setLoading]=useState(true);
  const [busy,setBusy]=useState(false);
  const [acknowledged,setAcknowledged]=useState(false);
  useEffect(()=>{
    const node=dialog.current;const previous=document.activeElement as HTMLElement|null;node?.showModal();
    let active=true;
    void api.previewCommitHistory(membershipId,redo).then(value=>{if(active)setPreview(value);}).catch(reason=>{if(active)setError(errorMessage(reason));}).finally(()=>{if(active)setLoading(false);});
    return()=>{active=false;node?.close();if(previous?.isConnected)previous.focus();};
  },[membershipId,redo]);
  const apply=async()=>{
    if(!preview||inFlight.current||preview.blockedReason||(!redo&&!acknowledged))return;
    inFlight.current=true;setBusy(true);onBusyChange(true);setError('');
    try {
      const receipt=await api.applyCommitHistory(membershipId,redo,preview.token);
      await onDone(receipt);
      if(receipt.status==='succeeded')onClose();else {setError(receipt.summary);setPreview(null);}
    } catch(reason) {const message=errorMessage(reason);setError(message);setPreview(null);onFailure(membershipId,message);}
    finally {inFlight.current=false;setBusy(false);onBusyChange(false);}
  };
  return <dialog ref={dialog} className="settings-dialog lock-recovery-dialog commit-history-review" aria-labelledby="commit-history-title" onCancel={event=>{event.preventDefault();if(!inFlight.current)onClose();}} onKeyDown={event=>event.stopPropagation()}>
    <h2 id="commit-history-title">{t(redo?'Redo commit':'Undo latest commit')}</h2>
    {loading&&<p role="status"><Icon name="refresh" className="loading-spin"/> {t('Reading your repository…')}</p>}
    {preview&&<>
      <p>{t('On branch {0}',[preview.branch])}</p>
      <p><code>{preview.commitOid.slice(0,12)}</code> · {preview.subject}</p>
      <p>{t(redo?'Restore the original commit. Files and staging stay in place.':'Move this branch to its first parent. Keep all committed changes staged so you can edit and commit again.')}</p>
      {!redo&&!preview.parentOid&&<p>{t('This is the first commit. Undo leaves this branch with no commits and its files staged.')}</p>}
      {!redo&&<>
        <p className="warning">{t('Publication checks use locally fetched remote refs and tags. They cannot detect pushes that have not been fetched.')}</p>
        <label><input type="checkbox" checked={acknowledged} disabled={busy} onChange={event=>setAcknowledged(event.target.checked)}/>{t('I confirm this commit has not been published or shared.')}</label>
      </>}
      <p>{t('Recovery ref')} <code className="history-recovery-ref">{preview.recoveryRef}</code></p>
      <p>{t('Redo restores one original commit and is available only while files, staging and branch history still match the Undo result.')}</p>
      {preview.blockedReason&&<p role="alert" className="warning">{t(preview.blockedReason)}</p>}
    </>}
    {error&&<p role="alert" className="error">{t(error)}</p>}
    {busy&&<p role="status"><Icon name="refresh" className="loading-spin"/> {t(redo?'Restoring commit…':'Undoing commit…')}</p>}
    <div className="form-actions"><button disabled={busy} onClick={onClose}>{t('Cancel')}</button><button className="primary" disabled={busy||loading||!preview||Boolean(preview.blockedReason)||(!redo&&!acknowledged)} onClick={()=>void apply()}>{t(redo?'Restore commit':'Undo and keep changes')}</button></div>
  </dialog>;
}
