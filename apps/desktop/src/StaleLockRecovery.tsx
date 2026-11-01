import {useEffect,useRef,useState} from 'react';
import {api,errorMessage} from './api';
import type {StaleLockPreview,NativePath} from './contracts';
import {t} from './i18n';
export function StaleLockRecovery({membershipId,disabled,onBusyChange,onRefresh,autoInspect=false,onRecovered}:{membershipId:string;disabled:boolean;onBusyChange:(busy:boolean)=>void;onRefresh:()=>Promise<void>;autoInspect?:boolean;onRecovered?:()=>void}){
 const [preview,setPreview]=useState<StaleLockPreview|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState(''),[archive,setArchive]=useState<NativePath|null>(null),[archiving,setArchiving]=useState(false);
 const alive=useRef(true);useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 const inspect=async()=>{if(busy||disabled)return;setBusy(true);setError('');setArchive(null);try{const p=await api.previewStaleLock(membershipId);if(alive.current)setPreview(p);}catch(e){if(alive.current){setPreview(null);setError(errorMessage(e));}}finally{if(alive.current)setBusy(false);}};
 const inspected=useRef(false);
 useEffect(()=>{if(autoInspect&&!disabled&&!inspected.current){inspected.current=true;void inspect();}},[autoInspect,disabled]);
 const recover=async()=>{if(busy||disabled||!preview?.exists||preview.blockedReason)return;setArchiving(true);setBusy(true);onBusyChange(true);setError('');try{const location=await api.recoverStaleLock(membershipId,preview.token);if(alive.current){setArchive(location);setPreview(null);}try{await onRefresh();window.dispatchEvent(new CustomEvent('gitnova-lock-recovered',{detail:membershipId}));if(alive.current)onRecovered?.();}catch{if(alive.current)setError('Lock archived. Refresh Working changes to inspect the current Git state.');}}catch(e){if(alive.current){setPreview(null);setError(errorMessage(e));}}finally{if(alive.current){setBusy(false);setArchiving(false);}onBusyChange(false);}};
 return <div className="stale-lock-recovery">
 {(!autoInspect||!preview||Boolean(preview.blockedReason))&&<button className={busy?"loading-status":undefined} disabled={busy||disabled} onClick={()=>void inspect()}>{busy?t(archiving?'Archiving lock…':'Inspecting lock…'):t(autoInspect?'Check lock again':'Recover stale lock…')}</button>}
 {error&&<p role="alert" className="notice">{t(error)}</p>}
 {preview&&<div role="region" aria-label={t('Review stale lock recovery')}>
 <code className="recovery-ref">{preview.path.display}</code>
 {!preview.exists?<p role="status">{t('No GitNova operation lock is present.')}</p>:<>
 {preview.ownerPid!=null&&<p>{t('Previous owner PID')}: {preview.ownerPid}</p>}
 <p>{t('Archives only this GitNova lock. Files, staging, branches and operation journals are preserved. No Git operation is resumed or retried.')}</p>
 {preview.blockedReason?<p role="alert" className="notice">{t(preview.blockedReason)}</p>:<p>{t('The recorded process is absent and no active Git operation was detected. Ownership is checked again on confirmation.')}</p>}
 <button disabled={busy||disabled||Boolean(preview.blockedReason)||!preview.token} onClick={()=>void recover()}>{t(autoInspect?'Recover and refresh':'Archive stale lock')}</button>
 </>}
 <button disabled={busy} onClick={()=>setPreview(null)}>{t('Cancel')}</button>
 </div>}
 {archive&&<div role="status"><p>{t('Stale lock archived. Inspect the current operation before trying another Git action.')}</p><code className="recovery-ref">{archive.display}</code></div>}
 </div>;
}
