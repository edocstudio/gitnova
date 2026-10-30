import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import { t, useLocale } from './i18n';
import { stashTitle } from './graph-state';
import type { MergeState, NativePath, OperationReceipt, StashEntry, StashKind, StashList, StashPreview, StashRequest, WorkingChanges } from './contracts';
import type { WorkingDiff } from './WorkingDetails';
import { Icon } from './Icons';
const request = (kind: StashKind, entry: Pick<StashEntry, 'oid' | 'index'> | null, message = '', includeUntracked = true, restoreIndex = false): StashRequest => ({ kind, index: entry?.index ?? null, oid: entry?.oid ?? null, message, includeUntracked: kind === 'push' && includeUntracked, restoreIndex, id: '' });
const popRemovalBlocked = (reason: string) => /Pop\/Drop|Packed stash refs|Reference-transaction hooks|stash metadata|reflog disagree/.test(reason);
export function StashControls({ operationState, initialKind, initialEntry, membershipId, changes, disabled, onBusyChange, onOperation, onFailure, onRefresh, onDiffChange, onClose }: {
  operationState?: MergeState;
  initialKind?: 'push' | 'pop'; initialEntry?: Pick<StashEntry, 'oid' | 'index'>; membershipId: string; changes: WorkingChanges | null; disabled: boolean; onBusyChange: (value: boolean) => void; onOperation: (receipt: OperationReceipt) => void; onFailure: (id: string, message: string) => void; onRefresh: () => Promise<void>; onDiffChange: (value: WorkingDiff) => void; onClose: () => void;
}) {
  const locale = useLocale(); const root = useRef<HTMLDivElement>(null);
  const [list, setList] = useState<StashList | null>(null); const [preview, setPreview] = useState<StashPreview | null>(null);
  const [message, setMessage] = useState(''); const [includeUntracked, setIncludeUntracked] = useState(true);
  const [loading, setLoading] = useState(false); const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [result, setResult] = useState<OperationReceipt | null>(null); const [failedRequest, setFailedRequest] = useState<StashRequest | null>(null);
  const [patch,setPatch]=useState<{key:string;diff:string;truncated:boolean}|null>(null);
  const [patchError,setPatchError]=useState<{key:string;message:string}|null>(null);
  const [file, setFile] = useState<NativePath | null>(null); const [indexView, setIndexView] = useState(false);
  const sequence = useRef(0); const inFlight = useRef(false); const mounted = useRef(true);
  const context = `${changes?.token ?? ''}:${operationState?.token ?? ''}`;
  const contextRef = useRef(context); contextRef.current = context;
  const [previewContext, setPreviewContext] = useState('');
  const reload = async () => { const value = await api.stashes(membershipId); if (mounted.current) setList(value); return value; };
  const inspect = async (value: StashRequest, preserveSelection = false) => {
    if (disabled || inFlight.current) return; const version = ++sequence.current; const startedContext = contextRef.current; setLoading(true); setError(''); setResult(null); setFailedRequest(null);
    try { const next = await api.previewStash(membershipId, value); if (version === sequence.current && mounted.current) { setPreview(next); setPreviewContext(startedContext); setFile(current => preserveSelection && current ? next.files.find(path => path.hex === current.hex) ?? next.files[0] ?? null : next.files[0] ?? null); if (!preserveSelection) setIndexView(false); } }
    catch (e) { if (version === sequence.current && mounted.current) { setError(errorMessage(e)); setFailedRequest(value); setPreview(null); } }
    finally { if (version === sequence.current && mounted.current) setLoading(false); }
  };
  useEffect(() => {
    mounted.current = true; let cancelled = false;
    void reload().then(value => {
      if (cancelled) return;
      if (initialEntry || initialKind === 'pop') {
        const entry = initialEntry ? value.entries.find(item => item.oid === initialEntry.oid && item.index === initialEntry.index) : value.entries[0];
        if (entry) void inspect(request('pop', entry)); else setError('This saved entry changed. Select it again from the graph or stash list.');
      } else if (initialKind === 'push') void inspect(request('push', null));
    }).catch(e => { if (!cancelled) setError(errorMessage(e)); });
    return () => { cancelled = true; mounted.current = false; sequence.current++; };
  }, [membershipId, initialEntry?.oid, initialEntry?.index, initialKind]);
  useEffect(() => {
    if (!preview || loading || busy || disabled || previewContext === context) return;
    // Read-only revalidation. Never replay Apply/Pop/Drop after a context change.
    void inspect(preview.request.kind === 'push'
      ? request('push', null, message, includeUntracked)
      : { ...preview.request, id: '' }, true);
  }, [context, previewContext, preview, loading, busy, disabled]);
  const patchKey=preview&&file?`${preview.token}:${file.hex}:${indexView}`:'';
  const activePatch=patch?.key===patchKey?patch:null;
  const activePatchError=patchError?.key===patchKey?patchError.message:'';
  const patchLoading=Boolean(preview&&file&&!activePatch&&!activePatchError);
  useEffect(()=>{
    let cancelled=false;setPatch(null);setPatchError(null);
    if(!preview||!file||loading)return;
    api.stashPatch(membershipId,preview.request,preview.token,file,indexView).then(value=>{if(!cancelled)setPatch({key:patchKey,...value});}).catch(e=>{if(!cancelled)setPatchError({key:patchKey,message:errorMessage(e)});});
    return()=>{cancelled=true;};
  },[membershipId,preview?.token,file?.hex,indexView,loading]);
  useEffect(() => {
    onDiffChange({ membershipId, loading:loading||patchLoading, source: { diff: activePatch?.diff??'', truncated:activePatch?.truncated??false, truncationMessage:t('This file preview is partial. The stash operation includes the complete reviewed scope.'), label: t('Stash review'), title: indexView ? t('Saved index') : t('Saved working tree'), comparison: preview?.request.kind === 'push' ? t('Changes to save') : t('Captured changes'), before: preview?.request.kind === 'push' ? preview.head?.slice(0, 8) ?? '' : t('Saved base'), after: indexView ? t('Saved index') : t('Saved working tree'), selectedPath: file, emptyMessage: activePatchError?t(activePatchError):preview ? t('This path has no changes in the selected saved version.') : t('Choose saved work in the graph or save your current changes.') } });
  }, [locale, membershipId, preview, indexView, file, loading, patchLoading, activePatch, activePatchError, changes?.status.branch, onDiffChange]);
  useEffect(() => () => onDiffChange(null), [onDiffChange]);
  useEffect(() => { if (preview && !loading) requestAnimationFrame(() => root.current?.querySelector<HTMLButtonElement>('.stash-confirm')?.focus()); }, [preview?.request.id, loading]);
  const apply = async () => {
    if (!preview || preview.blockedReason || disabled || loading || inFlight.current) return;
    inFlight.current = true; setBusy(true); onBusyChange(true); setError('');
    try {
      const receipt = await api.applyStash(membershipId, preview.request, preview.token); onOperation(receipt); setPreview(null); setFile(null);
      try { await onRefresh(); await reload(); } catch (refreshError) { setError(errorMessage(refreshError)); setResult(receipt); return; }
      if (receipt.status === 'succeeded') onClose(); else setResult(receipt);
    } catch (e) { setError(errorMessage(e)); onFailure(membershipId, errorMessage(e)); setPreview(null); }
    finally { inFlight.current = false; if (mounted.current) setBusy(false); onBusyChange(false); }
  };
  const branch = preview?.branch ?? changes?.status.branch ?? t('Detached HEAD');
  const selectedEntry = list?.entries.find(entry => entry.oid === preview?.request.oid && entry.index === preview?.request.index);
  const push = preview?.request.kind === 'push'; const drop = preview?.request.kind === 'drop';
  const ownedOperationBlocked = Boolean(operationState?.sessionId && operationState.phase === 'pending' && preview?.blockedReason === 'Finish or recover the existing merge, rebase, cherry-pick or revert in Git before writing here');
  const operationLabel = operationState?.operation === 'cherryPick' ? t('Cherry-pick') : operationState?.operation === 'revert' ? t('Revert') : t('Merge');
  const settingsChanged = Boolean(push && preview && (preview.request.message !== message || preview.request.includeUntracked !== includeUntracked));
  const label = push ? t('Stash on {0}', [branch]) : drop ? t('Remove saved stash') : preview?.request.kind === 'apply' ? t('Apply on {0}', [branch]) : t('Pop on {0}', [branch]);
  const updateKind = (kind: StashKind) => { if (preview) { const next = { ...preview.request, kind, id: '' }; setPreview({ ...preview, request: next }); void inspect(next); } };
  const updateIndex = (restoreIndex: boolean) => { if (preview) { const next = { ...preview.request, restoreIndex, id: '' }; setPreview({ ...preview, request: next }); void inspect(next); } };
  return <div ref={root} className="stash-controls">
    {error && <div className="inline-error" role="alert">{t(error)}{failedRequest?.kind === 'pop' && popRemovalBlocked(error) && <div className="stash-secondary"><button disabled={busy || loading || disabled} onClick={() => void inspect({ ...failedRequest, kind: 'apply', id: '' })}>{t('Restore and keep stash')}</button></div>}</div>}
    {result && <div className="notice" role="status"><strong>{t(result.summary)}</strong><p>{result.status === 'conflicted' ? t('Saved work is retained for recovery. Resolve stash conflicts with Git before retrying.') : t('Inspect local files, saved entries and recovery metadata before retrying this operation.')}</p><button disabled={busy || disabled} onClick={onClose}>{t('Review working changes')}</button><details className="stash-options"><summary>{t('Recovery details')}</summary>{result.planned?.type === 'stash' && <code className="recovery-ref">{result.planned.preview.recoveryRef}</code>}</details></div>}
    {loading && <p className="quiet-state" role="status">{t('Preparing saved work…')}</p>}
    {!preview && !loading && !result && <>
      <button className="primary" disabled={busy || disabled || !changes?.status.dirty} onClick={() => void inspect(request('push', null, message, includeUntracked))}>{t('Stash changes…')}</button>
      <div className="stash-list-heading"><strong>{t('Saved stashes')} {list ? `(${list.entries.length}${list.more ? '+' : ''})` : ''}</strong><button className="icon-button" aria-label={t('Refresh stashes')} disabled={busy || disabled} onClick={() => void reload().catch(e => setError(errorMessage(e)))}><Icon name="refresh" /></button></div>
      <div className="stash-list">{list?.entries.map(entry => <button key={`${entry.index}:${entry.oid}`} disabled={busy || disabled} onClick={() => void inspect(request('pop', entry))} title={entry.message}><strong>{`stash@{${entry.index}}`}</strong><span>{stashTitle(entry.message)}</span></button>)}{list && !list.entries.length && <p className="quiet-state">{t('No saved stashes.')}</p>}{list?.more && <p className="notice">{t('Showing the latest 200 stashes. Inspect older entries with Git.')}</p>}</div>
    </>}
    {activePatchError&&<div className="inline-error" role="alert">{t(activePatchError)}<button disabled={busy||disabled||loading} onClick={()=>{if(preview)void inspect({...preview.request,id:''},true);}}>{t('Review updates')}</button></div>}
    {preview && <div className="stash-preview">
      <div className="stash-destination"><small>{push ? t('Save current changes from') : drop ? t('Remove saved work') : t('Restore saved changes onto')}</small><strong>{drop ? `stash@{${preview.request.index}}` : branch}</strong>{selectedEntry && <p title={selectedEntry.message}><code>{`stash@{${selectedEntry.index}}`}</code> · {stashTitle(selectedEntry.message)}</p>}<small>{t('{0} files', [preview.files.length])}</small></div>
      <p>{push ? t('Save these changes so you can switch branches. New files are included unless you turn them off below.') : drop ? t('Remove this saved entry without changing working files. Its commit is retained by the recovery ref.') : preview.request.kind === 'apply' ? t('Restore onto this branch and keep the saved stash.') : t('Restore onto this branch, then remove the stash if successful.')}</p>
      {preview.blockedReason && <div className="notice" role="status">{ownedOperationBlocked ? t('{0} is still active on {1}, even with zero changed files. Finish or abort it before restoring saved work.', [operationLabel, branch]) : t(preview.blockedReason)}<div className="stash-secondary">{preview.request.kind === 'pop' && popRemovalBlocked(preview.blockedReason) ? <button disabled={busy || disabled || loading} onClick={() => updateKind('apply')}>{t('Restore and keep stash')}</button> : <button disabled={busy || disabled} onClick={onClose}>{ownedOperationBlocked ? t('Review pending {0}', [operationLabel]) : t('Review working changes')}</button>}</div></div>}
      {(preview.truncated||preview.indexTruncated)&&<p className="hint">{t('The complete file list is included. Code loads per file; large files may have a partial preview. No files are excluded because of the code preview limit.')}</p>}
      <div className="stash-review-files" aria-label={t('Stash files')}>{preview.files.map(path => <button key={path.hex} aria-pressed={file?.hex === path.hex} onClick={() => setFile(path)}><Icon name="file" size={14}/><span>{path.display}</span></button>)}</div>
      <div className="graph-action-footer"><button disabled={busy} onClick={onClose}>{t('Cancel')}</button><button className={`stash-confirm ${drop ? 'danger-action' : 'primary'}`} disabled={busy || disabled || loading || patchLoading || Boolean(activePatchError) || Boolean(preview.blockedReason && !settingsChanged)} onClick={() => { if (settingsChanged) void inspect(request('push', null, message, includeUntracked)); else void apply(); }}>{busy ? t('Applying…') : settingsChanged ? t('Review updates') : label}</button></div>
      <details className="stash-options"><summary>{t('Options and recovery')}</summary>
        {push && <><label htmlFor="stash-message">{t('Stash message')} <span>{t('optional')}</span></label><input id="stash-message" value={message} maxLength={256} disabled={busy || disabled || loading} onChange={event => setMessage(event.target.value)} /><label className="ref-checkbox"><input type="checkbox" checked={includeUntracked} disabled={busy || disabled || loading} onChange={event => setIncludeUntracked(event.target.checked)}/>{t('Include untracked files')}</label></>}
        {!push && !drop && <><label className="ref-checkbox"><input type="checkbox" checked={preview.request.kind === 'apply'} disabled={busy || disabled || loading} onChange={event => updateKind(event.target.checked ? 'apply' : 'pop')}/>{t('Keep the stash after restoring')}</label><label className="ref-checkbox"><input type="checkbox" checked={preview.request.restoreIndex} disabled={busy || disabled || loading} onChange={event => updateIndex(event.target.checked)}/>{t('Restore the saved staging state')}</label></>}
        <div className="view-switch" role="group" aria-label={t('Saved comparison')}><button aria-pressed={!indexView} onClick={() => setIndexView(false)}>{t('Saved working tree')}</button><button aria-pressed={indexView} onClick={() => setIndexView(true)}>{t('Saved index')}</button></div>
        {!push && !drop && <button className="danger-action" disabled={busy || disabled || loading} onClick={() => updateKind('drop')}>{t('Remove saved stash…')}</button>}
        {selectedEntry && <p>{selectedEntry.message}<br/><code>{`stash@{${selectedEntry.index}}`} · {selectedEntry.oid}</code></p>}
        <p>{t('Review shows the captured delta. Applying to this checkout may produce conflicts; configured filters or hooks can run.')}</p><small>{t('Recovery ref')}</small><code className="recovery-ref">{preview.recoveryRef}</code>
      </details>
    </div>}
  </div>;
}
