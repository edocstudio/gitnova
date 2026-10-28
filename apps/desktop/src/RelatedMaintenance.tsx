import {useEffect,useRef,useState} from 'react';
import {api,errorMessage} from './api';
import {t} from './i18n';
import type {LinkedWorktree,NativePath,OperationReceipt,SubmoduleSyncPreview,WorktreeMaintenancePreview} from './contracts';
type Shared={membershipId:string;disabled:boolean;onBusyChange:(busy:boolean)=>void;onDone:(receipt:OperationReceipt)=>Promise<void>};
export function WorktreeMaintenance({tree,membershipId,disabled,onBusyChange,onDone}:Shared&{tree:LinkedWorktree}){
 const [preview,setPreview]=useState<WorktreeMaintenancePreview|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('');const flight=useRef(false);
 async function review(kind:'lock'|'unlock'|'repair'){
  if(flight.current||disabled)return;flight.current=true;setBusy(true);setError('');setPreview(null);
  try{setPreview(await api.previewWorktreeMaintenance(membershipId,{path:tree.path,kind,reason:kind==='lock'?'Locked from GitNova':''}));}catch(e){setError(errorMessage(e));}finally{flight.current=false;setBusy(false);}
 }
 async function apply(){if(!preview||preview.blockedReason||flight.current||disabled)return;flight.current=true;setBusy(true);onBusyChange(true);setError('');try{const r=await api.maintainWorktree(membershipId,preview.request,preview.token);setPreview(null);await onDone(r);}catch(e){setPreview(null);setError(errorMessage(e));}finally{flight.current=false;setBusy(false);onBusyChange(false);}}
 return <div className="related-maintenance"><details><summary>{t('Worktree metadata')}</summary><div className="submodule-main-actions"><button disabled={busy||disabled||tree.prunable} onClick={()=>void review(tree.locked?'unlock':'lock')}>{tree.locked?t('Unlock worktree…'):t('Lock worktree…')}</button><button disabled={busy||disabled} onClick={()=>void review('repair')}>{t('Repair metadata…')}</button></div>
 {busy&&<p role="status">{t('Checking worktree metadata…')}</p>}{error&&<p role="alert" className="notice">{t(error)}</p>}
 {preview&&<div className="submodule-preview" role="region" aria-label={t('Review worktree metadata')}><strong>{t(preview.request.kind==='lock'?'Lock worktree':preview.request.kind==='unlock'?'Unlock worktree':'Repair worktree metadata')}</strong><p><code>{preview.tree.path.display}</code></p><p>{t('Only Git metadata changes. Working files, branches and commits are kept.')}</p>{preview.request.kind==='repair'&&<><p>{t('Git repair checks the repository and these registered checkout locations:')}</p>{preview.affected.map(v=><code className="maintenance-path" key={v.path.hex}>{v.path.display}</code>)}</>}{preview.blockedReason&&<p className="notice">{t(preview.blockedReason)}</p>}<div className="submodule-confirm"><button disabled={busy} onClick={()=>setPreview(null)}>{t('Back')}</button><button disabled={busy||disabled||!!preview.blockedReason} onClick={()=>void apply()}>{t('Apply metadata change')}</button></div></div>}
 </details></div>;
}
export function SubmoduleSync({path,membershipId,disabled,onBusyChange,onDone}:Shared&{path:NativePath}){
 const [preview,setPreview]=useState<SubmoduleSyncPreview|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('');const flight=useRef(false);
 const reviewElement=useRef<HTMLDivElement>(null);
 useEffect(()=>{if(preview)reviewElement.current?.scrollIntoView({block:'start'});},[preview]);
 async function review(){if(flight.current||disabled)return;flight.current=true;setBusy(true);setError('');setPreview(null);try{setPreview(await api.previewSubmoduleSync(membershipId,path));}catch(e){setError(errorMessage(e));}finally{flight.current=false;setBusy(false);}}
 async function apply(){if(!preview||preview.blockedReason||flight.current||disabled)return;flight.current=true;setBusy(true);onBusyChange(true);setError('');try{const r=await api.syncSubmodule(membershipId,path,preview.token);setPreview(null);await onDone(r);}catch(e){setPreview(null);setError(errorMessage(e));}finally{flight.current=false;setBusy(false);onBusyChange(false);}}
 return <div className="related-maintenance"><button disabled={busy||disabled} onClick={()=>void review()}>{t('Sync URLs…')}</button>{busy&&<p role="status">{t('Checking submodule URLs…')}</p>}{error&&<p role="alert" className="notice">{t(error)}</p>}
 {preview&&<div ref={reviewElement} className="submodule-preview" role="region" aria-label={t('Review submodule URL synchronization')}><strong>{t('Sync selected submodule URLs')}</strong><p><code>{preview.submodule.path.display}</code></p><dl><dt>{t('Parent URL before')}</dt><dd>{preview.previousUrl??t('Not configured')}</dd>{preview.childRemote&&<><dt>{t('Child remote before: {0}',[preview.childRemote])}</dt><dd>{preview.childPreviousUrl??t('Not configured')}</dd></>}<dt>{t('Use URL from .gitmodules')}</dt><dd>{preview.url}</dd></dl><p>{t('Updates only this submodule’s configured URLs. No fetch, checkout or pointer change is performed.')}</p><p className="hint">{t('Configuration backup:')} <code>{preview.recoveryDir.display}</code></p>{preview.blockedReason&&<p className="notice">{t(preview.blockedReason)}</p>}<div className="submodule-confirm"><button disabled={busy} onClick={()=>setPreview(null)}>{t('Back')}</button><button disabled={busy||disabled||!!preview.blockedReason} onClick={()=>void apply()}>{t('Sync URLs')}</button></div></div>}
 </div>;
}
