import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import type { NativePath, OperationReceipt, RepoStatus, Submodule } from './contracts';
import { Icon } from './Icons';
import { SubmoduleLifecycle } from './SubmoduleLifecycle';
import { SubmoduleEntry } from './SubmoduleEntry';
import { t } from './i18n';

export function BranchSubmodules({ membershipId, status, revision, disabled, onOpen, onBusyChange, onDone, onFailure }: {
  membershipId: string; status: RepoStatus; revision?: string; disabled: boolean;
  onOpen: (path: NativePath) => void; onBusyChange: (value: boolean) => void;
  onDone: (receipt: OperationReceipt) => Promise<void>; onFailure: (id: string, message: string) => void;
}) {
  const [expanded, setExpanded] = useState(true);
  const [modules, setModules] = useState<Submodule[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [refresh, setRefresh] = useState(0);
  const inFlight = useRef(false);
  const rowsRef = useRef<Submodule[] | null>(null);
  const lastReadAt = useRef(0);
  const manualRevision = useRef(0);
  useEffect(() => {
    if (disabled || !expanded || selected) { setLoading(false); return; }
    let cancelled = false;
    const manual = refresh !== manualRevision.current;
    manualRevision.current = refresh;
    let retry: ReturnType<typeof setTimeout> | undefined;
    const read = async (foreground = false, force = false) => {
      if (cancelled || disabled || !expanded || selected || document.visibilityState === 'hidden') return;
      if (!force && Date.now() - lastReadAt.current < 30000) return;
      if (inFlight.current) { if (!retry) retry = setTimeout(() => { retry = undefined; void read(foreground, force); }, 250); return; }
      inFlight.current = true; lastReadAt.current = Date.now();
      if (foreground || rowsRef.current === null) setLoading(true);
      try {
        const {status: after, submodules: rows} = await api.submoduleSnapshot(membershipId);
        if (cancelled) return;
        if (after.branch !== status.branch || after.head !== status.head) {
          rowsRef.current = null; setModules(null); setError('Branch context changed. Refresh the graph to inspect its submodules.'); return;
        }
        rowsRef.current = rows; setModules(previous => JSON.stringify(previous) === JSON.stringify(rows) ? previous : rows); setError('');
      } catch (e) { if (!cancelled) setError(errorMessage(e)); }
      finally { inFlight.current = false; if (!cancelled) setLoading(false); }
    };
    // StrictMode cleanup occurs before this read starts.
    queueMicrotask(() => void read(manual, true));
    const backgroundRead = () => { void read(); };
    const timer = setInterval(backgroundRead, 60000);
    window.addEventListener('focus', backgroundRead);
    document.addEventListener('visibilitychange', backgroundRead);
    return () => { cancelled = true; clearInterval(timer); clearTimeout(retry); window.removeEventListener('focus', backgroundRead); document.removeEventListener('visibilitychange', backgroundRead); };
  }, [membershipId, status.branch, status.head, revision, disabled, expanded, selected, refresh]);

  const done = async (receipt: OperationReceipt) => {
    setSelected(null);
    await onDone(receipt); setRefresh(value => value + 1);
  };
  return <section className="graph-submodules" aria-label={t('Branch submodules')}>
    <div className="graph-submodules-heading"><button aria-expanded={expanded} onClick={() => setExpanded(value => !value)}><Icon name="layers" size={14}/><strong>{t('Submodules')}</strong><span>{modules?.length ?? '…'}</span><span className="graph-submodules-context">{status.branch ?? t('Detached HEAD')}</span><Icon name="chevron" size={12}/></button><button disabled={disabled||loading||!expanded||Boolean(selected)} onClick={()=>setSelected('add-registration')}>{t('Add submodule…')}</button><button disabled={disabled || loading || !expanded || Boolean(selected)} aria-label={t('Refresh submodules')} onClick={() => setRefresh(value => value + 1)}><Icon name="refresh" size={13} className={loading && modules !== null ? 'loading-spin' : undefined}/></button></div>
    {expanded && <div className={`graph-submodules-list ${selected ? 'managing' : ''}`}>
      {loading && modules === null && <p className="hint operation-wait" role="status"><span className="loading-spin"><Icon name="refresh" size={12}/></span>{t('Reading branch submodules…')}</p>}
      {error && <p className="inline-error" role="alert">{modules !== null && <>{t('Could not refresh submodules. Showing the last known state.')} </>}{t(error)}</p>}
      {selected==='add-registration'&&<SubmoduleLifecycle membershipId={membershipId} disabled={disabled} onBusyChange={onBusyChange} onDone={done} onClose={()=>setSelected(null)}/>}
      {modules?.length === 0 && <p className="hint">{t('No submodules recorded in this checkout.')}</p>}
      {modules?.map(sub => <div key={sub.relationId} className="branch-submodule" onContextMenu={event => { event.preventDefault(); if (!disabled) setSelected(sub.relationId); }}>
        {selected === sub.relationId ? <><div className="branch-submodule-toolbar"><strong>{sub.path.display}</strong><button disabled={disabled} onClick={() => setSelected(null)}>{t('Back')}</button></div><SubmoduleEntry detailed membershipId={membershipId} submodule={sub} disabled={disabled} onOpen={onOpen} onBusyChange={onBusyChange} onDone={done} onFailure={onFailure}/></> : <div className="branch-submodule-row"><button className="branch-submodule-open" disabled={disabled || !sub.initialized || Boolean(sub.inspectionError)} title={sub.path.display} onClick={() => onOpen(sub.path)}><Icon name="layers" size={14}/><strong>{sub.path.display}</strong></button><span className="branch-submodule-state">{!sub.initialized ? t('Not initialized') : sub.inspectionError || sub.dirty === null ? t('Status unavailable') : sub.dirty ? t('Uncommitted changes inside') : t('Files inside are clean')}{sub.initialized && sub.checkedOutOid !== sub.indexOid && <span>{t('Commit changed')}</span>}{sub.headOid !== sub.indexOid && <span>{t('Pointer staged')}</span>}{sub.detached && <span>{t('Detached HEAD')}</span>}</span><button disabled={disabled} onClick={() => setSelected(sub.relationId)} aria-label={t('Manage submodule {0}', [sub.path.display])}>{t('Manage…')}</button></div>}
      </div>)}
    </div>}
  </section>;
}
