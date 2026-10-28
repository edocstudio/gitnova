import { t } from './i18n';
import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import type { BranchTracking, BranchSwitchPreview, LocalBranch, OperationReceipt, RemoteBranches, RemoteSwitchPreview, RepoStatus } from './contracts';
import { Icon } from './Icons';
import { BranchTrackingBadge } from './BranchTrackingBadge';
export function BranchSwitcher({ tracking = [], membershipId, status, blocked = false, contextActive = false, onBusyChange, onSwitched }: { tracking?: BranchTracking[]; membershipId: string; blocked?: boolean; contextActive?: boolean; status: RepoStatus | undefined; onBusyChange: (busy: boolean) => void; onSwitched: (receipt: OperationReceipt) => Promise<void> }) {
  const [open, setOpen] = useState(false); const [query, setQuery] = useState(''); const [scope, setScope] = useState<'local' | 'remote'>('local');
  const [branches, setBranches] = useState<LocalBranch[]>([]); const [remotes, setRemotes] = useState<RemoteBranches>({ remotes: [], branches: [] });
  const [preview, setPreview] = useState<BranchSwitchPreview | null>(null); const [remotePreview, setRemotePreview] = useState<RemoteSwitchPreview | null>(null);
  const [loading, setLoading] = useState(false); const [writing, setWriting] = useState(false); const [phase, setPhase] = useState('');
  const [error, setError] = useState(''); const [receipt, setReceipt] = useState<OperationReceipt | null>(null);
  const root = useRef<HTMLDivElement>(null); const input = useRef<HTMLInputElement>(null); const request = useRef(0); const inFlight = useRef(false);
  useEffect(() => { if ((contextActive || blocked) && !inFlight.current) setOpen(false); }, [contextActive, blocked]);
  useEffect(() => {
    if (!open) return; const version = ++request.current; setLoading(true); setError(''); setReceipt(null); setPreview(null); setRemotePreview(null); setQuery('');
    Promise.all([api.branches(membershipId), api.remoteBranches(membershipId)]).then(([local, remote]) => { if (version === request.current) { setBranches(local); setRemotes(remote); } }).catch(e => { if (version === request.current) setError(errorMessage(e)); }).finally(() => { if (version === request.current) { setLoading(false); requestAnimationFrame(() => input.current?.focus()); } });
    return () => { request.current++; };
  }, [membershipId, open]);
  useEffect(() => {
    const pointer = (event: PointerEvent) => { if ((event.target as Element)?.closest?.('dialog[open]')) return; if (!inFlight.current && !root.current?.contains(event.target as Node)) setOpen(false); };
    const key = (event: KeyboardEvent) => { if ((event.target as Element)?.closest?.('dialog[open]')) return; if (event.key === 'Escape' && open && !inFlight.current) { event.preventDefault(); event.stopImmediatePropagation(); setOpen(false); root.current?.querySelector<HTMLButtonElement>('.branch-current')?.focus(); } };
    document.addEventListener('pointerdown', pointer); document.addEventListener('keydown', key, true);
    return () => { document.removeEventListener('pointerdown', pointer); document.removeEventListener('keydown', key, true); };
  }, [open]);
  const choose = async (branch: string) => {
    if (blocked || inFlight.current) return;
    const version = ++request.current; setLoading(true); setPreview(null); setRemotePreview(null); setError(''); setReceipt(null);
    try { const value = await api.previewBranchSwitch(membershipId, branch); if (version === request.current) setPreview(value); }
    catch (e) { if (version === request.current) setError(errorMessage(e)); }
    finally { if (version === request.current) setLoading(false); }
  };
  const fetchLatest = async (remote: string, branch: string | null) => {
    if (inFlight.current || blocked) return;
    inFlight.current = true; setWriting(true); onBusyChange(true); setPhase(t("Fetching {0}{1}…", [remote,branch ? `/${branch}` : ''])); setPreview(null); setRemotePreview(null); setReceipt(null); setError('');
    try {
      const value = await api.fetchRemote(membershipId, remote, branch); setReceipt(value); await onSwitched(value);
      const [local, remoteBranches] = await Promise.all([api.branches(membershipId), api.remoteBranches(membershipId)]); setBranches(local); setRemotes(remoteBranches);
      if (value.status === 'succeeded' && branch) {
        setPhase(t("Preparing local branch…")); const review = await api.previewRemoteSwitch(membershipId, remote, branch); setRemotePreview(review); setPreview(review.switch);
      }
    } catch (e) { setError(errorMessage(e)); }
    finally { inFlight.current = false; setWriting(false); setPhase(''); onBusyChange(false); }
  };
  const apply = async () => {
    if (!preview || preview.blockedReason || inFlight.current || blocked) return;
    inFlight.current = true; setWriting(true); onBusyChange(true); setPhase(remotePreview ? t("Opening local branch and pulling…") : t("Switching branch…")); setError('');
    try {
      const value = remotePreview ? await api.switchRemoteBranch(membershipId, remotePreview.remote, remotePreview.remoteBranch, preview.token) : await api.switchBranch(membershipId, preview.branch, preview.token);
      setReceipt(value); setPreview(null); setRemotePreview(null); await onSwitched(value); if (value.status === 'succeeded') setOpen(false);
    } catch (e) { setError(errorMessage(e)); setPreview(null); setRemotePreview(null); }
    finally { inFlight.current = false; setWriting(false); setPhase(''); onBusyChange(false); }
  };
  const changeScope = (next: 'local' | 'remote') => { request.current++; setScope(next); setPreview(null); setRemotePreview(null); setReceipt(null); setError(''); setLoading(false); input.current?.focus(); };
  const matches = (name: string) => name.toLowerCase().includes(query.toLowerCase());
  const action = t(remotePreview ? remotePreview.create ? 'Create & open {0}' : 'Open & pull {0}' : 'Switch to {0}', [preview?.branch]);
  return <div className="branch-switcher" ref={root}>
    <button className="branch-current" aria-label={t("Switch branch")} aria-expanded={open} aria-controls="branch-picker" disabled={!status || status.bare || writing || blocked} title={receipt?.summary ? t(receipt.summary) : t("Open a local or remote branch")} onClick={() => setOpen(value => !value)}><Icon name="branch" size={15} /><span>{status?.branch ?? (status?.head ? t('Detached {0}', [status.head.slice(0, 8)]) : t('Reading…'))}</span>{status?.branch && <BranchTrackingBadge value={tracking.find(item => item.branch === status.branch)} oid={status.head}/>}<Icon name="chevron" size={13} /></button>
    <span className="visually-hidden" role="status">{receipt?.summary ? t(receipt.summary) : ''}</span>
    {open && <div id="branch-picker" className="branch-menu" role="region" aria-label={t("Open branch")}><div className="branch-menu-heading"><Icon name="branch" /><strong>{t("Open branch")}</strong></div>
      <div className="branch-scopes" role="group" aria-label={t("Branch scope")}><button aria-pressed={scope === 'local'} disabled={loading || writing || blocked} onClick={() => changeScope('local')}>{t("Local")} <span>{branches.length}</span></button><button aria-pressed={scope === 'remote'} disabled={loading || writing || blocked} onClick={() => changeScope('remote')}>{t("Remotes")} <span>{remotes.branches.length}</span></button></div>
      <input ref={input} aria-label={t("Find branch")} placeholder={scope === 'local' ? t("Find a local branch…") : t("Find a remote branch…")} value={query} onChange={event => setQuery(event.target.value)} disabled={loading || writing || blocked} />
      <div className="branch-options">
        {scope === 'local' ? <>{branches.filter(branch => matches(branch.name)).map(branch => <button key={branch.name} aria-label={t("Select branch {0}", [branch.name])} aria-pressed={!remotePreview && preview?.branch === branch.name} disabled={branch.current || loading || writing || blocked} onClick={() => void choose(branch.name)}><Icon name="branch" size={15} /><span><strong>{branch.name}</strong><small>{branch.current ? t("Current branch") : branch.checkedOutAt ? t("In use: {0}", [branch.checkedOutAt.display]) : branch.oid.slice(0, 8)}</small></span><BranchTrackingBadge value={tracking.find(item => item.branch === branch.name)} oid={branch.oid}/>{branch.current && <Icon name="check" size={15} />}</button>)}{!loading && !branches.some(branch => matches(branch.name)) && <p className="hint">{branches.length ? t("No matching local branches.") : t("No committed local branches yet.")}</p>}</> : <>
          {remotes.remotes.map(remote => <div className="remote-group" key={remote}><div className="remote-group-heading"><span><Icon name="remote" size={13} />{remote}</span><button aria-label={t("Fetch branches from {0}", [remote])} disabled={loading || writing || blocked} onClick={() => void fetchLatest(remote, null)}><Icon name="refresh" size={12} />{t("Fetch")}</button></div>{remotes.branches.filter(branch => branch.remote === remote && matches(`${remote}/${branch.name}`)).map(branch => <button key={branch.name} aria-label={t("Open remote {0}/{1}", [remote, branch.name])} title={t("Fetch the latest revision, then review its local branch")} disabled={loading || writing || blocked} aria-pressed={remotePreview?.remote === remote && remotePreview.remoteBranch === branch.name} onClick={() => void fetchLatest(remote, branch.name)}><Icon name="branch" size={15} /><span><strong>{branch.name}</strong><small>{branch.oid.slice(0, 8)} {t("· fetch & open locally")}</small></span><Icon name="arrow" size={13} /></button>)}{!remotes.branches.some(branch => branch.remote === remote && matches(`${remote}/${branch.name}`)) && <p className="hint">{query ? t("No matching remote branches.") : t("Fetch to discover branches.")}</p>}</div>)}{!remotes.remotes.length && <p className="hint">{t("No remotes configured. Add a remote with Git to connect this repository.")}</p>}
        </>}
      </div>
      {scope === 'remote' && !preview && !writing && <p className="hint">{t("Choose a branch to fetch its latest revision and open it locally. Git uses your configured credentials or SSH agent.")}</p>}
      {(loading || writing) && <p className="hint" role="status">{phase || t("Reading branches and switch preview…")}</p>}
      {error && <div className="inline-error" role="alert">{t(error)}</div>}
      {receipt && receipt.status !== 'succeeded' && <div className="inline-error" role="alert">{t(receipt.summary)}<p>{receipt.recovery}</p></div>}
      {receipt?.kind === 'fetch_remote' && receipt.status === 'succeeded' && !preview && !writing && <p className="hint" role="status">{t("Remote branches refreshed. Choose a branch to open locally.")}</p>}
      {preview && <div className="branch-preview"><div className="branch-preview-title"><span>{preview.sourceBranch ?? t("Detached {0}", [preview.sourceOid?.slice(0, 8) ?? ''])}</span><Icon name="arrow" size={14} /><strong>{preview.branch}</strong></div>
        {remotePreview && <p className="remote-integration-summary">{t("Fetched")} {remotePreview.remote}/{remotePreview.remoteBranch}<br />{remotePreview.create ? t("Create a local tracking branch at the fetched revision.") : t("{0} local {1} ahead · {2} {3} to pull. Fast-forward only; local commits stay intact.", [remotePreview.ahead, remotePreview.ahead === 1 ? t("commit") : t("commits"), remotePreview.behind, remotePreview.behind === 1 ? t("commit") : t("commits")])}</p>}
        {!preview.blockedReason && <><p>{preview.files.length} {preview.files.length === 1 ? t("path changes") : t("paths change")} {t("· target")} <code>{preview.targetOid.slice(0, 8)}</code></p><div className="branch-preview-files">{preview.files.map(file => <div key={file.path.hex}><span className={`file-status status-${file.status}`}>{file.status}</span><code>{file.path.display}</code></div>)}</div></>}
        {preview.submodules.length > 0 && <div className="branch-submodules"><strong>{t("Submodule checkouts stay in place")}</strong>{preview.submodules.map(sub => <div key={sub.path.hex}><code>{sub.path.display}</code><span>{t("Recorded")} {sub.sourceOid.slice(0, 8)} → {sub.targetOid.slice(0, 8)}</span><span>{sub.checkedOutOid ? t("Checkout {0}{1}", [sub.checkedOutOid.slice(0, 8), sub.dirty ? t(" · uncommitted work") : t(" · preserved")]) : t("Not initialized · stays uninitialized")}</span></div>)}<p>{t("The parent may show a submodule pointer difference afterward. Restore its recorded version separately when needed.")}</p></div>}
        {preview.blockedReason ? <div className="notice" role="status">{t(preview.blockedReason)}</div> : <p className="hint">{t("Configured Git hooks and filters can run. Submodules are not initialized or updated.")}</p>}
        <button className="primary" disabled={Boolean(preview.blockedReason) || writing || blocked} onClick={() => void apply()}>{writing ? phase : action}<Icon name="arrow" size={14} /></button>
      </div>}
    </div>}
  </div>;
}
