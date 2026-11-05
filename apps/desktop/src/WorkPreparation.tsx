import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import { t } from './i18n';
import { DiffView } from './DiffView';
import type { OperationReceipt, WorkingChanges } from './contracts';
import type { WorkingDiff } from './WorkingDetails';

const PreparationStash = lazy(() => import('./StashControls').then(module => ({default: module.StashControls})));

// Only parent-work blockers are actionable here. Child dirtiness, locks and
// pending/conflicted operations must never be treated as a parent stash request.
export function needsWorkPreparation(reason?: string | null) {
  return Boolean(reason?.startsWith('Commit or preserve your uncommitted changes with Git before switching.') || reason?.startsWith('Commit or preserve uncommitted work before starting this operation.') || reason === 'Stash or commit current work before applying saved work. The working tree must be clean.');
}
export function WorkPreparation({membershipId, disabled, onBusyChange, onDone, onFailure, onReady, onCarry}: {
  membershipId: string; disabled: boolean; onBusyChange: (busy: boolean) => void;
  onDone: (receipt: OperationReceipt) => Promise<void>; onFailure: (id: string, message: string) => void;
  onReady: () => void; onCarry?: () => void;
}) {
  const [open, setOpen] = useState(true); const [changes, setChanges] = useState<WorkingChanges | null>(null);
  const [error, setError] = useState(''); const [diff, setDiff] = useState<WorkingDiff>(null);
  const [ready, setReady] = useState(false); const receipt = useRef<OperationReceipt | null>(null);
  const consumed = useRef(false); const refreshed = useRef(false);
  useEffect(() => {
    let cancelled = false;
    api.changes(membershipId).then(value => { if (!cancelled) setChanges(value); }).catch(e => { if (!cancelled) setError(errorMessage(e)); });
    return () => { cancelled = true; };
  }, [membershipId]);
  useEffect(() => {
    if (!ready || disabled || consumed.current) return;
    consumed.current = true; onReady();
  }, [ready, disabled, onReady]);
  const refresh = useCallback(async () => {
    if (receipt.current) await onDone(receipt.current);
    setChanges(await api.changes(membershipId)); refreshed.current = true;
  }, [membershipId, onDone]);
  return <section className="work-preparation" aria-label={t('Prepare working changes')}>
    <strong>{t('Save work before continuing')}</strong>
    <p>{t('1. Review and stash your changes. 2. Return to a fresh review of your original action.')}</p>
    <p className="hint">{t('Nothing runs automatically. Saved changes stay in the stash until you choose Apply or Pop.')}</p>
    {!changes && !error && <p className="loading-status" role="status">{t('Preparing saved work…')}</p>}
    {error && <p className="inline-error" role="alert">{t(error)}</p>}
    {onCarry && <button disabled={disabled} onClick={onCarry}>{t('Carry changes into the new branch')}</button>}
    {open && changes ? <><Suspense fallback={<p className="loading-status" role="status">{t('Preparing saved work…')}</p>}><PreparationStash initialKind="push" membershipId={membershipId} changes={changes} disabled={disabled} onBusyChange={onBusyChange} onOperation={value => { receipt.current = value; }} onFailure={onFailure} onRefresh={refresh} onDiffChange={setDiff} onClose={() => { setOpen(false); if (receipt.current?.status === 'succeeded' && refreshed.current) setReady(true); }}/></Suspense>
      {diff && <div className="preparation-diff">{diff.loading ? <p className="loading-status" role="status">{t('Reading changes…')}</p> : <><DiffView diff={diff.source.diff} label={t('Stash review')}/>{diff.source.truncated && <p className="hint">{t('This file preview is partial. The stash operation includes the complete reviewed scope.')}</p>}</>}</div>}
    </> : !ready && <button disabled={disabled || !changes} onClick={() => { receipt.current = null; refreshed.current = false; setOpen(true); }}>{t('Review stash first…')}</button>}
  </section>;
}
