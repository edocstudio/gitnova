import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import { t } from './i18n';
import type { BranchTracking, OperationReceipt, RemoteSwitchPreview } from './contracts';
import { MergeStartReview } from './MergeStartReview';

export function SyncReview({ kind, membershipId, branch, tracking, blocked, onBusyChange, onDone, onClose, onFailure, onReviewWork }: {
  kind: 'fetch' | 'pull'; membershipId: string; branch: string | null; tracking?: BranchTracking; blocked: boolean;
  onBusyChange: (busy: boolean) => void; onDone: (receipt: OperationReceipt) => Promise<void>; onClose: () => void;
  onFailure: (id: string, message: string) => void; onReviewWork: () => void;
}) {
  const [remotes, setRemotes] = useState<string[]>([]); const [remote, setRemote] = useState('');
  const [reading, setReading] = useState(true); const [writing, setWriting] = useState(false);
  const [phase, setPhase] = useState(''); const [error, setError] = useState('');
  const [review, setReview] = useState<RemoteSwitchPreview | null>(null);
  const [merging, setMerging] = useState(false);
  const inFlight = useRef(false);
  const fetch = async (selected: string, upstream?: { remote: string; remoteBranch: string }) => {
    if (inFlight.current) return;
    inFlight.current = true; setWriting(true); onBusyChange(true); setError(''); setReview(null);
    setPhase(t('Fetching {0}{1}…', [selected, upstream ? `/${upstream.remoteBranch}` : '']));
    try {
      const receipt = await api.fetchRemote(membershipId, selected, upstream?.remoteBranch ?? null);
      await onDone(receipt);
      if (receipt.status !== 'succeeded') { setError(`${t(receipt.summary)} ${receipt.recovery ?? ''}`); return; }
      if (!upstream) { onClose(); return; }
      setPhase(t('Preparing pull preview…'));
      const next = await api.previewPullCurrent(membershipId);
      if (next.switch.branch !== branch || next.remote !== upstream.remote || next.remoteBranch !== upstream.remoteBranch) throw new Error('Branch or upstream changed. Close this review and try again.');
      setReview(next);
    } catch (e) { setError(errorMessage(e)); }
    finally { inFlight.current = false; setWriting(false); setPhase(''); onBusyChange(false); }
  };
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        if (kind === 'pull') {
          const initial = await api.pullUpstream(membershipId);
          if (cancelled) return;
          if (initial.branch !== branch || !initial.remote || !initial.remoteBranch) throw new Error('Branch or upstream changed. Close this review and try again.');
          setRemote(initial.remote); setReading(false); await fetch(initial.remote, {remote: initial.remote, remoteBranch: initial.remoteBranch});
        } else {
          const listing = await api.remoteBranches(membershipId);
          if (cancelled) return;
          setRemotes(listing.remotes);
          const preferred = tracking?.remote && listing.remotes.includes(tracking.remote) ? tracking.remote : listing.remotes.length === 1 ? listing.remotes[0] : '';
          setRemote(preferred); setReading(false);
          if (preferred) await fetch(preferred);
        }
      } catch (e) { if (!cancelled) { setError(errorMessage(e)); setReading(false); } }
    })();
    return () => { cancelled = true; };
    // One explicit graph intent. Ref refresh after Fetch must not replay it.
  }, []);
  const apply = async () => {
    if (!review || review.switch.blockedReason || blocked || inFlight.current) return;
    inFlight.current = true; setWriting(true); onBusyChange(true); setPhase(t('Pulling current branch…')); setError('');
    try {
      const receipt = await api.pullCurrent(membershipId, review.switch.token);
      setReview(null); await onDone(receipt);
      if (receipt.status === 'succeeded') onClose(); else setError(`${t(receipt.summary)} ${receipt.recovery ?? ''}`);
    } catch (e) { setReview(null); setError(errorMessage(e)); }
    finally { inFlight.current = false; setWriting(false); setPhase(''); onBusyChange(false); }
  };
  const locked = blocked || reading || writing;
  const diverged = Boolean(review && review.ahead > 0 && review.behind > 0);
  if (merging && review && review.localOid) return <div className="sync-review"><MergeStartReview membershipId={membershipId} oid={review.remoteOid} sourceRef={`refs/remotes/${review.remote}/${review.remoteBranch}`} expectedDestination={{branch: review.switch.branch, oid: review.localOid}} blocked={blocked} onBusyChange={onBusyChange} onDone={onDone} onClose={onClose} onFailure={onFailure} onReviewWork={onReviewWork} onBack={() => setMerging(false)}/></div>;
  return <div className="sync-review" aria-busy={reading || writing}>
    <strong>{kind === 'pull' ? t('Pull into {0}', [branch]) : t('Fetch remote branches')}</strong>
    <p className="hint">{kind === 'pull' ? t('Fetch first, then review incoming changes. If branches have diverged, review a merge. Local commits are preserved.') : t('Updates remote-tracking refs. Local branches and working files stay in place.')}</p>
    {(reading || writing) && <p className="operation-wait" role="status"><span className="loading-spin">↻</span>{phase || t('Reading configured remotes…')}</p>}
    {kind === 'fetch' && !reading && !writing && <><label htmlFor="sync-remote">{t('Remote')}</label><select id="sync-remote" disabled={locked} value={remote} onChange={event => setRemote(event.target.value)}><option value="">{t('Choose remote…')}</option>{remotes.map(name => <option key={name}>{name}</option>)}</select>{!remotes.length && <p className="notice">{t('No configured remotes. Add one with Git before using this action.')}</p>}</>}
    {review && <div className="pull-preview"><p><b>{review.remote}/{review.remoteBranch} → {review.switch.branch}</b></p><p>{diverged ? <>{t(review.ahead === 1 ? '{0} local commit' : '{0} local commits', [review.ahead])} · {t(review.behind === 1 ? '{0} incoming commit' : '{0} incoming commits', [review.behind])}</> : t('{0} commits ahead · {1} to pull. Fast-forward only.', [review.ahead, review.behind])}</p>{!review.switch.blockedReason && <><p>{t('Target')} <code>{review.switch.targetOid.slice(0, 12)}</code> · {review.switch.files.length} {t('paths change')}</p><div className="branch-preview-files">{review.switch.files.map(file => <div key={file.path.hex}><span>{file.status}</span><code>{file.path.display}</code></div>)}</div></>}
      {diverged && <p>{t('Fetched revision')} <code>{review.remoteOid?.slice(0, 12) ?? t('Status unavailable')}</code></p>}
      {review.switch.submodules.length > 0 && <p className="notice">{t('Submodule checkouts stay in place')}. {t('Restore its recorded version separately when needed.')}</p>}
      {review.switch.blockedReason ? <p className="notice" role="status">{diverged ? t('Branches have diverged. Review a merge to keep both histories; a fast-forward cannot combine them.') : t(review.switch.blockedReason)}</p> : <p className="hint">{t('Configured Git hooks and filters can run. Submodules are not initialized or updated.')}</p>}
    </div>}
    {error && <p className="inline-error" role="alert">{t(error)}</p>}
    <div className="graph-action-footer"><button disabled={writing} onClick={onClose}>{t('Close')}</button>{kind === 'fetch' ? <button className="primary" disabled={locked || !remote} onClick={() => void fetch(remote)}>{t('Fetch')}</button> : review && (diverged ? <button className="primary" disabled={locked || !review.localOid} onClick={() => { if (!review.remoteOid) { setError('Restart GitNova to load the fetched revision before reviewing a merge.'); return; } setMerging(true); }}>{t('Review merge…')}</button> : <button className="primary" disabled={locked || Boolean(review.switch.blockedReason)} onClick={() => void apply()}>{t('Pull into {0}', [review.switch.branch])}</button>)}</div>
  </div>;
}
