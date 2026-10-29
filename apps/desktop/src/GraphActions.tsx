import { t } from './i18n';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { WorktreeCreateReview } from './WorktreeCreateReview';
import { MergeStartReview } from './MergeStartReview';
import { HostingControls } from './HostingControls';
import { TagControls } from './TagControls';
import { SyncReview } from './SyncReview';
import { RemoteDeleteControls } from './RemoteDeleteControls';
import { RefManager } from './RefManager';
import { api, errorMessage } from './api';
import type { BranchTracking, BranchSwitchPreview, GitRef, GraphAction, GraphActionPreview, OperationReceipt, RemoteSwitchPreview, RepoStatus } from './contracts';
import type { GraphIntent } from './History';
export function GraphActions({ onConfigureRemote, pendingOperation = false, tracking = [], intent, membershipId, refs, status, blocked, onClose, onInspect, onBusyChange, onDone, onFailure }: {
  onConfigureRemote?:()=>void; pendingOperation?: boolean; tracking?: BranchTracking[]; intent: GraphIntent; membershipId: string; refs: GitRef[]; status: RepoStatus; blocked: boolean;
  onClose: () => void; onInspect: (oid: string) => void; onBusyChange: (value: boolean) => void;
  onDone: (receipt: OperationReceipt) => Promise<void>; onFailure: (id: string, message: string) => void;
}) {
  const root = useRef<HTMLDivElement>(null); const inFlight = useRef(false); const sequence = useRef(0);
  const [position, setPosition] = useState({ left: intent.x, top: intent.y });
  const [mode, setMode] = useState<'fetch' | 'pull' | 'menu' | 'createBranch' | 'createTag' | 'pushTag' | 'hosting' | 'worktree' | 'manage' | 'deleteLocal' | 'deleteRemote' | 'push' | 'merge' | 'cherryPick' | 'revert' | 'rebase' | 'review'>('menu');
  const [managedRef, setManagedRef] = useState<GitRef | null>(null);
  const [name, setName] = useState(''); const [error, setError] = useState(''); const [loading, setLoading] = useState(false); const [writing, setWriting] = useState(false); const [phase, setPhase] = useState('');
  const [branchPreview, setBranchPreview] = useState<BranchSwitchPreview | null>(null);
  const [remotePreview, setRemotePreview] = useState<RemoteSwitchPreview | null>(null);
  const [graphPreview, setGraphPreview] = useState<GraphActionPreview | null>(null);
  const targetRefs = refs.filter(ref => ref.oid === intent.oid && /^refs\/(heads|remotes|tags)\//.test(ref.name) && !(ref.name.startsWith('refs/remotes/') && ref.name.endsWith('/HEAD')));
  const locals = targetRefs.filter(ref => ref.name.startsWith('refs/heads/'));
  const tags = targetRefs.filter(ref => ref.name.startsWith('refs/tags/'));
  const remotes = targetRefs.filter(ref => ref.name.startsWith('refs/remotes/'));
  const locked = blocked || loading || writing;
  const clear = () => { sequence.current++; setLoading(false); setBranchPreview(null); setRemotePreview(null); setGraphPreview(null); setError(''); };
  const inspectAction = async (action: GraphAction) => {
    if (blocked || inFlight.current) return; const request = ++sequence.current;
    setLoading(true); setError(''); setMode('review'); setBranchPreview(null); setRemotePreview(null); setGraphPreview(null);
    try { const value = await api.previewGraphAction(membershipId, action); if (request === sequence.current) setGraphPreview(value); }
    catch (e) { if (request === sequence.current) setError(errorMessage(e)); }
    finally { if (request === sequence.current) setLoading(false); }
  };
  const openRef = async (ref: string) => {
    if (blocked || inFlight.current) return;
    if (ref.startsWith('refs/heads/')) {
      const request = ++sequence.current; setLoading(true); setError(''); setMode('review'); setGraphPreview(null); setRemotePreview(null); setBranchPreview(null);
      try { const value = await api.previewBranchSwitch(membershipId, ref.slice(11)); if (request === sequence.current) setBranchPreview(value); }
      catch (e) { if (request === sequence.current) setError(errorMessage(e)); }
      finally { if (request === sequence.current) setLoading(false); }
    } else if (ref.startsWith('refs/remotes/')) {
      inFlight.current = true; setWriting(true); onBusyChange(true); setMode('review'); setError(''); setGraphPreview(null); setBranchPreview(null); setRemotePreview(null); setPhase(t("Fetching selected remote branch…"));
      try {
        const listing = await api.remoteBranches(membershipId);
        const remote = listing.branches.find(branch => `refs/remotes/${branch.remote}/${branch.name}` === ref);
        if (!remote) throw new Error('This remote branch is no longer available. Refresh the graph and select it again.');
        const receipt = await api.fetchRemote(membershipId, remote.remote, remote.name); await onDone(receipt);
        if (receipt.status !== 'succeeded') { setError(`${t(receipt.summary)} ${receipt.recovery ?? ''}`); return; }
        setPhase(t("Preparing local checkout and pull preview…"));
        const review = await api.previewRemoteSwitch(membershipId, remote.remote, remote.name); setRemotePreview(review); setBranchPreview(review.switch);
      } catch (e) { setError(errorMessage(e)); onFailure(membershipId, errorMessage(e)); }
      finally { inFlight.current = false; setWriting(false); onBusyChange(false); setPhase(''); }
    } else { await inspectAction({ kind: 'checkoutCommit', oid: intent.oid, name: '' }); }
  };
  useEffect(() => {
    clear(); setMode('menu'); setName('');
    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled && (intent.action === 'fetch' || intent.action === 'pull')) { setMode(intent.action); return; }
      if (!cancelled && intent.action === 'push') { const ref = targetRefs.find(item => item.name === intent.refName && item.name.startsWith('refs/heads/')); if (ref) { setManagedRef(ref); setMode('push'); } else setError('This branch changed. Refresh and select it again.'); return; }
      if (!cancelled && intent.quick && !blocked && !status.bare) {
        const exact = targetRefs.find(ref => ref.name === intent.refName);
        const preferred = exact ?? (locals.length === 1 ? locals[0] : locals.length === 0 && remotes.length === 1 ? remotes[0] : undefined);
        if (preferred) void openRef(preferred.name);
        else if (!locals.length && !remotes.length) void inspectAction({ kind: 'checkoutCommit', oid: intent.oid, name: '' });
      }
    });
    return () => { cancelled = true; sequence.current++; };
    // Each intent is a new user request. Ref refresh after fetch must not replay it.
  }, [intent]);
  useLayoutEffect(() => {
    const node = root.current; if (!node) return;
    const fit = () => { const box = node.getBoundingClientRect(); setPosition({ left: Math.max(12, Math.min(intent.x, window.innerWidth - box.width - 12)), top: Math.max(12, Math.min(intent.y, window.innerHeight - box.height - 12)) }); };
    const observer = new ResizeObserver(fit); observer.observe(node); window.addEventListener('resize', fit); fit();
    return () => { observer.disconnect(); window.removeEventListener('resize', fit); };
  }, [intent]);
  useEffect(() => {
    requestAnimationFrame(() => root.current?.querySelector<HTMLElement>('input, button:not(:disabled)')?.focus());
  }, [mode, loading]);
  useEffect(() => {
    const close = (event: PointerEvent) => { if ((event.target as Element)?.closest?.('dialog[open]')) return; if (!inFlight.current && !root.current?.contains(event.target as Node)) onClose(); };
    const key = (event: KeyboardEvent) => { if ((event.target as Element)?.closest?.('dialog[open]')) return;
      if (event.key === 'Escape' && !inFlight.current) { event.preventDefault(); event.stopImmediatePropagation(); onClose(); document.getElementById(`commit-${intent.oid}`)?.focus(); }
      if (event.key === 'Tab' && root.current?.contains(event.target as Node)) {
        const items = [...root.current.querySelectorAll<HTMLElement>('input:not(:disabled),select:not(:disabled),button:not(:disabled),summary'
)];
        const index = items.indexOf(document.activeElement as HTMLElement); if (!items.length) return;
        event.preventDefault(); items[(index + (event.shiftKey ? -1 : 1) + items.length) % items.length].focus();
      }
      if (mode === 'menu' && ['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key) && root.current?.contains(event.target as Node)) {
        const items = [...root.current.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')]; if (!items.length) return;
        const index = items.indexOf(document.activeElement as HTMLButtonElement); event.preventDefault(); event.stopPropagation();
        items[event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : (index + (event.key === 'ArrowUp' ? -1 : 1) + items.length) % items.length].focus();
      }
    };
    document.addEventListener('pointerdown', close); document.addEventListener('keydown', key, true);
    return () => { document.removeEventListener('pointerdown', close); document.removeEventListener('keydown', key, true); };
  }, [onClose, intent.oid, mode]);
  const copy = async (value: string) => { try { await navigator.clipboard.writeText(value); onClose(); } catch (e) { setError(`Cannot copy: ${errorMessage(e)}`); } };
  const apply = async () => {
    if (locked || inFlight.current || (!branchPreview && !graphPreview)) return;
    inFlight.current = true; setWriting(true); onBusyChange(true); setError('');
    try {
      const receipt = graphPreview ? await api.applyGraphAction(membershipId, graphPreview.action, graphPreview.token) : remotePreview ? await api.switchRemoteBranch(membershipId, remotePreview.remote, remotePreview.remoteBranch, branchPreview!.token) : await api.switchBranch(membershipId, branchPreview!.branch, branchPreview!.token);
      await onDone(receipt); if (receipt.status === 'succeeded') onClose(); else { setError(`${t(receipt.summary)} ${receipt.recovery ?? ''}`); setGraphPreview(null); setBranchPreview(null); setRemotePreview(null); }
    } catch (e) { setError(errorMessage(e)); onFailure(membershipId, errorMessage(e)); setGraphPreview(null); setBranchPreview(null); setRemotePreview(null); }
    finally { inFlight.current = false; setWriting(false); onBusyChange(false); }
  };
  const preview = graphPreview?.switch ?? branchPreview;
  const reason = graphPreview?.blockedReason ?? preview?.blockedReason;
  const label = graphPreview ? graphPreview.action.kind === 'checkoutCommit' ? t("Checkout commit (detached)") : graphPreview.action.kind === 'createBranch' ? t("Create & open {0}", [graphPreview.action.name]) : t("Create tag {0}", [graphPreview.action.name]) : remotePreview ? t(remotePreview.create ? 'Create & open {0}' : 'Open & pull {0}', [preview?.branch]) : t("Switch to {0}", [preview?.branch]);
  return <div ref={root} className="graph-actions" style={position} role={mode === 'menu' ? 'menu' : 'region'} aria-label={mode === 'menu' ? t("Commit actions") : t("Review graph action")}>
    <div className="graph-actions-heading"><strong>{mode === 'fetch' ? t('Fetch') : mode === 'pull' ? t('Pull') : <>{t("Commit")} {intent.oid.slice(0, 8)}</>}</strong><small>{status.branch ? mode === 'hosting' ? t('Checked out: {0}', [status.branch]) : t("On {0}", [status.branch]) : t("Detached HEAD")}</small></div>
    {mode === 'menu' ? <>
      <button role="menuitem" onClick={() => { onInspect(intent.oid); onClose(); }}>{t("Inspect commit")}</button>
      {(intent.workflows?.length?intent.workflows:intent.pipeline?[intent.pipeline]:[]).map(run=><button key={run.url} role="menuitem" disabled={locked} onClick={()=>{if(locked)return;setLoading(true);setError('');void api.openCiUrl(run.url).then(onClose).catch(e=>setError(errorMessage(e))).finally(()=>setLoading(false));}}>{t(run.kind==='workflow'?'Open workflow #{0}':'Open pipeline #{0}',[run.id])}{run.name?` · ${run.name}`:''}</button>)}
      {!!locals.length && <span className="graph-action-group">{t("Local branches")}</span>}{locals.map(ref => <button role="menuitem" key={ref.name} disabled={locked || status.bare || status.branch === ref.name.slice(11)} title={status.branch === ref.name.slice(11) ? t("Current branch") : t("Preview checkout")} onClick={() => void openRef(ref.name)}>{t("Checkout")} {ref.name.slice(11)}{status.branch === ref.name.slice(11) ? t(" · current") : ''}</button>)}
      {locals.some(ref => ref.name === `refs/heads/${status.branch}`) && <button role="menuitem" disabled={locked || status.bare || pendingOperation} title={pendingOperation ? t('Finish or recover the current operation before pulling.') : undefined} onClick={() => { clear(); setMode('pull'); }}>{t('Pull into {0}…', [status.branch])}</button>}
      <button role="menuitem" disabled={locked} onClick={() => { clear(); setMode('fetch'); }}>{t('Fetch…')}</button>
      {locals.map(ref => <button role="menuitem" key={`push-${ref.name}`} title={pendingOperation ? t('Finish or recover the current operation before pushing.') : undefined} disabled={locked || status.bare || pendingOperation} onClick={() => { clear(); setManagedRef(ref); setMode('push'); }}>{t('Push {0}…', [ref.name.slice(11)])}</button>)}
      {locals.map(ref => <button role="menuitem" key={`manage-${ref.name}`} disabled={locked || status.bare} onClick={() => { clear(); setManagedRef(ref); setMode('manage'); }}>{t("Manage branch")} {ref.name.slice(11)}…</button>)}
      {locals.map(ref=><button role="menuitem" key={`delete-local-${ref.name}`} disabled={locked||status.bare||pendingOperation} onClick={()=>{clear();setManagedRef(ref);setMode('deleteLocal');}}>{t('Delete local branch {0}…',[ref.name.slice(11)])}</button>)}
      {locals.map(ref=><button role="menuitem" key={`hosting-${ref.name}`} disabled={locked||status.bare||pendingOperation} onClick={()=>{clear();setManagedRef(ref);setMode('hosting');}}>{t('Create / open PR or MR for {0}…',[ref.name.slice(11)])}</button>)}
      {!!tags.length && <span className="graph-action-group">{t("Tags")}</span>}{tags.map(ref => <button role="menuitem" key={ref.name} disabled={locked || status.bare} onClick={() => { clear(); setManagedRef(ref); setMode('manage'); }}>{t("Manage tag")} {ref.name.slice(10)}…</button>)}
      {tags.map(ref => <button role="menuitem" key={`publish-${ref.name}`} disabled={locked || status.bare || pendingOperation} onClick={() => { clear(); setManagedRef(ref); setMode('pushTag'); }}>{t('Push tag {0}…',[ref.name.slice(10)])}</button>)}
      {!!remotes.length && <span className="graph-action-group">{t("Remote branches")}</span>}{remotes.map(ref => <button role="menuitem" key={ref.name} disabled={locked || status.bare} onClick={() => void openRef(ref.name)}>{t("Fetch & open")} {ref.name.slice(13)}…</button>)}
      {remotes.map(ref=><button role="menuitem" key={`delete-remote-${ref.name}`} disabled={locked||status.bare||pendingOperation} onClick={()=>{clear();setManagedRef(ref);setMode('deleteRemote');}}>{t('Delete remote branch {0}…',[ref.name.slice(13)])}</button>)}
      <span className="graph-action-group">{t("At this commit")}</span>
      <button role="menuitem" disabled={locked || status.bare} onClick={() => void inspectAction({ kind: 'checkoutCommit', oid: intent.oid, name: '' })}>{t("Checkout commit (detached)…")}</button>
      <button role="menuitem" disabled={locked || status.bare || !status.branch || !status.head} onClick={() => { clear(); setMode('merge'); }}>{t("Merge into")} {status.branch ?? t("current branch")}…</button>
      <button role="menuitem" disabled={locked || status.bare || !status.branch || !status.head} onClick={() => { clear(); setMode('cherryPick'); }}>{t("Cherry-pick onto")} {status.branch ?? t("current branch")}…</button>
      <button role="menuitem" disabled={locked||status.bare||!status.branch||!status.head||pendingOperation} onClick={()=>{clear();setMode('rebase');}}>{t('Rebase {0} onto this commit…',[status.branch??''])}</button>
      <button role="menuitem" disabled={locked || status.bare || !status.branch || !status.head} onClick={() => { clear(); setMode('revert'); }}>{t("Revert on")} {status.branch ?? t("current branch")}…</button>
      <button role="menuitem" disabled={locked || status.bare} onClick={() => { clear(); setMode('createBranch'); }}>{t("Create branch here…")}</button>
      <button role="menuitem" disabled={locked || status.bare} onClick={() => { clear(); setMode('worktree'); }}>{t("Create worktree here…")}</button>
      <button role="menuitem" disabled={locked || status.bare} onClick={() => { clear(); setMode('createTag'); }}>{t("Create tag…")}</button>
      {blocked && <p className="hint">{t("Wait for the current Git operation before changing this repository.")}</p>}{status.bare && <p className="hint">{t("Open a worktree to use checkout and ref creation here.")}</p>}
      <span className="graph-action-group">{t("Copy")}</span><button role="menuitem" onClick={() => void copy(intent.oid)}>{t("Copy commit SHA")}</button>
      {targetRefs.map(ref => <button role="menuitem" key={`copy-${ref.name}`} onClick={() => void copy(ref.name.replace(/^refs\/(heads|remotes|tags)\//, ''))}>{t("Copy")} {ref.name.startsWith('refs/tags/') ? t("tag") : t("branch")} {t("name:")} {ref.name.replace(/^refs\/(heads|remotes|tags)\//, '')}</button>)}
    </> : mode === 'deleteRemote' && managedRef ? <RemoteDeleteControls membershipId={membershipId} selectedRef={managedRef} blocked={blocked||pendingOperation} onBack={()=>setMode('menu')} onClose={onClose} onDone={onDone} onBusyChange={value=>{inFlight.current=value;setWriting(value);onBusyChange(value);}} onFailure={onFailure}/> : mode === 'hosting' && managedRef ? <HostingControls membershipId={membershipId} branch={managedRef.name.slice(11)} oid={managedRef.oid} tracking={tracking.find(value=>value.branch===managedRef.name.slice(11))} blocked={blocked} onBusyChange={value=>{inFlight.current=value;setWriting(value);onBusyChange(value);}} onDone={onDone} onClose={onClose} onBack={()=>setMode('menu')} onFailure={onFailure}/> : mode === 'createTag' || mode === 'pushTag' ? <TagControls key={`${mode}-${managedRef?.name??''}`} membershipId={membershipId} oid={intent.oid} initialPush={mode==='pushTag'} initialName={mode==='pushTag'?managedRef?.name.slice(10):''} blocked={blocked} onBusyChange={value=>{inFlight.current=value;setWriting(value);onBusyChange(value);}} onDone={onDone} onClose={onClose} onBack={()=>setMode('menu')} onFailure={onFailure}/> : mode === 'fetch' || mode === 'pull' ? <SyncReview key={mode} kind={mode} membershipId={membershipId} branch={status.branch} tracking={tracking.find(item => item.branch === status.branch)} onFailure={onFailure} onReviewWork={() => { onInspect('working'); onClose(); }} blocked={blocked || mode === 'pull' && pendingOperation} onBusyChange={value => { inFlight.current = value; setWriting(value); onBusyChange(value); }} onDone={onDone} onClose={onClose} /> : mode === 'worktree' ? <WorktreeCreateReview membershipId={membershipId} oid={intent.oid} locals={locals} blocked={blocked} onBack={() => setMode('menu')} onBusyChange={value => { inFlight.current = value; setWriting(value); onBusyChange(value); }} onDone={onDone} onClose={onClose} onFailure={onFailure} /> : (mode === 'merge' || mode === 'cherryPick' || mode === 'revert' || mode === 'rebase') ? <MergeStartReview onReviewWork={() => { onInspect('working'); onClose(); }} operation={mode} membershipId={membershipId} oid={intent.oid} sourceRef={intent.refName ?? (locals.length === 1 ? locals[0].name : null)} blocked={blocked} onBack={() => setMode('menu')} onBusyChange={value => { inFlight.current = value; setWriting(value); onBusyChange(value); }} onDone={onDone} onClose={onClose} onFailure={onFailure} /> : (mode === 'manage' || mode === 'push' || mode === 'deleteLocal') && managedRef ? <RefManager onConfigureRemote={onConfigureRemote} initialMode={mode === 'push' ? 'push' : mode === 'deleteLocal' ? 'delete' : 'actions'} tracking={tracking.find(item => `refs/heads/${item.branch}` === managedRef.name && item.oid === managedRef.oid)} key={`${managedRef.name}:${mode}`} membershipId={membershipId} selectedRef={managedRef} blocked={blocked || mode === 'push' && pendingOperation} onBack={() => { clear(); setMode('menu'); }} onBusyChange={value => { inFlight.current = value; setWriting(value); onBusyChange(value); }} onDone={onDone} onClose={onClose} onFailure={onFailure} /> : mode === 'createBranch' ? <form onSubmit={event => { event.preventDefault(); void inspectAction({ kind: mode, oid: intent.oid, name }); }}>
      <label htmlFor="graph-ref-name">{t("New branch name")}</label><input id="graph-ref-name" value={name} onChange={event => setName(event.target.value)} maxLength={256} autoComplete="off" spellCheck={false} />
      <p className="hint">{t("Creates a new branch at this commit and opens it. Existing branches stay in place.")}</p><div className="graph-action-footer"><button type="button" onClick={() => { clear(); setMode('menu'); }}>{t("Back")}</button><button className="primary" disabled={!name || locked}>{t("Review")} {t("branch")}</button></div>
    </form> : <>
      {(loading || writing) && <p role="status">{writing ? phase || t("Applying reviewed Git action…") : t("Inspecting checkout, files and local branches…")}</p>}
      {preview && !reason && <div className="graph-action-impact"><strong>{preview.sourceBranch ?? t("Detached {0}", [preview.sourceOid?.slice(0, 8) ?? ''])} → {preview.branch}</strong><p>{t("Target")} <code>{preview.targetOid.slice(0, 12)}</code> · {preview.files.length} {t("paths change")}</p><div className="branch-preview-files">{preview.files.map(file => <div key={file.path.hex}><span>{file.status}</span><code>{file.path.display}</code></div>)}</div>{preview.submodules.length > 0 && <div className="branch-submodules"><strong>{t("Submodule checkouts stay in place")}</strong>{preview.submodules.map(sub => <div key={sub.path.hex}><code>{sub.path.display}</code><span>{t("Recorded")} {sub.sourceOid.slice(0, 8)} → {sub.targetOid.slice(0, 8)}</span><span>{t("Checkout")} {sub.checkedOutOid?.slice(0, 8) ?? t("not initialized")} {t("· preserved")}</span></div>)}</div>}</div>}
      {remotePreview && <p>{remotePreview.create ? t("Creates a local tracking branch at the fetched revision.") : t("{0} commits ahead · {1} to pull. Fast-forward only.", [remotePreview.ahead, remotePreview.behind])}</p>}
      {graphPreview?.action.kind === 'checkoutCommit' && <p className="notice">{t("Detached HEAD: you are opening a commit directly. Create a branch before making commits you want to keep.")}</p>}
      {graphPreview?.action.kind === 'createTag' && <p>{t("Create lightweight tag")} <strong>{graphPreview.action.name}</strong> {t("at")} <code>{graphPreview.action.oid.slice(0, 12)}</code>{t(". Checkout and files stay in place.")}</p>}
      {(graphPreview || branchPreview) && <>{reason ? <p className="notice" role="status">{t(reason)}</p> : <p className="hint">{t("Configured Git hooks and checkout filters can run. No force, reset or automatic stash.")}</p>}<div className="graph-action-footer"><button disabled={writing} onClick={() => { clear(); setMode('menu'); }}>{t("Back")}</button><button className="primary" disabled={locked || Boolean(reason)} onClick={() => void apply()}>{writing ? t("Applying…") : label}</button></div></>}
      {!loading && !writing && !graphPreview && !branchPreview && <button onClick={() => { clear(); setMode('menu'); }}>{t("Back to actions")}</button>}
    </>}
    {error && <div className="inline-error" role="alert">{t(error)}</div>}
  </div>;
}
