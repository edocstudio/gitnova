import {useEffect,useId,useRef,useState} from 'react';
import {api,errorMessage} from './api';
import type {NativePath,OperationReceipt,SubmodulePointerPreview} from './contracts';
import {t} from './i18n';
import {useRecoveredLockError} from './useRecoveredLockError';

export function SubmodulePointerControls({membershipId,path,stage,disabled,onBusyChange,onDone,onClose,onFailure}:{
 membershipId:string;path:NativePath;stage:boolean;disabled:boolean;
 onBusyChange:(value:boolean)=>void;onDone:(receipt:OperationReceipt)=>Promise<void>;onClose:()=>void;onFailure:(id:string,message:string)=>void;
}){
 const dialog=useRef<HTMLDialogElement>(null),cancel=useRef<HTMLButtonElement>(null),inFlight=useRef(false);
 const titleId=useId();
 const [preview,setPreview]=useState<SubmodulePointerPreview|null>(null),[error,setError]=useState('');
 const [loading,setLoading]=useState(true),[writing,setWriting]=useState(false),[revision,setRevision]=useState(0);
 useRecoveredLockError(membershipId,setError);
 useEffect(()=>{const node=dialog.current;const previous=document.activeElement as HTMLElement|null;node?.showModal();cancel.current?.focus();return()=>{node?.close();requestAnimationFrame(()=>{if(previous?.isConnected&&!previous.hasAttribute('disabled'))previous.focus();else document.querySelector<HTMLButtonElement>('.working-row')?.focus();});};},[]);
 useEffect(()=>{
  let cancelled=false;setPreview(null);setError('');setLoading(true);
  void api.previewSubmodulePointer(membershipId,path,stage).then(value=>{
   if(cancelled)return;
   if(value.version!==1)throw new Error('Restart GitNova before changing submodule pointers. Nothing has been applied.');
   setPreview(value);
  }).catch(e=>{if(!cancelled)setError(errorMessage(e));}).finally(()=>{if(!cancelled)setLoading(false);});
  return()=>{cancelled=true;};
 },[membershipId,path.hex,stage,revision]);
 const apply=async()=>{
  if(!preview||preview.blockedReason||disabled||inFlight.current)return;
  inFlight.current=true;setWriting(true);onBusyChange(true);setError('');
  try{const receipt=await api.changeSubmodulePointer(membershipId,path,stage,preview.token);setPreview(null);await onDone(receipt);if(receipt.status==='succeeded')onClose();else setError(`${t(receipt.summary)} ${receipt.recovery??''}`);}
  catch(e){setPreview(null);setError(errorMessage(e));onFailure(membershipId,errorMessage(e));}
  finally{inFlight.current=false;setWriting(false);onBusyChange(false);}
 };
 return <dialog ref={dialog} className="settings-dialog submodule-pointer-dialog" aria-labelledby={titleId} onCancel={event=>{event.preventDefault();event.stopPropagation();if(!inFlight.current)onClose();}} onKeyDown={event=>event.stopPropagation()}>
  <div className="submodule-pointer-review" role="region" aria-label={t('Review submodule pointer')} aria-busy={loading||writing}>
   <h2 id={titleId}>{stage?t('Stage submodule pointer'):t('Unstage submodule pointer')}</h2>
   <p className="pointer-path"><strong>{preview?.submodule.relativePath.display??path.display}</strong></p>
   <p>{stage?t('Include the checked-out submodule commit in the next parent commit.'):t('Remove this pointer change from the next parent commit.')}</p>
   {(loading||writing)&&<p className="loading-status" role="status">{writing?t('Updating parent index…'):t('Reading parent index and submodule checkout…')}</p>}
   {preview&&<>
    <p>{t('Parent branch')}: <b>{preview.parentStatus.branch??t('Detached HEAD')}</b></p>
    <div className="pointer-transition"><div><small>{t('Parent index')}</small><code title={preview.submodule.indexOid}>{preview.submodule.indexOid.slice(0,12)}</code></div><span aria-hidden="true">→</span><div><small>{stage?t('Checked out'):t('Parent commit')}</small><code title={preview.targetOid}>{preview.targetOid.slice(0,12)}</code></div></div>
    <p className="hint">{t('Updates the parent index only. The submodule checkout, files and branches stay in place.')}</p>
    {preview.submodule.dirty&&<p className="notice">{t('Uncommitted files inside this submodule stay outside the parent commit.')}</p>}
    {preview.blockedReason&&<p className="notice" role="alert">{t(preview.blockedReason)}</p>}
   </>}
   {error&&<p className="inline-error" role="alert">{t(error)}</p>}
   {!loading&&!writing&&(!preview||preview.blockedReason)&&<button disabled={disabled} onClick={()=>setRevision(value=>value+1)}>{t('Check again')}</button>}
   <div className="submodule-confirm"><button ref={cancel} disabled={writing} onClick={onClose}>{t('Cancel')}</button><button className="primary" disabled={disabled||loading||writing||!preview||Boolean(preview.blockedReason)} onClick={()=>void apply()}>{writing?t('Updating parent index…'):stage?t('Stage pointer'):t('Unstage pointer')}</button></div>
  </div>
 </dialog>;
}
