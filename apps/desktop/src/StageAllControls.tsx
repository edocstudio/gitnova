import {useEffect,useState,useRef} from 'react';
import {api,errorMessage} from './api';
import {t} from './i18n';
import type {OperationReceipt,StageAllPreview} from './contracts';
export function StageAllControls({membershipId,onClose,onBusyChange,onDone}:{membershipId:string;onClose:()=>void;onBusyChange:(busy:boolean)=>void;onDone:(receipt:OperationReceipt)=>Promise<void>}) {
 const [preview,setPreview]=useState<StageAllPreview|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);const writing=useRef(false);
 useEffect(()=>{let cancelled=false;api.previewStageAll(membershipId).then(p=>{if(!cancelled)setPreview(p);}).catch(e=>{if(!cancelled)setError(errorMessage(e));});return()=>{cancelled=true;};},[membershipId]);
 const apply=async()=>{if(!preview||preview.blockedReason||writing.current)return;writing.current=true;setBusy(true);onBusyChange(true);setError('');try{const receipt=await api.stageAll(membershipId,preview.token);await onDone(receipt);if(receipt.status==='succeeded')onClose();else{setError(receipt.summary);setPreview(null);}}catch(e){setError(errorMessage(e));setPreview(null);}finally{writing.current=false;setBusy(false);onBusyChange(false);}};
 return <div className="commit-compose stage-all-review" aria-label={t('Review Stage all')}>
  <strong>{t('Stage all files')}</strong>
  {!preview&&!error&&<p role="status">{t('Checking file contents and staging scope…')}</p>}
  {preview&&<><p>{t('{0} files will be staged. Working files stay in place.',[preview.files.length])}</p><div className="stage-all-paths">{preview.files.map(file=><code key={file.path.hex}>{file.path.display}</code>)}</div>
  {!!preview.excluded.length&&<><p className="notice">{t('{0} paths need separate review and will stay unchanged.',[preview.excluded.length])}</p><div className="stage-all-paths">{preview.excluded.map(file=><div key={file.path.hex}><code>{file.path.display}</code><small>{t(file.blockedReason??'Needs separate review')}</small></div>)}</div></>}
  <p className="hint">{t('Only these reviewed paths are included. Ignored files stay excluded. Configured Git clean filters may run after confirmation.')}</p>{preview.blockedReason&&<p className="notice">{t(preview.blockedReason)}</p>}</>}
  {error&&<p className="inline-error" role="alert">{t(error)}</p>}<div className="compose-footer"><button disabled={busy} onClick={onClose}>{t('Back to changes')}</button><button className="primary" disabled={busy||!preview||!!preview.blockedReason||!preview.files.length} onClick={()=>void apply()}>{busy?t('Staging files…'):t('Stage {0} files',[preview?.files.length??0])}</button></div>
 </div>;
}
