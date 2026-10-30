import { useEffect, useRef, useState } from 'react';
import {WorktreeMaintenance} from './RelatedMaintenance';
import { api, errorMessage } from './api';
import type { LinkedWorktree, NativePath, OperationReceipt, WorktreeRemovePreview } from './contracts';
import { Icon } from './Icons';
import { t } from './i18n';

// Reuse the related-repository row and inline review; opening never removes a checkout.
export function WorktreeEntry({ tree, membershipId, disabled, canRemove, onOpen, onBusyChange, onDone }: {
  tree: LinkedWorktree; membershipId: string; disabled: boolean; canRemove: boolean; onOpen: (path: NativePath) => void;
  onBusyChange: (value: boolean) => void; onDone: (receipt: OperationReceipt) => Promise<void>;
}) {
  const [preview, setPreview] = useState<WorktreeRemovePreview | null>(null);
  const [loading, setLoading] = useState(false); const [writing, setWriting] = useState(false); const [error, setError] = useState('');
  const sequence = useRef(0); const inFlight = useRef(false);
  useEffect(() => () => { sequence.current++; }, []);
  const review = async () => {
    if (inFlight.current || disabled) return;
    const request = ++sequence.current; setLoading(true); setError(''); setPreview(null);
    try { const value = await api.previewRemoveWorktree(membershipId, { path: tree.path, id: '' }); if (request === sequence.current) setPreview(value); }
    catch (e) { if (request === sequence.current) setError(errorMessage(e)); }
    finally { if (request === sequence.current) setLoading(false); }
  };
  const apply = async () => {
    if (!preview || preview.blockedReason || inFlight.current || disabled) return;
    inFlight.current = true; setWriting(true); onBusyChange(true); setError('');
    let receipt: OperationReceipt;
    try { receipt = await api.removeWorktree(membershipId, preview.request, preview.token); }
    catch (e) { setPreview(null); setError(errorMessage(e)); inFlight.current = false; setWriting(false); onBusyChange(false); return; }
    // Publishing the result precedes follow-up reads. A failed refresh cannot turn a completed write into a retry.
    setPreview(null);
    try { await onDone(receipt); }
    catch { setError('The operation result was recorded, but related repositories could not refresh. Refresh to inspect the current state.'); }
    finally { inFlight.current = false; setWriting(false); onBusyChange(false); }
  };
  return <div className="worktree-entry submodule-entry">
    <button className="submodule-open" disabled={disabled || writing || tree.prunable || tree.missing} title={tree.path.display} onClick={() => onOpen(tree.path)}>
      <span><Icon name="branch" size={14} />{tree.branch ?? (tree.bare ? t('Bare repository') : t('Detached HEAD'))}{tree.locked ? t(' · locked') : ''}{tree.missing ? t(' · missing') : tree.prunable ? t(' · prunable') : ''}</span><small>{tree.path.display}</small>
    </button>
    {canRemove&&<WorktreeMaintenance tree={tree} membershipId={membershipId} disabled={disabled||writing||loading||!!preview} onBusyChange={onBusyChange} onDone={onDone}/>}
    {canRemove && <button className="submodule-action" disabled={disabled || loading || writing} onClick={() => void review()}>{t('Remove worktree…')}</button>}
    {loading && <p className="hint" role="status">{t('Checking worktree files and workspace impact…')}</p>}
    {error && <div className="inline-error" role="alert">{t(error)}</div>}
    {preview && <div className="submodule-preview" role="region" aria-label={t('Review worktree removal')}>
      <strong>{t('Remove this checkout?')}</strong>
      <dl><div><dt>{t('Folder')}</dt><dd>{preview.request.path.display}</dd></div><div><dt>{t('Branch')}</dt><dd>{preview.branch ?? t('Detached HEAD')}</dd></div><div><dt>{t('Commit')}</dt><dd><code>{preview.head?.slice(0, 12) ?? t('None')}</code></dd></div></dl>
      <p>{t('Removes this clean checkout folder. Keeps its branch and saves its commit for recovery. Your current checkout stays open.')}</p>
      {preview.memberships.length > 0 && <><p>{t('Also removes these workspace entries:')}</p><ul>{preview.memberships.map(member => <li key={member.membershipId}>{member.workspaceName} / {member.name}</li>)}</ul></>}
      <details><summary>{t('Recovery details')}</summary><p>{t('Recovery ref')}</p><code className="worktree-recovery">{preview.recoveryRef}</code></details>
      {preview.blockedReason && <div className="notice" role="status">{t(preview.blockedReason)}</div>}
      <div className="submodule-confirm"><button disabled={writing} onClick={() => setPreview(null)}>{t('Cancel')}</button><button className="danger-action" disabled={disabled || writing || Boolean(preview.blockedReason)} onClick={() => void apply()}>{writing ? t('Removing…') : t('Remove checkout')}</button></div>
    </div>}
  </div>;
}
