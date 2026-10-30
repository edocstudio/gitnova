import type {TerminalEntry} from './RepositoryTerminal';
import {lazy,Suspense} from 'react';
import {useAutoFetch} from './useAutoFetch';
import {reuseObserved} from './observed-state';
import {RemoteFooter} from './RemoteFooter';
import brandIcon from '../assets/icon.svg';
import { t, useLocale } from './i18n';
import { Settings } from './Settings';
import { useCallback, useEffect, useRef, useState } from 'react';
import { api, desktop, errorMessage } from './api';
import type { StashList, SearchLocation, MergeState, Catalog, CommitDetail, LinkedWorktree, Submodule, WorkingChanges, OperationReceipt, PlannedOperation } from './contracts';
import { historyItems, appendPage, retainRecent, retainGraphSessions, MAX_ROWS, ROW_HEIGHT } from './graph-state';
import { CommitSearch } from './CommitSearch';
import { Icon } from './Icons';
import { CommitDiffDock } from './CommitDiffDock';
import { CommitInspector } from './CommitInspector';
import { GraphActions } from './GraphActions';
import type { GraphIntent } from './History';
import { History, WORKING } from './History';
import type { GraphSession as Session } from './History';
import { RepositoryList } from './RepositoryList';
import { InitControls } from './InitControls';
import { CloneControls } from './CloneControls';
import type { CloneDraft } from './CloneControls';
import { BranchSwitcher } from './BranchSwitcher';
import { WorktreeEntry } from './WorktreeEntry';
import { SubmoduleEntry } from './SubmoduleEntry';
import { BranchSubmodules } from './BranchSubmodules';
import { MergeInspector } from './MergeInspector';
import { WorkingDetails } from './WorkingDetails';
import type { WorkingDiff } from './WorkingDetails';
const RepositoryTerminal=lazy(()=>import('./RepositoryTerminal').then(module=>({default:module.RepositoryTerminal})));
export function App() {
  useLocale();
  const [repositoryViews,setRepositoryViews]=useState<Record<string,{query:string;page:number}>>({});
  const [terminalEntries,setTerminalEntries]=useState<TerminalEntry[]>([]);
  const [terminalVisible,setTerminalVisible]=useState(false);
  const [settingsOpen,setSettingsOpen]=useState(false);
  const settingsButton=useRef<HTMLButtonElement>(null);
  const settingsReturn=useRef<HTMLElement|null>(null);
  const closeSettings=useCallback(()=>{setSettingsOpen(false);requestAnimationFrame(()=>{const target=settingsReturn.current;settingsReturn.current=null;(target?.isConnected?target:settingsButton.current)?.focus();});},[]);
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  const [sessions, setSessions] = useState<Record<string, Session>>({});
  const [changes, setChanges] = useState<Record<string, WorkingChanges>>({});
  const [merges, setMerges] = useState<Record<string, MergeState>>({});
  const [mergeDrafts, setMergeDrafts] = useState<Record<string, { session: string | null; text: string }>>({});
  const [workingDiff, setWorkingDiff] = useState<WorkingDiff>(null);
  const [stashLaunch, setStashLaunch] = useState<{ id: string; kind: 'push' | 'pop'; nonce: number } | null>(null);
  const [stashes, setStashes] = useState<Record<string, StashList>>({});
  const [stashErrors, setStashErrors] = useState<Record<string, string>>({});
  const stashRef = useRef(stashes); stashRef.current = stashes;
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [searchOpen, setSearchOpen] = useState(false);
  const [cloneDrafts, setCloneDrafts] = useState<Record<string, CloneDraft>>({});
  const [menu, setMenu] = useState(false); const [path, setPath] = useState(''); const [workspaceName, setWorkspaceName] = useState('');
  const [operationFeedback, setOperationFeedback] = useState<Record<string, { status: string; summary: string; kind?: string; planned?: PlannedOperation | null; recovery: string | null; branchCleanup?: OperationReceipt['branchCleanup'] }>>({});
  const reportOperation = useCallback((receipt: OperationReceipt) => {window.dispatchEvent(new Event('gitnova-write-completed'));setOperationFeedback(old => retainRecent(old, receipt.membershipId, receipt));}, []);
  const reportOperationError = useCallback((id: string, message: string) => setOperationFeedback(old => retainRecent(old, id, { status: 'unknown', summary: message, recovery: 'Inspect the repository before retrying.' })), []);
  const [error, setError] = useState(''); const [syncError, setSyncError] = useState('');
  const [busy, setBusy] = useState(false); const [graphBusy, setGraphBusy] = useState(false); const [detailBusy, setDetailBusy] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [diffCollapsed, setDiffCollapsed] = useState(false);
  const [diffHeight, setDiffHeight] = useState<number | null>(null);
  const [detail, setDetail] = useState<CommitDetail | null>(null);
  const [related, setRelated] = useState<{ subs: Submodule[]; trees: LinkedWorktree[]; error: string; loading: boolean }>({ subs: [], trees: [], error: '', loading: false });
  const picker = useRef<HTMLButtonElement>(null); const menuRoot = useRef<HTMLDivElement>(null);
  const workingSequence = useRef<Record<string, number>>({});
  const sequence = useRef(0); const detailSequence = useRef(0); const sessionRef = useRef(sessions); sessionRef.current = sessions;
  const workspace = catalog?.workspaces.find(w => w.id === catalog.activeWorkspaceId);
  const selected = workspace?.memberships.find(m => m.id === workspace.selectedId); const selectedId = selected?.id;
  const [graphIntent, setGraphIntent] = useState<GraphIntent | null>(null);
  const [foregroundWriteBusy, setWriteBusy] = useState(false);
  const [autoFetchBusy,setAutoFetchBusy]=useState(false);
  const writeBusy=foregroundWriteBusy||autoFetchBusy;
  const writingRef = useRef(false);
  const writeEpoch = useRef(0);
  const onWriteBusy = useCallback((value: boolean) => { writingRef.current = value; if (value) { writeEpoch.current++; setSearchOpen(false); } setWriteBusy(value); }, []);
  const activeId = useRef(selectedId); activeId.current = selectedId;
  const session = selectedId ? sessions[selectedId] : undefined; const currentStatus = session?.page.status;
  const selectedOid = session?.selected; const stashMatch = selectedOid?.match(/^stash:(\d+):([a-fA-F0-9]{40,64})$/); const selectedStash = stashMatch ? { index: Number(stashMatch[1]), oid: stashMatch[2] } : undefined; const working = selectedOid === WORKING || !!selectedStash;
  const rootPath = selected?.worktreeId ? catalog?.worktrees.find(w => w.id === selected.worktreeId)?.root.display : catalog?.repositories.find(r => r.id === selected?.repositoryId)?.commonDir.display;
  useEffect(() => { api.catalog().then(setCatalog).catch(e => setError(errorMessage(e))); }, []);
  useEffect(() => {
    const key = (event: KeyboardEvent) => { if ((event.metaKey || event.ctrlKey) && event.key === ',') { event.preventDefault(); if (!writingRef.current) { setMenu(false);setSearchOpen(false);setSettingsOpen(true); } return; } if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'o') { event.preventDefault(); if (!writingRef.current) { setSearchOpen(false); setMenu(true); } } if (event.key === 'Escape' && menu && !writingRef.current) { setMenu(false); picker.current?.focus(); } };
    window.addEventListener('keydown', key); return () => window.removeEventListener('keydown', key);
  }, [menu]);
  useEffect(() => { if (menu) requestAnimationFrame(() => menuRoot.current?.querySelector<HTMLButtonElement | HTMLInputElement>('.repo-filter input, .repo-option-main, #repo-path')?.focus()); }, [menu]);
  useEffect(() => { const click = (event: PointerEvent) => { if (!writingRef.current && menuRoot.current && !menuRoot.current.contains(event.target as Node)) setMenu(false); }; window.addEventListener('pointerdown', click); return () => window.removeEventListener('pointerdown', click); }, []);
  const load = useCallback(async (id: string, older = false, preserve = false) => {
    const request = ++sequence.current; setGraphBusy(true); if (!preserve) setError('');
    try {
      let page = await api.graph(id, older ? sessionRef.current[id]?.page.next : null);
      let refreshed = page.rows;
      const cached = preserve && !older ? sessionRef.current[id] : undefined;
      const unchanged = cached && cached.page.fingerprint === page.fingerprint;
      // Rebuild the cached range after ref changes, keeping bounded memory and valid lane cursors.
      const restoreCount = cached ? Math.min(MAX_ROWS, cached.base + cached.rows.length + 100) : 100;
      while (cached && !unchanged && page.next && refreshed.length < restoreCount) {
        if (request !== sequence.current || activeId.current !== id) return;
        page = await api.graph(id, page.next); refreshed = [...refreshed, ...page.rows];
      }
      if (request !== sequence.current || activeId.current !== id) return;
      setSessions(old => {
        const previous = old[id];
        if (unchanged && previous) {
          return retainGraphSessions(old, id, { ...previous, page: { ...previous.page, refs: page.refs, status: page.status, tracking: page.tracking }, revision: (previous.revision ?? 0) + 1 });
        }
        const joined = older && previous ? appendPage(previous.rows, page.rows) : { rows: refreshed, dropped: 0 };
        const anchor = (preserve || older) && previous ? previous.anchor ?? previous.rows[Math.floor(previous.scroll / ROW_HEIGHT)]?.commit.oid : undefined;
        const anchorIndex = anchor ? historyItems(joined.rows, stashRef.current[id]?.entries ?? []).findIndex(item => item.id === anchor) : -1;
        const scroll = previous?.scroll === 0 ? 0 : (preserve || older) && previous && anchorIndex >= 0 ? anchorIndex * ROW_HEIGHT + (previous.anchorOffset ?? previous.scroll % ROW_HEIGHT) : 0;
        const notice = anchor && anchorIndex < 0 ? 'History changed and the previous position is outside the refreshed range. Showing the latest loaded history.' : undefined;
        const nextSession = { page, rows: joined.rows, selected: older || preserve ? previous?.selected ?? page.rows[0]?.commit.oid ?? null : page.rows[0]?.commit.oid ?? null, scroll, base: older ? (previous?.base ?? 0) + joined.dropped : 0, revision: (previous?.revision ?? 0) + 1, notice, anchor: anchorIndex >= 0 ? anchor : undefined, anchorOffset: previous?.anchorOffset ?? (previous?.scroll ?? 0) % ROW_HEIGHT };
        return retainGraphSessions(old, id, nextSession);
      });
      return page;
    } catch (e) { if (request === sequence.current) setError(errorMessage(e)); }
    finally { if (request === sequence.current) setGraphBusy(false); }
  }, []);
  const refreshWorking = useCallback(async (id: string, forceGraph = false, knownFingerprint?: string) => {
    const epoch = writeEpoch.current;
    const request = (workingSequence.current[id] ?? 0) + 1; workingSequence.current[id] = request;
    const [snapshot, saved] = await Promise.all([api.snapshot(id), api.stashes(id).then(value => ({value, error: ''}), e => ({value: null, error: errorMessage(e)}))]); if (activeId.current !== id || epoch !== writeEpoch.current || workingSequence.current[id] !== request) return;
    if (saved.value) { const value=reuseObserved(stashRef.current[id],saved.value);stashRef.current = retainRecent(stashRef.current, id, value); setStashes(old => old[id]===value ? old : retainRecent(old, id, value)); }
    setStashErrors(old => old[id]===saved.error ? old : { ...old, [id]: saved.error });
    // Working snapshots are mutable and can contain thousands of paths. Only
    // retain the active checkout; switching always obtains a fresh snapshot.
    setChanges(old=>{const value=reuseObserved(old[id],snapshot.changes);return old[id]===value&&Object.keys(old).length===1?old:{[id]:value};});
    setMerges(old=>{const value=reuseObserved(old[id],snapshot.merge);return old[id]===value&&Object.keys(old).length===1?old:{[id]:value};});
    if (snapshot.merge.phase !== 'none') setMergeDrafts(old => old[id]?.session === snapshot.merge.sessionId ? old : { ...old, [id]: { session: snapshot.merge.sessionId, text: snapshot.merge.suggestedMessage ?? `Merge ${snapshot.merge.sourceOid?.slice(0, 8) ?? ''} into ${snapshot.merge.branch ?? 'current branch'}` } });
    if (forceGraph || (sessionRef.current[id]?.page.fingerprint ?? knownFingerprint) !== snapshot.fingerprint) await load(id, false, true);
    else setSessions(old => {const current=old[id];if(!current)return old;const status=reuseObserved(current.page.status,snapshot.changes.status),tracking=reuseObserved(current.page.tracking,snapshot.tracking);return status===current.page.status&&tracking===current.page.tracking?old:retainGraphSessions(old,id,{...current,page:{...current.page,status,tracking}});});
  }, [load]);
  const onAutoFetchBusy=useCallback((value:boolean)=>{writingRef.current=value||foregroundWriteBusy;if(value)writeEpoch.current++;setAutoFetchBusy(value);},[foregroundWriteBusy]);
  const autoFetch=useAutoFetch(selectedId,terminalEntries.length>0||!currentStatus||busy||graphBusy||foregroundWriteBusy||settingsOpen||menu||searchOpen||Boolean(graphIntent)||Boolean(selectedId&&merges[selectedId]&&merges[selectedId].phase!=='none'),onAutoFetchBusy,async id=>{window.dispatchEvent(new Event('gitnova-remotes-changed'));await refreshWorking(id);});
  useEffect(() => {
    sequence.current++; setSearchOpen(false); setGraphIntent(null); setGraphBusy(false); setError(''); setSyncError(''); setDetail(null); setRelated({ subs: [], trees: [], error: '', loading: false });
    if (!selectedId) return;
    const initialLoad = !sessionRef.current[selectedId] ? load(selectedId) : null;
    let cancelled = false; let checking = false;
    const check = async () => {
      if (checking || cancelled || writingRef.current || document.visibilityState !== 'visible') return; checking = true;
      try { const initial = await initialLoad; if (cancelled) return; await refreshWorking(selectedId, false, initial?.fingerprint); if (!cancelled) setSyncError(''); } catch (e) { if (!cancelled) setSyncError(errorMessage(e)); } finally { checking = false; }
    };
    void check(); const timer = window.setInterval(() => void check(), 5000);
    window.addEventListener('focus', check); document.addEventListener('visibilitychange', check);
    return () => { cancelled = true; window.clearInterval(timer); window.removeEventListener('focus', check); document.removeEventListener('visibilitychange', check); };
  }, [selectedId, load, refreshWorking]);
  useEffect(() => {
    if (!selectedId || !menu) return;
    let cancelled = false; let version = 0;
    const inspect = async () => {
      if (writingRef.current) return;
      const epoch = writeEpoch.current;
      const request = ++version; setRelated(old => ({ ...old, loading: true }));
      const [subs, trees] = await Promise.allSettled([api.submodules(selectedId), api.worktrees(selectedId)]);
      if (!cancelled && request === version && epoch === writeEpoch.current) setRelated({ subs: subs.status === 'fulfilled' ? subs.value : [], trees: trees.status === 'fulfilled' ? trees.value : [], error: [subs, trees].filter(result => result.status === 'rejected').map(result => errorMessage((result as PromiseRejectedResult).reason)).join('; '), loading: false });
    };
    void inspect(); window.addEventListener('focus', inspect);
    return () => { cancelled = true; window.removeEventListener('focus', inspect); };
  }, [selectedId, menu]);
  useEffect(() => {
    const request = ++detailSequence.current; setDetail(null); setDetailBusy(false);
    if (!selectedId || !selectedOid || (selectedOid === WORKING || selectedOid.startsWith('stash:'))) return;
    setDetailBusy(true); api.detail(selectedId, selectedOid).then(value => { if (request === detailSequence.current) setDetail(value); }).catch(e => { if (request === detailSequence.current) setError(errorMessage(e)); }).finally(() => { if (request === detailSequence.current) setDetailBusy(false); });
    return () => { detailSequence.current++; };
  }, [selectedId, selectedOid]);
  const mutate = async (action: Promise<Catalog>, close = true) => { setBusy(true); setError(''); try { setCatalog(await action); if (close) { setMenu(false); setPath(''); } } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); } };
  const selectCommit = (oid: string | null) => { if (busy) return; if (!selectedId) return; if (oid !== WORKING) setStashLaunch(null); if (oid) setInspectorOpen(true); if (oid === WORKING || oid?.startsWith('stash:')) setDiffCollapsed(false); setSessions(old => old[selectedId] ? { ...old, [selectedId]: { ...old[selectedId], selected: oid } } : old); };
  const [investigationFile,setInvestigationFile]=useState<{membershipId:string;oid:string;path:string}|null>(null);
  const openSearchResult = async (location: SearchLocation, oid: string, path?: string) => {
    if (!workspace || writingRef.current || busy) throw new Error('Wait for the current operation before opening a result.');
    const origin = activeId.current; const epoch = writeEpoch.current; setBusy(true);
    try {
      const [focus, saved] = await Promise.all([api.focusCommit(workspace.id, location.membershipId, oid), api.stashes(location.membershipId).then(value => ({ value, error: '' }), e => ({ value: null, error: errorMessage(e) }))]);
      if (activeId.current !== origin || writeEpoch.current !== epoch) throw new Error('Repository context changed. Open the result again.');
      if (saved.value) { stashRef.current = retainRecent(stashRef.current, location.membershipId, saved.value); setStashes(old => retainRecent(old, location.membershipId, saved.value!)); }
      setStashErrors(old => ({ ...old, [location.membershipId]: saved.error }));
      const nextCatalog = await api.select(location.membershipId);
      setInvestigationFile(path?{membershipId:location.membershipId,oid,path}:null);
      setDetail(null); setDetailBusy(true); detailSequence.current++; sequence.current++;
      setSessions(old => retainGraphSessions(old, location.membershipId, { page: focus.page, rows: focus.page.rows, selected: oid, base: focus.base, scroll: Math.max(0, historyItems(focus.page.rows, saved.value?.entries ?? stashRef.current[location.membershipId]?.entries ?? []).findIndex(item => item.id === oid) * ROW_HEIGHT - 72), revision: (old[location.membershipId]?.revision ?? 0) + 1 }));
      setCatalog(nextCatalog); setInspectorOpen(true); setDiffCollapsed(false); setGraphIntent(null); setMenu(false);
      requestAnimationFrame(() => document.getElementById(`commit-${oid}`)?.focus({ preventScroll: true }));
    } finally { setBusy(false); }
  };
  const repositoryWritten = async (receipt: OperationReceipt) => {
    reportOperation(receipt);
    if (!selectedId || activeId.current !== selectedId) return;
    await refreshWorking(selectedId);
    if (activeId.current === selectedId && receipt.kind === 'merge_release' && receipt.status === 'succeeded') { selectCommit(WORKING); }
    else if (activeId.current === selectedId && receipt.merge && receipt.merge.phase !== 'none') { selectCommit(WORKING); }
    else if (activeId.current === selectedId && receipt.status === 'succeeded' && receipt.kind !== 'fetch_remote' && receipt.headAfter) {
      setSessions(old => old[selectedId] ? { ...old, [selectedId]: { ...old[selectedId], selected: receipt.kind.startsWith('merge_') ? receipt.headAfter : receipt.planned ? graphIntent?.oid ?? receipt.headAfter : receipt.graphAction?.kind === 'createTag' ? graphIntent?.oid ?? receipt.headAfter : receipt.headAfter } } : old);
    }
  };
  const feedback = selectedId ? operationFeedback[selectedId] : undefined;
  const cleanupProblem = feedback?.branchCleanup && feedback.branchCleanup.status !== 'deleted' ? t('Merge succeeded. Inspect cleanup of {0}: {1}', [feedback.branchCleanup.sourceRef.replace(/^refs\/heads\//, ''), t(feedback.branchCleanup.message)]) : '';
  const problem = cleanupProblem || (feedback && !['succeeded', 'pending', 'conflicted'].includes(feedback.status) ? `${t(feedback.summary)}${feedback.recovery ? ' ' + t(feedback.recovery) : ''}` : '') || error;
  const recoveryLocation = feedback?.planned?.type === 'discard' ? feedback.planned.preview.recoveryDir.display : feedback?.planned?.type === 'merge' ? feedback.planned.preview.recoveryDir?.display ?? feedback.planned.preview.recoveryRef : feedback?.planned?.type === 'amend' || feedback?.planned?.type === 'remoteDelete' || feedback?.planned?.type === 'ref' || feedback?.planned?.type === 'stash' || feedback?.planned?.type === 'worktreeRemove' ? feedback.planned.preview.recoveryRef : null;
  const addForm = <><form className="add-form" onSubmit={event => { event.preventDefault(); if (path.trim()) void mutate(api.add(path)); }}><label htmlFor="repo-path">{t("Add an existing repository")}</label><div className="input-row"><Icon name="folder" /><input id="repo-path" placeholder={t("/absolute/path/to/repository")} value={path} onChange={e => setPath(e.target.value)} disabled={busy || writeBusy || !desktop} required /><button className="primary" disabled={busy || writeBusy || !path.trim() || !desktop}>{t("Open")}<Icon name="arrow" size={14} /></button></div><p className="hint">{t("Works with local repositories, bare repositories and linked worktrees.")}</p></form>{workspace && <InitControls key={`init-${workspace.id}`} workspaceId={workspace.id} defaultPath={path} disabled={busy || writeBusy || !desktop} onBusyChange={onWriteBusy} onCreated={value => { setError(''); setCatalog(value); setMenu(false); setPath(''); }} />}{workspace && <CloneControls key={workspace.id} workspaceId={workspace.id} workspaceName={workspace.name} draft={cloneDrafts[workspace.id] ?? {source:'',destination:''}} onDraft={draft => setCloneDrafts(old => retainRecent(old, workspace.id, draft))} disabled={busy || writeBusy || !desktop} onBusyChange={onWriteBusy} onCloned={value => { setCatalog(value); setMenu(false); setPath(''); }} />}</>;
  return <div className="app-shell">
    <header className="topbar"><div className="brand" aria-label={t("GitNova")}><span className="brand-mark"><img src={brandIcon} alt="" aria-hidden="true" /></span><span>{t("GitNova")}</span><span className="alpha">{t("alpha")}</span></div><div className="top-divider" />
      <div className="picker-root" ref={menuRoot}><button className="repo-picker" disabled={writeBusy || busy} ref={picker} aria-expanded={menu} aria-controls="workspace-picker" onClick={() => setMenu(!menu)}><span className="picker-icon"><Icon name="folder" size={19} /></span><span className="picker-copy"><span className="workspace-label">{workspace?.name ?? t("Local workspace")}</span><span className="picker-name">{selected?.name ?? t("Open a repository")}</span></span><Icon name="chevron" size={16} /></button>
      {menu && <div id="workspace-picker" className="picker-menu" role="region" aria-label={t("Workspaces and repositories")}><div className="picker-title"><Icon name="layers" /><span>{t("Your workspace")}</span><kbd>{t("esc")}</kbd></div>
        <label className="field-label" htmlFor="workspace">{t("Workspace")}</label><select id="workspace" value={workspace?.id ?? ''} disabled={busy || writeBusy} onChange={e => void mutate(api.switchWorkspace(e.target.value), false)}>{catalog?.workspaces.map(w => <option key={w.id} value={w.id}>{w.name}</option>)}</select>
        {catalog && workspace && <RepositoryList initialView={repositoryViews[workspace.id]} onViewChange={value=>setRepositoryViews(old=>retainRecent(old,workspace.id,value))} key={workspace.id} catalog={catalog} workspace={workspace} selectedStatus={currentStatus} blocked={busy || writeBusy} onSelect={id => void mutate(api.select(id))} onPin={id => void mutate(api.pin(id), false)}/>}
        {addForm}<form className="new-workspace" onSubmit={e => { e.preventDefault(); void mutate(api.createWorkspace(workspaceName), false); setWorkspaceName(''); }}><input aria-label={t("New workspace name")} placeholder={t("New workspace name")} value={workspaceName} maxLength={80} onChange={e => setWorkspaceName(e.target.value)} /><button disabled={busy || writeBusy || !workspaceName.trim()}>{t("Create")}</button></form>
        {selectedId && (related.subs.length > 0 || related.trees.length > 1 || related.error || related.loading) && <div className="related"><span className="field-label">{t("Related repositories")}</span>{related.loading && <p className="hint" role="status">{t("Discovering submodules and linked worktrees…")}</p>}{related.error && <p className="hint">{t("Inspection incomplete:")} {related.error}</p>}{related.subs.map(sub => <SubmoduleEntry key={sub.relationId} membershipId={selectedId} submodule={sub} disabled={busy || writeBusy} onOpen={path => void mutate(api.openRelated(selectedId, path))} onBusyChange={onWriteBusy} onFailure={reportOperationError} onDone={async receipt => {
          reportOperation(receipt); if (activeId.current !== selectedId) return;
          await refreshWorking(selectedId);
          const [subs, trees] = await Promise.all([api.submodules(selectedId), api.worktrees(selectedId)]);
          if (activeId.current === selectedId) setRelated({ subs, trees, error: '', loading: false });
        }} />)}{related.trees.length > 1 && related.trees.map((tree, index) => <WorktreeEntry canRemove={index > 0 && tree.path.display !== rootPath} key={tree.path.hex} tree={tree} membershipId={selectedId} disabled={busy || writeBusy} onOpen={path => void mutate(api.openRelated(selectedId, path))} onBusyChange={onWriteBusy} onDone={async receipt => {
          reportOperation(receipt); if (activeId.current !== selectedId) return;
          try {
            setCatalog(await api.catalog());
            await refreshWorking(selectedId);
            const [subs, trees] = await Promise.all([api.submodules(selectedId), api.worktrees(selectedId)]);
            if (activeId.current === selectedId) setRelated({ subs, trees, error: '', loading: false });
          } catch { setError('The operation result was recorded, but related repositories could not refresh. Refresh to inspect the current state.'); }
        }} />)}</div>}
        <p className="shortcut"><kbd>{t("⌘ / Ctrl O")}</kbd> {t("Switch repository")}</p>
      </div>}</div>
      {catalog && workspace && selectedId && <CommitSearch key={workspace.id} catalog={catalog} workspace={workspace} currentId={selectedId} disabled={writeBusy || busy} open={searchOpen} onOpenChange={value => { setSearchOpen(value); if (value) { setMenu(false); setGraphIntent(null); } }} onNavigate={openSearchResult} />}<div className="topbar-spacer" /><span className="local-indicator"><Icon name="lock" size={12} />{t("Local workspace")}</span>{selectedId && <button className="refresh" title={t("Refresh repository")} disabled={graphBusy || writeBusy} onClick={() => { const id=selectedId; window.dispatchEvent(new Event('gitnova-remotes-changed')); setGraphBusy(true); void refreshWorking(id, true).catch(e => setError(errorMessage(e))).finally(()=>{if(activeId.current===id)setGraphBusy(false);}); }}><Icon name="refresh" size={15} className={graphBusy ? 'spinning' : undefined} /><span>{t("Refresh")}</span></button>}
      <button ref={settingsButton} className="icon-button settings-trigger" aria-label={t('Settings')} title={t('Settings')} aria-haspopup="dialog" aria-expanded={settingsOpen} disabled={writeBusy || busy} onClick={()=>{setMenu(false);setSearchOpen(false);setSettingsOpen(true);}}><Icon name="settings" size={20}/></button>
    </header>
    {problem && <div className="error-banner" role="alert"><span>{t(problem)}</span><button className="icon-button" aria-label={t("Dismiss error")} onClick={() => { setError(''); if (selectedId) setOperationFeedback(old => Object.fromEntries(Object.entries(old).filter(([id]) => id !== selectedId))); }}><Icon name="close" /></button></div>}
    <main>{!selected ? <div className="welcome"><div className="welcome-visual" aria-hidden="true"><svg width="168" height="126" viewBox="0 0 168 126"><path d="M45 8v110M45 87c0-46 75-10 75-60" stroke="#4b8eff" fill="none" strokeWidth="2"/><path d="M45 58c0 25 39 11 39 39" stroke="#566989" fill="none" strokeWidth="2"/>{[[45,22],[45,58],[45,103],[120,25],[84,99]].map(([x,y],i) => <circle key={i} cx={x} cy={y} r={i===1?7:5} fill="#20222c" stroke={i===4?'#7288b0':'#6ca4ff'} strokeWidth="2" />)}</svg></div><span className="eyebrow">{t("A LITTLE CLARITY FOR YOUR CODE.")}</span><h1>{t("Every change.")}<br /><span>{t("A clearer picture.")}</span></h1><p>{t("Your history, branches and next commit.")}<br />{t("Bring your repositories together. Keep your focus here.")}</p>{!menu && addForm}<span className="welcome-footnote"><Icon name="lock" size={13} />{t("Open local repositories. Publish when you choose.")}</span></div> : <div className="repository-view">
      <div className="contextbar"><div className="context-main"><span className="context-title"><Icon name="branch" size={16} />{t("Repository history")}</span><span className="context-separator">/</span><span className="context-subtitle" title={rootPath}>{selected.name}</span></div><div className="repo-actions">{selectedId && <BranchSwitcher tracking={session?.page.tracking} key={`branch-${selectedId}`} membershipId={selectedId} blocked={writeBusy || busy} contextActive={Boolean(graphIntent) || searchOpen} status={currentStatus} onBusyChange={onWriteBusy} onSwitched={async receipt => {
        reportOperation(receipt);
        if (activeId.current !== selectedId) return;
        await refreshWorking(selectedId);
        if (activeId.current === selectedId && receipt.status === 'succeeded' && receipt.kind !== 'fetch_remote' && receipt.headAfter) {setSessions(old => old[selectedId] ? {...old,[selectedId]:{...old[selectedId],selected:receipt.headAfter}} : old);}
      }} />}
      {selectedId && <div className="sync-actions"><button className="sync-pull" disabled={writeBusy || busy || !currentStatus || !currentStatus.branch || !currentStatus.head || currentStatus.bare || Boolean(merges[selectedId] && merges[selectedId].phase !== 'none')} title={merges[selectedId] && merges[selectedId].phase !== 'none' ? t('Finish or recover the current operation before pulling.') : t('Fetch and review a pull into the current branch.')} onClick={event => { if (!currentStatus) return; const box = event.currentTarget.getBoundingClientRect(); setMenu(false); setSearchOpen(false); setGraphIntent({ oid: currentStatus.head ?? '', refName: currentStatus.branch ? `refs/heads/${currentStatus.branch}` : undefined, x: box.left, y: box.bottom + 6, quick: false, action: 'pull' }); }}>{t('Pull')} ↓</button></div>}
      {selectedId && <button className="push-current" disabled={writeBusy || busy || !currentStatus?.branch || !currentStatus.head || currentStatus.bare || Boolean(merges[selectedId] && merges[selectedId].phase !== 'none')} title={merges[selectedId] && merges[selectedId].phase !== 'none' ? t('Finish or recover the current operation before pushing.') : currentStatus?.branch ? t('Review push for {0}', [currentStatus.branch]) : t('Check out a local branch before pushing.')} onClick={event => { if (!currentStatus?.head || !currentStatus.branch) return; const box = event.currentTarget.getBoundingClientRect(); setMenu(false); setSearchOpen(false); setGraphIntent({ oid: currentStatus.head, refName: `refs/heads/${currentStatus.branch}`, x: box.left, y: box.bottom + 6, quick: false, action: 'push' }); }}>{t('Push')} ↑</button>}{selectedId&&<button className="terminal-toggle" disabled={busy||writeBusy||!desktop} aria-pressed={terminalVisible&&terminalEntries.some(e=>e.membershipId===selectedId)} title={t('Open a shell in this repository folder')} onClick={()=>{const exists=terminalEntries.some(entry=>entry.membershipId===selectedId);if(exists){setTerminalVisible(value=>!value);return;}if(terminalEntries.length>=4){setError('End an existing terminal session before opening another (maximum 4).');return;}setTerminalEntries(entries=>[...entries,{membershipId:selectedId,name:selected?.name??''}]);setTerminalVisible(true);}}><Icon name="terminal" size={16}/>{t('Terminal')}</button>}</div><div className="status-line">{syncError ? <span className="sync-error" title={syncError}>{t("Sync paused · refresh to retry")}</span> : currentStatus && <><span className={currentStatus.dirty || Boolean(merges[selectedId!] && merges[selectedId!].phase !== 'none') ? 'dirty-dot' : 'clean-dot'} />{merges[selectedId!]?.phase && merges[selectedId!].phase !== 'none' ? <button className="context-working-status operation-inspection" aria-label={t('Inspect current operation')} disabled={busy||writeBusy} title={t(merges[selectedId!].blockedReason??'Inspect current operation')} onClick={()=>selectCommit(WORKING)}>{t(merges[selectedId!].phase === 'pending' ? 'Operation pending · {0} changed paths' : 'Operation needs inspection · {0} changed paths', [currentStatus.changedEntries])}</button> : <span className="context-working-status">{currentStatus.bare ? t("Bare repository") : currentStatus.dirty ? t("{0} working tree {1}", [currentStatus.changedEntries, currentStatus.changedEntries === 1 ? t("change") : t("changes")]) : t("Working tree clean")}</span>}</>}{writeBusy && !autoFetchBusy && <span role="status" className="operation-wait"><Icon name="refresh" className="loading-spin" size={12}/>{t("Git operation in progress…")}</span>}{terminalEntries.length>0&&<span className="auto-fetch-status" title={t("Auto fetch pauses while terminal sessions are open.")}>{t("Auto fetch paused")}</span>}{autoFetch&&terminalEntries.length===0&&<span className="auto-fetch-status" title={autoFetch.error?`${t('Auto fetch paused; retrying later. Fetch manually if authorization is needed.')} ${t(autoFetch.error)}`:autoFetch.at?`${t('Auto fetched {0}',[autoFetch.remote??''])} · ${new Date(autoFetch.at).toLocaleTimeString()}`:t('Auto fetching…')}>{autoFetch.running?t('Auto fetching…'):autoFetch.error?t('Auto fetch paused'):autoFetch.at?t('Auto fetched'):''}</span>}{!writeBusy && feedback && ['succeeded', 'pending', 'conflicted'].includes(feedback.status) && (!working || feedback.kind?.startsWith('stash_') || feedback.kind === 'merge_release' || feedback.kind === 'initialize_submodule' || feedback.kind === 'restore_submodule' || feedback.kind === 'update_submodule_remote' || feedback.kind === 'remove_worktree') && <span role="status" className="operation-status" title={t(feedback.summary)}>{t(feedback.summary)}</span>}{!writeBusy && recoveryLocation && <button className="copy-recovery" title={recoveryLocation} onClick={async () => { try { await navigator.clipboard.writeText(recoveryLocation); } catch { setError('Could not copy the recovery location.'); } }}>{t("Copy recovery")} {(feedback?.planned?.type === 'discard' || feedback?.planned?.type === 'merge' && feedback.planned.preview.recoveryDir) ? t("location") : t("ref")}</button>}{graphBusy && !writeBusy && <span role="status" className="sync-state operation-wait"><Icon name="refresh" className="loading-spin" size={12}/>{t("Updating…")}</span>}{selectedOid && !working && !inspectorOpen && <div className="context-layout-actions"><button className="restore-details" aria-label={t("Show commit details")} onClick={() => setInspectorOpen(true)}><Icon name="panel" size={15} />{t("Details")}</button></div>}</div></div>
      <div className={`repository-layout ${working && inspectorOpen ? 'working-layout' : ''}`}><div className={`work-area ${selectedOid && inspectorOpen ? 'with-detail' : ''}`}>
        {session ? <History membershipId={selectedId!} submodules={!session.page.status.bare && <BranchSubmodules key={`${selectedId}-${session.page.status.branch}-${session.page.status.head}`} membershipId={selectedId!} status={session.page.status} revision={String(session.revision ?? 0)} disabled={busy || writeBusy} onOpen={path => void mutate(api.openRelated(selectedId!, path))} onBusyChange={onWriteBusy} onDone={repositoryWritten} onFailure={reportOperationError}/>} stashBusy={writeBusy || busy} onStash={() => { setStashLaunch({ id: selectedId!, kind: 'push', nonce: Date.now() }); selectCommit(WORKING); setDiffCollapsed(false); }} onPop={() => { setStashLaunch({ id: selectedId!, kind: 'pop', nonce: Date.now() }); selectCommit(WORKING); setDiffCollapsed(false); }} stashes={stashes[selectedId!]} stashError={stashErrors[selectedId!]} mergeState={selectedId ? merges[selectedId] : undefined} key={`history-${selectedId}`} session={session} busy={graphBusy} onAction={intent => { setMenu(false); setGraphIntent(intent); }} onSelect={selectCommit} onScroll={(top, anchor, anchorOffset) => setSessions(old => old[selectedId!] ? { ...old, [selectedId!]: { ...old[selectedId!], scroll: top, anchor, anchorOffset } } : old)} onMore={() => void load(selectedId!, true)} /> : <div className="quiet-state"><p role="status">{graphBusy ? t("Reading your repository…") : t("History could not be loaded. Refresh to try again.")}</p></div>}
        {working && selectedId && inspectorOpen ? (!selectedStash && merges[selectedId] && merges[selectedId].phase !== 'none' ? <MergeInspector onRefresh={()=>refreshWorking(selectedId)} discardedResult={feedback?.kind === 'merge_discard' && feedback.status === 'pending'} key={`merge-${selectedId}-${merges[selectedId].sessionId}`} membershipId={selectedId} state={merges[selectedId]} changes={changes[selectedId] ?? null} draft={mergeDrafts[selectedId]?.text ?? ''} onDraft={text => setMergeDrafts(old => ({ ...old, [selectedId]: { session: merges[selectedId].sessionId, text } }))} externalBusy={writeBusy} onBusyChange={onWriteBusy} onDone={repositoryWritten} onClose={() => selectCommit(null)} onDiffChange={setWorkingDiff} onInspect={() => setDiffCollapsed(false)} /> : <WorkingDetails operationState={merges[selectedId]} onOpenRelated={path => void mutate(api.openRelated(selectedId, path))} stashConflict={operationFeedback[selectedId]?.status === 'conflicted' && Boolean(operationFeedback[selectedId]?.kind?.startsWith('stash_'))} stashLaunch={stashLaunch?.id === selectedId ? stashLaunch : null} initialStash={selectedStash} onStashBack={() => { setStashLaunch(null); selectCommit(WORKING); }} key={`working-${selectedId}-${selectedOid?.startsWith('stash:') ? selectedOid : 'changes'}`} onBusyChange={onWriteBusy} membershipId={selectedId} externalBusy={writeBusy} onOperation={reportOperation} onOperationError={reportOperationError} onDiffChange={setWorkingDiff} onInspect={() => setDiffCollapsed(false)} changes={changes[selectedId] ?? null} draft={drafts[selectedId] ?? ''} onDraft={value => setDrafts(old => ({ ...old, [selectedId]: value }))} onRefresh={() => refreshWorking(selectedId)} onClose={() => selectCommit(null)} />) : selectedOid && !working && inspectorOpen && <CommitInspector membershipId={selectedId!} detail={detail} refs={session?.page.refs.filter(ref => ref.oid === detail?.commit.oid) ?? []} busy={detailBusy} onClose={() => setInspectorOpen(false)} onShowChanges={() => { setDiffCollapsed(false); requestAnimationFrame(() => document.querySelector<HTMLButtonElement>('.dock-files button[aria-pressed="true"]')?.focus()); }} />}
      </div>
      {working && inspectorOpen && <CommitDiffDock key={`${selectedId}:working`} source={workingDiff && workingDiff.membershipId === selectedId ? workingDiff.source : { diff: '', truncated: false, label: t("Working file diff"), title: t("Working changes"), comparison: '', before: '', after: '', selectedPath: null }} busy={!workingDiff || workingDiff.membershipId !== selectedId || workingDiff.loading} size={diffHeight} onResize={setDiffHeight} collapsed={diffCollapsed} onToggle={() => setDiffCollapsed(value => !value)} />}
      {selectedOid && !working && <CommitDiffDock key={`${selectedId}:${selectedOid}`} membershipId={selectedId} preferredPath={investigationFile?.membershipId===selectedId && investigationFile?.oid===selectedOid?investigationFile.path:undefined} detail={detail} busy={detailBusy} size={diffHeight} onResize={setDiffHeight} collapsed={diffCollapsed} onToggle={() => setDiffCollapsed(value => !value)} />}
      </div>
    </div>}</main>
    {graphIntent && selectedId && currentStatus && <GraphActions key={`actions-${selectedId}`} intent={graphIntent} onConfigureRemote={()=>{settingsReturn.current=document.activeElement as HTMLElement;setSettingsOpen(true);}} pendingOperation={Boolean(merges[selectedId] && merges[selectedId].phase !== 'none')} tracking={session?.page.tracking} membershipId={selectedId} refs={session?.page.refs ?? []} status={currentStatus} blocked={writeBusy || busy} onClose={() => setGraphIntent(null)} onInspect={selectCommit} onBusyChange={onWriteBusy} onDone={repositoryWritten} onFailure={reportOperationError} />}
    {terminalEntries.length>0&&<Suspense fallback={<div role="status">{t("Opening terminal…")}</div>}><RepositoryTerminal entries={terminalEntries} selectedId={selectedId} visible={terminalVisible} onHide={()=>setTerminalVisible(false)} onEnd={id=>setTerminalEntries(entries=>entries.filter(entry=>entry.membershipId!==id))} onChanged={()=>{if(selectedId)void refreshWorking(selectedId).catch(e=>setSyncError(errorMessage(e)));}}/></Suspense>}
    <Settings open={settingsOpen} onClose={closeSettings} />
    <footer className="app-footer"><span><span className="footer-dot" />{t("GitNova · by edocstudio")}</span><RemoteFooter membershipId={selectedId} branch={currentStatus?.branch} tracking={session?.page.tracking} revision={session?.page.fingerprint}/><span>{session ? session.rows.length ? t("{0}–{1} commits loaded", [session.base + 1, session.base + session.rows.length]) : t("0 commits loaded") : t("Git made simple.")}<span className="footer-separator">·</span><kbd>{t("⌘ / Ctrl O")}</kbd> {t("Switch repository")}</span></footer>
  </div>;
}
