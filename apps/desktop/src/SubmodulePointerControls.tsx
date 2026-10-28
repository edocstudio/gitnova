import { useEffect, useState } from 'react';
import { api, errorMessage } from './api';
import type { NativePath, OperationReceipt, SubmodulePointerPreview } from './contracts';
import { t } from './i18n';
export function SubmodulePointerControls({ membershipId, path, stage, disabled, onBusyChange, onDone, onClose, onFailure }: {
  membershipId: string; path: NativePath; stage: boolean; disabled: boolean;
  onBusyChange: (value: boolean) => void; onDone: (receipt: OperationReceipt) => Promise<void>; onClose: () => void; onFailure: (id: string, message: string) => void;
}) {
  const [preview, setPreview] = useState<SubmodulePointerPreview | null>(null); const [error, setError] = useState('');
  const [loading, setLoading] = useState(true); const [writing, setWriting] = useState(false);
  useEffect(() => { let cancelled=false;queueMicrotask(()=>{if(cancelled)return;void api.previewSubmodulePointer(membershipId,path,stage).then(value=>{if(!cancelled){if(value.version!==1)throw new Error('Restart GitNova before changing submodule pointers. Nothing has been applied.');setPreview(value);}}).catch(error=>{if(!cancelled)setError(errorMessage(error));}).finally(()=>{if(!cancelled)setLoading(false);});});return()=>{cancelled=true;}; }, [membershipId,path.hex,stage]);
  const apply=async()=>{if(!preview||preview.blockedReason||disabled||writing)return;setWriting(true);onBusyChange(true);setError('');try{const receipt=await api.changeSubmodulePointer(membershipId,path,stage,preview.token);setPreview(null);await onDone(receipt);if(receipt.status==='succeeded')onClose();else setError(`${t(receipt.summary)} ${receipt.recovery??''}`);}catch(error){setPreview(null);setError(errorMessage(error));onFailure(membershipId,errorMessage(error));}finally{setWriting(false);onBusyChange(false);}};
  return <div className="submodule-pointer-review" role="region" aria-label={t('Review submodule pointer')}>
    <strong>{stage?t('Stage submodule pointer'):t('Unstage submodule pointer')}</strong>
    {(loading||writing)&&<p role="status">{writing?t('Updating parent index…'):t('Reading parent index and submodule checkout…')}</p>}
    {preview&&<><p>{t('Parent branch')}: <b>{preview.parentStatus.branch??t('Detached HEAD')}</b></p><div className="pointer-transition"><code>{preview.submodule.indexOid.slice(0,12)}</code><span>→</span><code>{preview.targetOid.slice(0,12)}</code></div><p>{t('Updates the parent index only. The submodule checkout, files and branches stay in place.')}</p>{preview.submodule.dirty&&<p className="notice">{t('Uncommitted files inside this submodule stay outside the parent commit.')}</p>}{preview.blockedReason&&<p className="notice" role="status">{t(preview.blockedReason)}</p>}</>}
    {error&&<p className="inline-error" role="alert">{t(error)}</p>}
    <div className="submodule-confirm"><button disabled={writing} onClick={onClose}>{t('Back')}</button><button className="primary" disabled={disabled||loading||writing||!preview||Boolean(preview.blockedReason)} onClick={()=>void apply()}>{stage?t('Stage pointer'):t('Unstage pointer')}</button></div>
  </div>;
}
