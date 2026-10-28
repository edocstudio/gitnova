import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import { t } from './i18n';
import type { GitRef, OperationReceipt, WorktreeCreatePreview, WorktreeCreateRequest } from './contracts';

export function WorktreeCreateReview({ membershipId, oid, locals, blocked, onBack, onBusyChange, onDone, onClose, onFailure }: {
  membershipId: string; oid: string; locals: GitRef[]; blocked: boolean; onBack: () => void;
  onBusyChange: (value: boolean) => void; onDone: (receipt: OperationReceipt) => Promise<void>;
  onClose: () => void; onFailure: (id: string, message: string) => void;
}) {
  const [mode, setMode] = useState<WorktreeCreateRequest['mode']>('newBranch');
  const [branch, setBranch] = useState(''); const [destination, setDestination] = useState('');
  const [preview, setPreview] = useState<WorktreeCreatePreview | null>(null);
  const [loading, setLoading] = useState(false); const [writing, setWriting] = useState(false); const [error, setError] = useState('');
  const sequence = useRef(0); const inFlight = useRef(false); const confirm = useRef<HTMLButtonElement>(null);
  useEffect(() => () => { sequence.current++; }, []);
  useEffect(() => { if (preview) confirm.current?.focus(); }, [preview]);
  const review = async () => {
    if (blocked || loading || inFlight.current) return;
    const seq = ++sequence.current; setLoading(true); setError('');
    try { const result = await api.previewCreateWorktree(membershipId, { oid, mode, branch: mode === 'detached' ? '' : branch, destination }); if (seq === sequence.current) setPreview(result); }
    catch (e) { if (seq === sequence.current) setError(errorMessage(e)); }
    finally { if (seq === sequence.current) setLoading(false); }
  };
  const apply = async () => {
    if (!preview || preview.blockedReason || blocked || inFlight.current) return;
    inFlight.current = true; setWriting(true); onBusyChange(true); setError('');
    try {
      const receipt = await api.createWorktree(membershipId, preview.request, preview.token);
      setPreview(null);
      try { await onDone(receipt); }
      catch (e) { setError(`${t('Git returned a result, but refreshing failed. Inspect the recorded result before retrying.')} ${errorMessage(e)}`); return; }
      if (receipt.status === 'succeeded') onClose(); else setError(`${t(receipt.summary)} ${receipt.recovery ? t(receipt.recovery) : ''}`);
    } catch (e) { setPreview(null); setError(errorMessage(e)); onFailure(membershipId, errorMessage(e)); }
    finally { inFlight.current = false; setWriting(false); onBusyChange(false); }
  };
  return <div className="worktree-create-review">
    <strong>{t('Create linked worktree')}</strong>
    {preview ? <>
      <dl className="worktree-create-scope"><div><dt>{t('Source worktree')}</dt><dd>{preview.source.display}</dd></div><div><dt>{t('New folder')}</dt><dd>{preview.destination.display}</dd></div><div><dt>{t('Commit')}</dt><dd><code>{preview.request.oid.slice(0, 12)}</code></dd></div><div><dt>{t('Branch')}</dt><dd>{preview.request.mode === 'detached' ? t('Detached HEAD') : preview.request.branch}</dd></div></dl>
      <p>{preview.files === 1 ? t('{0} path checked out into a separate folder.', [preview.files]) : t('{0} paths checked out into a separate folder.', [preview.files])}</p>
      <p className="hint">{t('Current files, staged work and branch stay in this worktree. Open the new worktree from Related repositories after creation.')}</p>
      {preview.submodules > 0 && <p className="notice">{t('{0} submodule pointers; initialize them separately in the new worktree.', [preview.submodules])}</p>}
      {preview.request.mode === 'detached' && <p className="notice">{t('Detached HEAD: create a branch in the new worktree before making commits you want to keep.')}</p>}
      {preview.blockedReason ? <p className="notice" role="status">{t(preview.blockedReason)}</p> : <p className="hint">{t('Configured Git hooks and checkout filters can run. No force, reset or automatic stash.')}</p>}
      <div className="graph-action-footer"><button disabled={writing} onClick={() => setPreview(null)}>{t('Back')}</button><button ref={confirm} className="primary" disabled={blocked || writing || !!preview.blockedReason} onClick={() => void apply()}>{writing ? t('Creating worktree…') : t('Create worktree')}</button></div>
    </> : <form onSubmit={event => { event.preventDefault(); void review(); }}>
      <label htmlFor="worktree-create-mode">{t('Checkout in the new folder')}</label><select id="worktree-create-mode" disabled={loading || writing} value={mode} onChange={e => { setMode(e.target.value as WorktreeCreateRequest['mode']); setBranch(e.target.value === 'existingBranch' ? locals[0]?.name.slice(11) ?? '' : ''); setError(''); }}><option value="newBranch">{t('New branch')}</option><option value="existingBranch" disabled={!locals.length}>{t('Existing local branch at this commit')}</option><option value="detached">{t('Detached HEAD')}</option></select>
      {mode !== 'detached' && <><label htmlFor="worktree-create-branch">{t('Branch name')}</label>{mode === 'existingBranch' ? <select id="worktree-create-branch" disabled={loading || writing} value={branch} onChange={e => setBranch(e.target.value)}>{locals.map(ref => <option key={ref.name} value={ref.name.slice(11)}>{ref.name.slice(11)}</option>)}</select> : <input id="worktree-create-branch" value={branch} onChange={e => setBranch(e.target.value)} disabled={loading || writing} maxLength={256} autoComplete="off" spellCheck={false} required />}</>}
      <label htmlFor="worktree-create-destination">{t('New folder (absolute path)')}</label><input id="worktree-create-destination" value={destination} onChange={e => setDestination(e.target.value)} disabled={loading || writing} maxLength={4096} autoComplete="off" spellCheck={false} placeholder={t('/absolute/path/to/new-worktree')} required />
      <p className="hint">{t('Choose a new folder outside existing repositories. Its parent directory must already exist.')}</p>
      {loading && <p role="status">{t('Inspecting destination and branch availability…')}</p>}
      <div className="graph-action-footer"><button type="button" disabled={loading || writing} onClick={onBack}>{t('Back')}</button><button className="primary" disabled={blocked || loading || writing || !destination || (mode !== 'detached' && !branch)}>{t('Review worktree')}</button></div>
    </form>}
    {error && <div className="inline-error" role="alert">{t(error)}</div>}
  </div>;
}
