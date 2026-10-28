import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import type { Catalog, CloneOutcome, ClonePreview, WorkProgress } from './contracts';
import { t } from './i18n';
import { Icon } from './Icons';
export type CloneDraft = { source: string; destination: string };
export function CloneControls({ workspaceId, workspaceName, draft, onDraft, disabled, onBusyChange, onCloned }: {
  workspaceId: string; workspaceName: string; draft: CloneDraft; onDraft: (draft: CloneDraft) => void; disabled: boolean;
  onBusyChange: (busy: boolean) => void; onCloned: (catalog: Catalog) => void;
}) {
  const [preview, setPreview] = useState<ClonePreview | null>(null); const [outcome, setOutcome] = useState<CloneOutcome | null>(null);
  const [reading, setReading] = useState(false); const [writing, setWriting] = useState(false); const [error, setError] = useState('');
  const [recent, setRecent] = useState<CloneOutcome[]>([]); const [progress, setProgress] = useState<WorkProgress | null>(null);
  const inFlight = useRef(false); const sequence = useRef(0);
  useEffect(() => () => { sequence.current++; }, []);
  const loadRecent = async () => { try { setRecent(await api.recentClones(workspaceId)); } catch (e) { setError(errorMessage(e)); } };
  const review = async () => {
    if (disabled || inFlight.current || reading) return;
    const version = ++sequence.current; setReading(true); setError(''); setOutcome(null); setPreview(null);
    try { const value = await api.previewClone({ workspaceId, ...draft }); if (version === sequence.current) { if (value.version !== 1) throw new Error('Restart GitNova before cloning. Nothing has been applied.'); setPreview(value); } }
    catch (e) { if (version === sequence.current) setError(errorMessage(e)); }
    finally { if (version === sequence.current) setReading(false); }
  };
  const clone = async () => {
    if (!preview || disabled || inFlight.current) return;
    inFlight.current = true; setWriting(true); onBusyChange(true); setError(''); setProgress(null);
    try {
      const value = await api.cloneRepository(preview.request, preview.token, setProgress);
      setOutcome(value); setPreview(null);
      if (value.catalog && !value.recovery) { onDraft({source:'',destination:''}); onCloned(value.catalog); }
      else await loadRecent();
    } catch (e) { setPreview(null); setError(errorMessage(e)); await loadRecent(); }
    finally { inFlight.current = false; setWriting(false); onBusyChange(false); setProgress(null); }
  };
  const change = (next: CloneDraft) => { sequence.current++; setReading(false); setPreview(null); setOutcome(null); setError(''); onDraft(next); };
  const locked = disabled || reading || writing;
  return <details className="clone-controls" onToggle={event => { if (event.currentTarget.open && !inFlight.current) void loadRecent(); }}>
    <summary onClick={event => { if (writing) event.preventDefault(); }}>{t('Clone repository…')}</summary>
    <p className="hint">{t('Clone into {0}, then open its Git graph.', [workspaceName])}</p>
    <form onSubmit={event => { event.preventDefault(); void review(); }}>
      <label htmlFor="clone-source">{t('Repository URL or local source')}</label><input id="clone-source" value={draft.source} maxLength={8192} placeholder="git@host:team/project.git" disabled={locked} autoComplete="off" spellCheck={false} onChange={event => change({...draft,source:event.target.value})} required/>
      <label htmlFor="clone-destination">{t('New clone folder')}</label><input id="clone-destination" value={draft.destination} maxLength={8192} placeholder={t('/absolute/path/to/new-folder')} disabled={locked} autoComplete="off" spellCheck={false} onChange={event => change({...draft,destination:event.target.value})} required/>
      <p className="hint">{t('The parent folder must exist. Existing folders are never overwritten.')}</p>
      {!preview && <button className="primary" disabled={locked || !draft.source.trim() || !draft.destination.trim()}>{reading ? t('Inspecting clone destination…') : t('Review clone')}</button>}
    </form>
    {preview && <div className="clone-preview"><strong>{t('Clone into {0}', [workspaceName])}</strong><dl><dt>{t('Source')}</dt><dd>{preview.request.source}</dd><dt>{t('Destination')}</dt><dd>{preview.destination.display}</dd></dl><p className="hint">{t('Review does not contact the source. Clone uses configured credentials or SSH. Initial checkout may run configured Git filters. Submodules stay uninitialized.')}</p><div className="graph-action-footer"><button disabled={writing} onClick={() => setPreview(null)}>{t('Back')}</button><button className="primary" disabled={locked} onClick={() => void clone()}>{t('Clone & open graph')}</button></div></div>}
    {writing && <div className="discard-progress" role="status" aria-busy="true"><div><Icon name="refresh" className="loading-spin" size={14}/>{t(progress?.phase === 'verifying-clone' ? 'Verifying cloned repository…' : progress?.phase === 'cloning' ? 'Transferring repository and checking out files…' : 'Preparing clone…')}</div><progress aria-label={t('Clone progress')}/><small>{t('Transfer size is unknown; progress is shown by phase.')}</small></div>}
    {error && <p className="inline-error" role="alert">{t(error)}</p>}
    {outcome && (!outcome.catalog || outcome.recovery) && <div role="status" className="notice"><p>{t(outcome.summary)}</p><code>{outcome.destination.display}</code>{outcome.recovery && <p>{t(outcome.recovery)}</p>}{outcome.catalog && <button onClick={() => { onDraft({source:'',destination:''}); onCloned(outcome.catalog!); }}>{t('Open cloned graph')}</button>}</div>}
    {recent.length > 0 && <details className="clone-attempts"><summary>{t('Previous clone attempts')}</summary>{recent.map(item => <div key={item.id}><strong>{t(item.status === 'pending' ? 'Completion unknown · inspect destination' : item.status === 'cloned' ? 'Cloned · registration needs inspection' : item.status === 'succeeded' ? 'Clone completed' : 'Clone stopped')}</strong><code>{item.destination.display}</code>{item.status !== 'succeeded' && <><p className="hint">{t(item.summary)}</p><p className="hint">{item.recovery && t(item.recovery)}</p></>}</div>)}</details>}
  </details>;
}
