import {RemoteSetup} from './RemoteSetup';
import { t } from './i18n';
import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import type { BranchTracking, GitRef, OperationReceipt, PushPreview, RefOperation, RefOperationPreview, RemoteBranches } from './contracts';
type Mode = 'actions' | 'rename' | 'upstream' | 'push' | 'review';
export function RefManager({ onConfigureRemote, initialRemote, initialDestination, initialMode = 'actions', tracking, selectedRef, membershipId, blocked, onBack, onBusyChange, onDone, onClose, onFailure }: {
  onConfigureRemote?:()=>void; initialRemote?: string; initialDestination?: string; initialMode?: 'actions' | 'push' | 'delete'; tracking?: BranchTracking; selectedRef: GitRef; membershipId: string; blocked: boolean; onBack: () => void; onBusyChange: (busy: boolean) => void;
  onDone: (receipt: OperationReceipt) => Promise<void>; onClose: () => void; onFailure: (id: string, message: string) => void;
}) {
  const tag = selectedRef.name.startsWith('refs/tags/'); const name = selectedRef.name.replace(/^refs\/(heads|tags)\//, '');
  const [mode, setMode] = useState<Mode>(initialMode === 'delete' ? 'review' : initialMode); const [editMode, setEditMode] = useState<Mode>('actions');
  const [newName, setNewName] = useState(name); const [remote, setRemote] = useState(initialRemote ?? tracking?.pushRemote ?? tracking?.remote ?? ''); const [destination, setDestination] = useState(initialDestination ?? tracking?.pushBranch ?? (tracking?.pushRemote ? '' : tracking?.remoteBranch ?? name)); const [setUpstream, setSetUpstream] = useState(false);
  const [listing, setListing] = useState<RemoteBranches>({ remotes: [], branches: [] }); const [error, setError] = useState(''); const [loading, setLoading] = useState(false); const [writing, setWriting] = useState(false);
  const [settingUp,setSettingUp]=useState(false);
  const [bufferedHttps,setBufferedHttps]=useState(false);
  const [compatibilityOpen,setCompatibilityOpen]=useState(false);
  const [readingRemotes, setReadingRemotes] = useState(!tag);
  const [refReview, setRefReview] = useState<RefOperationPreview | null>(null); const [pushReview, setPushReview] = useState<PushPreview | null>(null);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => { requestAnimationFrame(() => root.current?.querySelector<HTMLElement>('input:not(:disabled), select:not(:disabled), button:not(:disabled)')?.focus()); }, [mode, loading]);
  const sequence = useRef(0); const inFlight = useRef(false); const locked = blocked || loading || writing || (mode === 'push' || mode === 'upstream') && readingRemotes;
  const loadRemotes = async () => { const value = await api.remoteBranches(membershipId); setListing(value); if (!remote && value.remotes.length === 1) setRemote(value.remotes[0]); return value; };
  useEffect(() => { let cancelled = false; if (!tag) void api.remoteBranches(membershipId).then(value => { if (!cancelled) { setListing(value); setRemote(current => current ? value.remotes.includes(current) ? current : '' : value.remotes.length === 1 ? value.remotes[0] : ''); } }).catch(e => { if (!cancelled) setError(errorMessage(e)); }).finally(() => { if (!cancelled) setReadingRemotes(false); }); return () => { cancelled = true; sequence.current++; }; }, [membershipId, tag]);
  const action = (kind: RefOperation['kind']): RefOperation => ({ kind, name, oid: selectedRef.oid, newName: kind === 'renameBranch' ? newName : '', remote: kind === 'setUpstream' ? remote : '', remoteBranch: kind === 'setUpstream' ? destination : '' });
  const reviewRef = async (kind: RefOperation['kind']) => {
    if (locked || inFlight.current) return; const request = ++sequence.current; setLoading(true); setError(''); setRefReview(null); setPushReview(null); setEditMode(mode === 'review' ? 'actions' : mode); setMode('review');
    try { const value = await api.previewRefOperation(membershipId, action(kind)); if (request === sequence.current) setRefReview(value); }
    catch (e) { if (request === sequence.current) { setError(errorMessage(e)); setMode(mode); } }
    finally { if (request === sequence.current) setLoading(false); }
  };
  useEffect(() => { if (initialMode === 'delete') void reviewRef('deleteBranch'); }, [initialMode, membershipId, selectedRef.name, selectedRef.oid]);
  const reviewPush = async () => {
    if (locked || inFlight.current) return; const request = ++sequence.current; setLoading(true); setError(''); setRefReview(null); setPushReview(null); setEditMode('push'); setMode('review');
    try { const value = await api.previewPush(membershipId, { branch: name, oid: selectedRef.oid, remote, remoteBranch: destination, setUpstream, bufferedHttps }); if (request === sequence.current) setPushReview(value); }
    catch (e) { if (request === sequence.current) { setError(errorMessage(e)); setMode('push'); } }
    finally { if (request === sequence.current) setLoading(false); }
  };
  const fetch = async () => {
    if (locked || inFlight.current || !remote) return; inFlight.current = true; setWriting(true); onBusyChange(true); setError('');
    try { const receipt = await api.fetchRemote(membershipId, remote); await onDone(receipt); if (receipt.status === 'succeeded') await loadRemotes(); }
    catch (e) { onFailure(membershipId, errorMessage(e)); }
    finally { inFlight.current = false; setWriting(false); onBusyChange(false); }
  };
  const apply = async () => {
    if (locked || inFlight.current || (!refReview && !pushReview) || refReview?.blockedReason || pushReview?.blockedReason) return;
    inFlight.current = true; setWriting(true); onBusyChange(true); setError('');
    try { const receipt = pushReview ? await api.pushBranch(membershipId, pushReview.request, pushReview.token) : await api.applyRefOperation(membershipId, refReview!.action, refReview!.token); await onDone(receipt); if (receipt.status === 'succeeded') onClose(); else { if(pushReview&&/HTTP 400|error: 400|returned error: 400/i.test(receipt.summary)){setCompatibilityOpen(true);setError(t("The HTTPS upload was rejected with HTTP 400. Inspect the remote before retrying. Buffered upload may help if chunked transfer was rejected; it does not bypass permissions or branch rules."));} setRefReview(null); setPushReview(null); setMode(editMode); } }
    catch (e) { onFailure(membershipId, errorMessage(e)); setRefReview(null); setPushReview(null); setMode(editMode); }
    finally { inFlight.current = false; setWriting(false); onBusyChange(false); }
  };
  const label = refReview?.action.kind === 'renameBranch' ? t("Rename branch") : refReview?.action.kind === 'deleteBranch' ? t("Delete local branch") : refReview?.action.kind === 'deleteTag' ? t("Delete local tag") : refReview?.action.kind === 'setUpstream' ? t("Set upstream") : t("Remove upstream");
  const chooseMode = (next: Mode) => { setMode(next); setError(''); setRefReview(null); setPushReview(null); };
  return <div ref={root} className="ref-manager" aria-label={t("Manage {0} {1}", [tag ? t("tag") : t("branch"), name])}>
    <div className="ref-manager-heading"><strong>{tag ? t("Tag") : t("Local branch")} · {name}</strong><code>{selectedRef.oid.slice(0, 12)}</code></div>
    {readingRemotes && (mode === 'push' || mode === 'upstream') && <p role="status">{t('Reading configured remotes…')}</p>}
    {settingUp?<RemoteSetup membershipId={membershipId} disabled={blocked||writing} onConnections={onConfigureRemote} onBusyChange={value=>{setWriting(value);onBusyChange(value);}} onDone={onDone} onAdded={value=>{setRemote(value);setSetUpstream(true);setSettingUp(false);void loadRemotes().catch(e=>setError(errorMessage(e)));}} onCancel={()=>setSettingUp(false)}/>:mode==='push'&&!readingRemotes&&!listing.remotes.length?<div className="notice remote-empty" role="status"><p>{t('No remote is configured for this repository. Add a destination before pushing?')}</p><div className="graph-action-footer"><button className="primary" disabled={locked} onClick={()=>setSettingUp(true)}>{t('Add remote…')}</button><button disabled={locked} onClick={onClose}>{t('Cancel')}</button></div></div>:
mode === 'actions' ? <div className="ref-manager-options">
      {!tag && <><button disabled={locked} onClick={() => chooseMode('push')}>{t("Push")} {name}…</button><button disabled={locked} onClick={() => chooseMode('rename')}>{t("Rename branch…")}</button><button disabled={locked} onClick={() => chooseMode('upstream')}>{t("Set or change upstream…")}</button><button disabled={locked} onClick={() => void reviewRef('unsetUpstream')}>{t("Remove upstream…")}</button></>}
      <button className="danger-action" disabled={locked} onClick={() => void reviewRef(tag ? 'deleteTag' : 'deleteBranch')}>{t("Delete local")} {tag ? t("tag") : t("branch")}…</button><button disabled={writing} onClick={onBack}>{t("Back to commit actions")}</button>
    </div> : mode === 'rename' ? <form onSubmit={e => { e.preventDefault(); void reviewRef('renameBranch'); }}><label htmlFor="rename-branch">{t("New branch name")}</label><input id="rename-branch" value={newName} onChange={e => setNewName(e.target.value)} maxLength={256} disabled={locked} /><p className="hint">{t("Keeps the commit, reflog and upstream. No checkout or file changes.")}</p><div className="graph-action-footer"><button type="button" disabled={writing} onClick={() => chooseMode('actions')}>{t("Back")}</button><button className="primary" disabled={locked || !newName || newName === name}>{t("Review rename")}</button></div></form> : mode === 'push' || mode === 'upstream' ? <form onSubmit={e => { e.preventDefault(); if (mode === 'push') void reviewPush(); else void reviewRef('setUpstream'); }}>
      <label htmlFor="manage-remote">{t("Remote")}</label><select id="manage-remote" value={remote} disabled={locked} onChange={e => { setRemote(e.target.value); if (mode === 'upstream') setDestination(''); }}><option value="">{t("Choose remote…")}</option>{listing.remotes.map(value => <option key={value}>{value}</option>)}</select>
      <label htmlFor="manage-destination">{mode === 'push' ? t("Destination branch") : t("Remote branch to track")}</label>
      {mode === 'push' ? <input id="manage-destination" value={destination} disabled={locked} onChange={e => setDestination(e.target.value)} maxLength={256} /> : <select id="manage-destination" value={destination} disabled={locked} onChange={e => setDestination(e.target.value)}><option value="">{t("Choose fetched branch…")}</option>{listing.branches.filter(b => b.remote === remote).map(b => <option key={b.name}>{b.name}</option>)}</select>}
      {mode === 'push' ? <><label className="ref-checkbox"><input type="checkbox" checked={setUpstream} disabled={locked} onChange={e => setSetUpstream(e.target.checked)} />{t("Set this destination as upstream after push")}</label><details open={compatibilityOpen} onToggle={e=>setCompatibilityOpen(e.currentTarget.open)}><summary>{t("Upload compatibility")}</summary><label className="ref-checkbox"><input type="checkbox" checked={bufferedHttps} disabled={locked} onChange={e=>setBufferedHttps(e.target.checked)}/>{t("Use buffered HTTPS upload (16 MiB)")}</label><p className="hint">{t("Try after an HTTP 400 upload failure. Buffers up to 16 MiB to avoid chunked transfer for smaller packs. Applies only to this push; Git configuration stays unchanged. Review the live destination again before retrying.")}</p></details><p className="hint">{t("HTTPS uses the matching GitNova host account when available, otherwise configured Git credentials. SSH uses your agent. Pre-push hooks run only after confirmation. Only this branch is published; no force push or tag publication.")}</p></> : <p className="hint">{t("Changes tracking configuration only. It does not fetch, pull, push or switch branches.")}</p>}
      {mode === 'upstream' && <button type="button" disabled={locked || !remote} onClick={() => void fetch()}>{t("Fetch branches from")} {remote || t("remote")}</button>}
      {mode==='push'&&listing.remotes.length>0&&<button type="button" disabled={locked} onClick={()=>setSettingUp(true)}>{t("Add remote…")}</button>}
      <div className="graph-action-footer"><button type="button" disabled={writing} onClick={() => chooseMode('actions')}>{t("Back")}</button><button className="primary" disabled={locked || !remote || !destination}>{mode === 'push' ? t("Check destination & review") : t("Review upstream")}</button></div>
    </form> : <>
      {loading && <p role="status">{editMode === 'push' ? t("Checking the live push destination and outgoing commits…") : t("Inspecting selected ref, configuration and worktrees…")}</p>}
      {refReview && <div className="ref-operation-review"><strong>{label} · {name}</strong>
        {refReview.action.kind === 'renameBranch' && <p>{name} → <b>{refReview.action.newName}</b><br />{t("Commit")} <code>{refReview.objectOid.slice(0, 12)}</code> {t("stays in place.")}</p>}
        {(refReview.action.kind === 'setUpstream' || refReview.action.kind === 'unsetUpstream') && <p>{t("Upstream:")} {refReview.upstreamBefore ?? t("None")} → <b>{refReview.upstreamAfter ?? t("None")}</b></p>}
        {refReview.recoveryRef && <><p>{t("Removes only this local")} {tag ? t("tag") : t("branch")}{t(". Commits and the original tag object remain reachable through:")}</p><code className="recovery-ref">{refReview.recoveryRef}</code><p className="hint">{t("Recovery object:")} {refReview.objectOid}{t(". This does not delete a remote branch/tag.")}</p>{refReview.retainedBy.length > 0 ? <p>{t("Also retained by")} {refReview.retainedBy.join(', ')}</p> : <p className="notice">{t("No other local branch retains this tip. The recovery ref will preserve its commits.")}</p>}</>}
        {refReview.annotation && <details><summary>{t("Annotated tag object preserved in recovery")}</summary><pre>{refReview.annotation}</pre></details>}
        {refReview.blockedReason && <p className="notice" role="status">{t(refReview.blockedReason)}</p>}
        <div className="graph-action-footer"><button disabled={writing} onClick={() => chooseMode(editMode)}>{t("Back")}</button><button className={refReview.recoveryRef ? 'danger-action' : 'primary'} disabled={locked || Boolean(refReview.blockedReason)} onClick={() => void apply()}>{writing ? t("Applying…") : label}</button></div>
      </div>}
      {pushReview && <div className="push-review"><strong>{pushReview.request.branch} → {pushReview.request.remote}/{pushReview.request.remoteBranch}</strong><p className="push-destination">{pushReview.destination}</p><p>{pushReview.remoteOid ? t("Remote {0}", [pushReview.remoteOid.slice(0, 12)]) : t("Creates a new remote branch")} → <code>{pushReview.request.oid.slice(0, 12)}</code></p>
        {pushReview.blockedReason ? <p>{t("Push is blocked. Resolve the destination issue before reviewing outgoing commits.")}</p> : <><p><b>{pushReview.outgoingCount} {t(pushReview.remoteOid?"outgoing commits":"commits in this branch history")}</b> · {pushReview.remoteOid ? t("Fast-forward only") : t("First publication to this branch")}</p>{!pushReview.remoteOid&&<p className="hint">{t("A new remote branch points to this complete history. These are not necessarily new commits or objects to upload.")}</p>}{pushReview.outgoing.length<pushReview.outgoingCount&&<p className="operation-warning">{t("Showing the latest {0} of {1} commits. Push publishes the complete branch at the reviewed SHA; only this displayed list is shortened.",[pushReview.outgoing.length,pushReview.outgoingCount])}</p>}<div className="outgoing-commits">{pushReview.outgoing.map(commit => <div key={commit.oid}><code>{commit.oid.slice(0, 8)}</code><span>{commit.subject}</span></div>)}</div></>}
        <p>{pushReview.trackingRef ? t("Refreshes cached {0} after remote verification.", [pushReview.trackingRef.replace('refs/remotes/', '')]) : t("Fetch/push URLs or mappings differ; cached fetch refs stay unchanged.")}</p>
        {pushReview.request.bufferedHttps&&<p className="notice">{t("Buffered HTTPS upload selected: 16 MiB. No automatic retry.")}</p>}
        {pushReview.request.setUpstream && <p>{t("Upstream:")} {pushReview.upstreamBefore ?? t("None")} → <b>{pushReview.request.remote}/{pushReview.request.remoteBranch}</b></p>}
        <p className="hint">{t("Configured pre-push hooks and push signing can run after confirmation. Destination checks use configured Git credentials/SSH. Remote publication and local tracking/configuration are separate effects; failures require inspection before retrying.")}</p>
        {pushReview.blockedReason && <p className="notice" role="status">{t(pushReview.blockedReason)}</p>}
        <div className="graph-action-footer"><button disabled={writing} onClick={() => chooseMode('push')}>{t("Back")}</button><button className="primary" disabled={locked || Boolean(pushReview.blockedReason)} onClick={() => void apply()}>{writing ? t("Pushing…") : t("Push reviewed branch")}</button></div>
      </div>}
    </>}
    {writing && <p role="status">{t("Git operation in progress…")}</p>}{error && <div className="inline-error" role="alert">{t(error)}</div>}
  </div>;
}
