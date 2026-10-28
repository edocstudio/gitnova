import { useEffect, useMemo, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import type { WorkProgress, DiscardPreview, NativePath, OperationReceipt, WorkingChanges } from './contracts';
import { t } from './i18n';
import { Icon } from './Icons';
export function DiscardControls({ membershipId, changes, operationToken, initialPath, initialScope, disabled, onBusyChange, onDone, onClose, onInspect }: {
  membershipId: string; changes: WorkingChanges; operationToken?: string; initialPath: NativePath | null; initialScope: 'unstaged' | 'all'; disabled: boolean;
  onBusyChange: (value: boolean) => void; onDone: (receipt: OperationReceipt) => Promise<void>; onClose: () => void; onInspect: (path: NativePath, staged: boolean) => void;
}) {
  const [progress,setProgress]=useState<WorkProgress>({phase:'preparing',completed:0,total:0,unit:'files'});
  const phaseLabels:Record<string,string>={preparing:'Preparing…',checking:'Checking files…',validating:'Validating current changes…',rechecking:'Final check before backup…','backing-up':'Saving recovery copies…','backing-up-file':'Copying recovery data…',discarding:'Discarding selected files…',finalizing:'Finalizing changes…',refreshing:'Refreshing repository…'};
  const region = useRef<HTMLElement>(null);
  useEffect(() => { requestAnimationFrame(() => region.current?.querySelector('select')?.focus()); }, []);
  const [scope, setScope] = useState(initialScope); const [selected, setSelected] = useState(() => initialPath ? [initialPath.hex] : [...new Set(changes.files.map(f => f.path.hex))]);
  const [reviewedContext, setReviewedContext] = useState(''); const [reviewedOperation, setReviewedOperation] = useState('');
  const [preview, setPreview] = useState<DiscardPreview | null>(null); const [loading, setLoading] = useState(false); const [writing, setWriting] = useState(false); const [error, setError] = useState(''); const inFlight = useRef(false); const previewQueue=useRef<Promise<void>>(Promise.resolve());
  const files = useMemo(() => [...new Map(changes.files.filter(f => scope === 'all' || f.unstaged).map(f => [f.path.hex, f])).values()], [changes.token, scope]);
  const paths = useMemo(() => files.filter(f => selected.includes(f.path.hex)).map(f => f.path), [files, selected]);
  useEffect(() => {
    let cancelled = false; setPreview(null); setError(''); setLoading(paths.length > 0); setProgress({phase:'preparing',completed:0,total:0,unit:'files'});
    if (!paths.length) return;
    const run = async () => {
      if (cancelled) return;
      try { const value=await api.previewDiscard(membershipId,{paths,scope,id:''},operationToken,event=>{if(!cancelled)setProgress(event);}); if(!cancelled){setReviewedContext(changes.token);setReviewedOperation(operationToken ?? '');setPreview(value);} }
      catch(e){if(!cancelled)setError(errorMessage(e));}
      finally{if(!cancelled)setLoading(false);}
    };
    previewQueue.current=previewQueue.current.then(run,run);
    return () => { cancelled = true; };
  }, [membershipId, paths, scope, operationToken]);
  const valid = Boolean(preview && reviewedOperation === (operationToken ?? '') && reviewedContext === changes.token && preview.request.scope === scope && preview.request.paths.length === paths.length && preview.request.paths.every((path, i) => path.hex === paths[i].hex && path.encoding === paths[i].encoding));
  const apply = async () => {
    if (!valid || !preview || preview.blockedReason || inFlight.current || disabled) return;
    inFlight.current = true; setProgress({phase:'preparing',completed:0,total:0,unit:'files'}); setWriting(true); onBusyChange(true);
    let result: OperationReceipt;
    try { result = await api.discardChanges(membershipId, preview.request, preview.token,operationToken,setProgress); }
    catch (e) { setError(errorMessage(e)); setPreview(null); inFlight.current = false; setWriting(false); onBusyChange(false); return; }
    try { setProgress({phase:'refreshing',completed:0,total:0,unit:'files'}); await onDone(result); onClose(); }
    catch { setError('Discard result recorded. Refresh Working changes to inspect the current state.'); setPreview(null); }
    finally { inFlight.current = false; setWriting(false); onBusyChange(false); }
  };
  return <section ref={region} className="discard-review" role="region" aria-label={t('Review discard')}>
    <strong>{t('Discard selected changes?')}</strong><p>{t('On branch {0}', [changes.status.branch ?? t('Detached HEAD')])}</p>
    {operationToken && <p className="notice">{t('Discard keeps this operation pending. To cancel the operation, use Review abort instead.')}</p>}
    <label htmlFor="discard-scope">{t('What to discard')}</label><select id="discard-scope" value={scope} disabled={writing || disabled} onChange={e => setScope(e.target.value as 'all' | 'unstaged')}>
      <option value="unstaged">{t('Unstaged only · keep staged content')}</option><option value="all">{changes.status.head ? t('Staged and unstaged · restore HEAD') : t('Staged and unstaged · restore empty tree')}</option>
    </select>
    <p>{scope === 'unstaged' ? t('Restores selected files to their staged version. Staging stays unchanged.') : changes.status.head ? t('Restores selected files and staging to HEAD. Selected new files are removed.') : t('Removes selected new files and their staged entries. No commit exists yet.')}</p>
    <div className="discard-files">{files.map(file => <div key={file.path.hex}><label><input type="checkbox" disabled={writing || disabled} checked={selected.includes(file.path.hex)} onChange={e => setSelected(old => e.target.checked ? [...old, file.path.hex] : old.filter(hex => hex !== file.path.hex))} aria-label={t('Discard {0}', [file.path.display])} /><span title={file.path.display}>{file.path.display}</span></label><button disabled={writing || disabled} onClick={() => onInspect(file.path, scope === 'all' && file.staged)}>{t('Inspect')}</button></div>)}</div>
    <p>{paths.length === 1 ? t('1 selected file') : t('{0} selected files', [paths.length])}</p><p>{t('Original files and staging are saved in a private recovery folder before changes. Configured Git filters may run.')}</p>
    {(loading || writing) && <div className="discard-progress" aria-busy="true"><div role="status"><Icon name="refresh" className="loading-spin" size={14}/><span>{t(phaseLabels[progress.phase] ?? 'Working…')}</span></div><progress aria-label={t('Current step progress')} max={progress.total>0?progress.total:undefined} value={progress.total>0?Math.min(progress.completed,progress.total):undefined}/>{progress.total>0 && <small>{Math.floor(Math.min(progress.completed,progress.total)*100/progress.total)}% · {progress.unit==='bytes' ? `${(progress.completed/1048576).toFixed(1)} / ${(progress.total/1048576).toFixed(1)} MiB` : t('{0} / {1} files',[progress.completed,progress.total])}</small>}</div>}
    {error && <div className="inline-error" role="alert">{t(error)}</div>}
    {preview?.blockedReason && <div className="notice" role="status">{t(preview.blockedReason)}<p>{t('Uncheck this path to discard other files. Submodule checkouts use Restore recorded version separately.')}</p></div>}
    <div className="compose-footer"><button disabled={writing} onClick={onClose}>{t('Cancel')}</button><button className="danger-action" disabled={disabled || writing || loading || !valid || !preview || Boolean(preview.blockedReason)} onClick={() => void apply()}>{writing ? t('Discarding…') : paths.length === 1 ? t('Discard 1 file') : t('Discard {0} files', [paths.length])}</button></div>
  </section>;
}
