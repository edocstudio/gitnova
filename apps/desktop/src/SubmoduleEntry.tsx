import { t } from './i18n';
import { useEffect, useRef, useState } from 'react';
import {SubmoduleSync} from './RelatedMaintenance';
import { api, errorMessage } from './api';
import type { NativePath, OperationReceipt, Submodule, SubmoduleUpdatePreview } from './contracts';
import { SubmodulePointerControls } from './SubmodulePointerControls';
import { Icon } from './Icons';
export type SubmoduleIntent = { action: 'stage' | 'unstage' | 'restore'; nonce: number };
export function SubmoduleEntry({ detailed = false, pointerIntent, submodule: sub, membershipId, disabled, onOpen, onBusyChange, onDone, onFailure }: {
  detailed?: boolean; pointerIntent?: SubmoduleIntent | null; submodule: Submodule; membershipId: string; disabled: boolean; onOpen: (path: NativePath) => void;
  onBusyChange: (value: boolean) => void; onDone: (receipt: OperationReceipt) => Promise<void>; onFailure: (id: string, message: string) => void;
}) {
  const [pointerStage, setPointerStage] = useState<{stage:boolean;nonce:number}|null>(null);
  const [preview, setPreview] = useState<SubmoduleUpdatePreview | null>(null); const [loading, setLoading] = useState(false); const [writing, setWriting] = useState(false); const [error, setError] = useState('');
  const sequence = useRef(0); const inFlight = useRef(false);
  useEffect(() => () => { sequence.current++; }, []);
  const review = async () => {
    const request = ++sequence.current; setPointerStage(null); setError(''); setLoading(true); setPreview(null);
    try { const value = await api.previewSubmoduleUpdate(membershipId, sub.path, sub.initialized ? 'restore' : 'initialize'); if (request === sequence.current) setPreview(value); }
    catch (e) { if (request === sequence.current) setError(errorMessage(e)); }
    finally { if (request === sequence.current) setLoading(false); }
  };
  const apply = async () => {
    if (!preview || preview.blockedReason || inFlight.current || disabled) return;
    inFlight.current = true; setWriting(true); onBusyChange(true);
    try { const receipt = await api.updateSubmodule(membershipId, sub.path, preview.mode, preview.token); setPreview(null); await onDone(receipt); }
    catch (e) { setPreview(null); onFailure(membershipId, errorMessage(e)); }
    finally { inFlight.current = false; setWriting(false); onBusyChange(false); }
  };
  useEffect(() => { if(!pointerIntent)return;if(pointerIntent.action==='restore'){void review();}else{sequence.current++;setPreview(null);setLoading(false);setPointerStage({stage:pointerIntent.action==='stage',nonce:pointerIntent.nonce});} }, [pointerIntent?.nonce]);
  const pointer = (stage:boolean) => {sequence.current++;setPreview(null);setLoading(false);setPointerStage({stage,nonce:Date.now()});};
  const short = (oid: string | null) => oid?.slice(0, 8) ?? t('None');
  return <div className="submodule-entry" style={{ marginLeft: Math.min(sub.depth, 4) * 10 }}>
    {detailed && (pointerStage || preview) ? <strong className="submodule-review-name">{sub.relativePath.display}</strong> : detailed ? <div className="submodule-summary"><strong><Icon name="layers" size={16}/>{sub.relativePath.display}<span className="submodule-type">{t('Submodule')}</span></strong><div className="submodule-states">{!sub.initialized ? <span>{t('Not initialized')}</span> : sub.indexOid!==sub.checkedOutOid&&<span>{t('Commit changed')}</span>}{sub.headOid!==sub.indexOid&&<span>{t('Pointer staged')}</span>}{sub.initialized&&<span>{sub.dirty===null?t('Status unavailable'):sub.dirty?t('Uncommitted changes inside'):t('Files inside are clean')}</span>}</div><dl><dt>{t('Parent commit')}</dt><dd><code>{short(sub.headOid)}</code></dd><dt>{t('Parent index')}</dt><dd><code>{short(sub.indexOid)}</code></dd><dt>{t('Submodule checkout')}</dt><dd><code>{short(sub.checkedOutOid)}</code>{sub.detached&&<small>{t('Detached HEAD')}</small>}</dd></dl>
      <div className="submodule-main-actions"><button className={sub.dirty?'primary submodule-open':'submodule-open'} disabled={disabled||writing||!sub.initialized||Boolean(sub.inspectionError)} title={sub.path.display} onClick={()=>onOpen(sub.path)}><Icon name="layers" size={14}/>{t('Open submodule')}</button>{sub.initialized&&sub.indexOid!==sub.checkedOutOid&&<button disabled={disabled||writing||loading} onClick={()=>pointer(true)}>{t('Stage pointer…')}</button>}{sub.headOid!==sub.indexOid&&<button disabled={disabled||writing||loading} onClick={()=>pointer(false)}>{t('Unstage pointer…')}</button>}{!sub.inspectionError && (!sub.initialized || sub.indexOid !== sub.checkedOutOid) && <button className={!sub.initialized ? 'primary' : undefined} disabled={disabled || loading || writing} onClick={() => void review()}><Icon name={sub.initialized ? 'back' : 'plus'} size={13}/>{sub.initialized ? t("Restore recorded…") : t("Initialize…")}</button>}</div>
      {sub.dirty&&<p className="hint">{t('Commit or discard files inside the submodule. Stage its commit pointer here when ready.')}</p>}
    </div> : <>    <button className="submodule-open" disabled={disabled || writing || !sub.initialized || Boolean(sub.inspectionError)} title={sub.path.display} onClick={() => onOpen(sub.path)}><span><Icon name="layers" size={14} />{sub.relativePath.display}</span><small>{sub.initialized ? t("Recorded {0} · checkout {1}{2}", [short(sub.indexOid), short(sub.checkedOutOid), sub.dirty ? t(" · uncommitted work") : sub.detached ? t(" · detached") : '']) : t("Not initialized")}</small>{sub.parentRelationId && <small>{t("Inside")} {sub.parentPath.display}</small>}{sub.headOid !== sub.indexOid && <small>{t("Parent HEAD")} {short(sub.headOid)} {t("· recorded pointer is staged")}</small>}</button></>}
    {pointerStage&&<SubmodulePointerControls key={pointerStage.nonce} membershipId={membershipId} path={sub.path} stage={pointerStage.stage} disabled={disabled} onBusyChange={onBusyChange} onDone={onDone} onFailure={onFailure} onClose={()=>setPointerStage(null)}/>}

    {detailed&&sub.initialized&&!pointerStage&&!preview&&<SubmoduleSync path={sub.path} membershipId={membershipId} disabled={disabled||writing||loading} onBusyChange={onBusyChange} onDone={onDone}/>}
    {pointerStage ? null : sub.inspectionError ? <p className="hint">{t("Inspection incomplete:")} {sub.inspectionError}</p> : !detailed && (!sub.initialized || sub.indexOid !== sub.checkedOutOid) && <button className="submodule-action" disabled={disabled || loading || writing} onClick={() => void review()}><Icon name={sub.initialized ? 'back' : 'plus'} size={13} />{sub.initialized ? t("Restore recorded…") : t("Initialize…")}</button>}
    {loading && <p className="hint" role="status">{t("Inspecting recorded revision and local work…")}</p>}{error && <div className="inline-error" role="alert">{t(error)}</div>}
    {preview && <div className="submodule-preview" role="region" aria-label={t("Review submodule {0}", [sub.path.display])}><strong>{preview.mode === 'initialize' ? t("Initialize") : t("Restore recorded version")} · {sub.relativePath.display}</strong><dl><div><dt>{t("Parent branch")}</dt><dd>{preview.parentStatus.branch ?? t("Detached HEAD")}</dd></div><div><dt>{t("Recorded version")}</dt><dd><code>{preview.submodule.indexOid.slice(0, 12)}</code></dd></div><div><dt>{t("Checked out")}</dt><dd>{preview.submodule.checkedOutOid ? <code>{preview.submodule.checkedOutOid.slice(0, 12)}</code> : t("Not initialized")}</dd></div>{preview.url && preview.networkRequired && <div><dt>{t("Source")}</dt><dd>{preview.url}</dd></div>}{preview.sourceBase && preview.networkRequired && <div><dt>{t("Relative to")}</dt><dd>{preview.sourceBase}</dd></div>}</dl>
      <p>{preview.networkRequired ? t("Downloads this repository at its recorded revision. Initialize nested modules separately afterward.") : t("Restores the parent’s recorded revision. Local branches and nested checkouts stay in place.")}</p>{sub.headOid !== sub.indexOid && <p>{t("The recorded version follows the staged pointer in the parent index.")}</p>}
      {preview.preservedBy.length > 0 && <p title={preview.preservedBy.join(', ')}>{t("Current commits retained by")} {preview.preservedBy.slice(0, 3).map(ref => ref.replace('refs/heads/', '')).join(', ')}{preview.preservedBy.length > 3 ? t(" and {0} more local branches", [preview.preservedBy.length - 3]) : ''}.</p>}
      {preview.blockedReason && <div className="notice" role="status">{t(preview.blockedReason)}</div>}
      <div className="submodule-confirm"><button disabled={writing} onClick={() => setPreview(null)}>{t("Back")}</button><button className="primary" disabled={disabled || writing || Boolean(preview.blockedReason)} onClick={() => void apply()}>{writing ? preview.mode === 'initialize' ? t("Initializing…") : t("Restoring…") : preview.mode === 'initialize' ? t("Initialize recorded version") : t("Restore recorded version")}</button></div>
    </div>}
  </div>;
}
