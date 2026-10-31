import {CommitIdentityNotice} from './CommitIdentity';
import { t, useLocale } from './i18n';
import { useEffect, useMemo, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import type { AmendPreview, MergeState, NativePath, Submodule, StashEntry, ChangedFile, CommitPreview, FilePreview, OperationReceipt, WorkingChanges } from './contracts';
import type { DockPatch } from './CommitDiffDock';
import { findPatch, parseDiff } from './diff';
import { Icon } from './Icons';
import { RecordedActivityView } from './RecordedActivity';
import { DiscardControls } from './DiscardControls';
import type { SubmoduleIntent } from './SubmoduleEntry';
import {SubmoduleLifecycle} from './SubmoduleLifecycle';
import { SubmoduleEntry } from './SubmoduleEntry';
import { StageAllControls } from './StageAllControls';
import { StashControls } from './StashControls';
import {VirtualFileRows} from './VirtualFileRows';
type Entry = { file: ChangedFile; staged: boolean; key: string };
export type WorkingDiff = { membershipId: string; source: DockPatch; loading: boolean } | null;
export function WorkingDetails({ operationState, stashConflict, stashLaunch, initialStash, onStashBack, membershipId, changes, draft, onDraft, onRefresh, onClose, onBusyChange, onDiffChange, onInspect, onOperation, onOperationError, externalBusy, onOpenRelated }: {
  operationState?: MergeState;
  onOpenRelated: (path: NativePath) => void; stashConflict?: boolean; stashLaunch?: { kind: 'push' | 'pop'; nonce: number } | null; initialStash?: Pick<StashEntry, 'oid' | 'index'>; onStashBack?: () => void; membershipId: string; changes: WorkingChanges | null; draft: string; onDraft: (value: string) => void;
  onRefresh: () => Promise<void>; onClose: () => void; onBusyChange: (value: boolean) => void; onDiffChange: (value: WorkingDiff) => void; onInspect: () => void; onOperation: (value: OperationReceipt) => void; onOperationError: (id: string, message: string) => void; externalBusy: boolean;
}) {
  const locale = useLocale();
  const [stageAllOpen,setStageAllOpen]=useState(false);
  const [discard, setDiscard] = useState<{path: NativePath | null; scope: 'unstaged' | 'all'} | null>(null);
  const [fileMenu, setFileMenu] = useState<{item: Entry; x: number; y: number} | null>(null);
  const [registration,setRegistration]=useState<{path?:string;action:'stage'|'unstage'}|null>(null);
  const [pointerIntent, setPointerIntent] = useState<SubmoduleIntent | null>(null);
  const isSubmodule = (file: ChangedFile) => file.blockedReason === 'Submodule pointer writes need a separate reviewed workflow';
  const [submodule, setSubmodule] = useState<Submodule | null>(null);
  useEffect(() => { if (!fileMenu) return; requestAnimationFrame(() => document.querySelector<HTMLButtonElement>('.working-file-menu button:not(:disabled)')?.focus()); const close = (event?: PointerEvent) => { if (!event || event.button === 0) setFileMenu(null); }; const key = (e: KeyboardEvent) => { if (e.key === 'Escape') { close(); document.querySelector<HTMLButtonElement>('.change-file-row.active .changed-file')?.focus(); } }; window.addEventListener('pointerdown', close); window.addEventListener('keydown', key); return () => { window.removeEventListener('pointerdown', close); window.removeEventListener('keydown', key); }; }, [fileMenu]);
  const [stashOpen, setStashOpen] = useState(Boolean(initialStash || stashLaunch));
  useEffect(() => { if (stashLaunch) setStashOpen(true); }, [stashLaunch?.nonce]);
  const [selection, setSelection] = useState<string | null>(null);
  const [hunks,setHunks]=useState<import('./contracts').HunkPreview|null>(null);
  const [preview, setPreview] = useState<FilePreview | null>(null);
  const [review, setReview] = useState<CommitPreview | null>(null);
  const [amend,setAmend]=useState(false);const [amendHead,setAmendHead]=useState<string|null>(null);const [amendDraft,setAmendDraft]=useState('');const [amendReview,setAmendReview]=useState<AmendPreview|null>(null);const [acknowledgeRewrite,setAcknowledgeRewrite]=useState(false);
  const [reviewInvalidated,setReviewInvalidated]=useState(false);
  const [reviewedState, setReviewedState] = useState<string | null>(null);
  const [error, setError] = useState(''); const [busy, setBusy] = useState(false); const [loading, setLoading] = useState(false);
  const [receipt, setReceipt] = useState<OperationReceipt | null>(null);
  const writing = useRef(false); const request = useRef(0); const latest = useRef(changes); latest.current = changes;
  const entries = useMemo(() => changes?.files.flatMap(file => [
    ...(file.unstaged ? [{ file, staged: false, key: `${file.path.hex}:working` }] : []),
    ...(file.staged ? [{ file, staged: true, key: `${file.path.hex}:staged` }] : []),
  ]) ?? [], [changes]);
  const entry = entries.find(e => e.key === selection);
  const unmatchedReview = useMemo(() => { if (!review || review.truncated) return false; const patches = parseDiff(review.diff); return review.files.some(file => findPatch(patches, file.path) < 0); }, [review]);
  const stagedCount = entries.filter(item => item.staged).length;
  const reviewPerFile=Boolean(review?.truncated&&review.completeManifest);
  const canCommitReview=Boolean(review&&(!review.truncated||reviewPerFile));
  const visiblePreview=preview?.path.hex===entry?.file.path.hex&&preview?.staged===entry?.staged?preview:null;
  const message=amend?amendDraft:draft;
  const newline = message.indexOf('\n'); const summary = newline < 0 ? message : message.slice(0, newline);
  const description = newline < 0 ? '' : message.slice(newline + 1).replace(/^\n/, '');
  const updateDraft = (title: string, body: string) => {const value=body ? `${title}\n\n${body}` : title;if(amend)setAmendDraft(value);else onDraft(value);};
  const amendBlocked=Boolean(amendReview?.blockedReason||amendReview?.referencedBy.length&&!acknowledgeRewrite);
  const toggleAmend=async(value:boolean)=>{
    if(busy||externalBusy||review)return;setError('');setAmendReview(null);setAcknowledgeRewrite(false);setAmend(value);
    if(!value)return;const head=changes?.status.head;if(!head)return;setAmendHead(head);setBusy(true);
    try{const info=await api.amendOriginal(membershipId,head);if(latest.current?.status.head!==head)throw new Error('HEAD changed. Select Amend again for the current commit.');setAmendDraft(info.message);}catch(e){setError(errorMessage(e));setAmend(false);}finally{setBusy(false);}
  };
  useEffect(() => { if (!entries.some(e => e.key === selection)) setSelection(entries[0]?.key ?? null); }, [entries, selection]);
  useEffect(() => {
    if (busy || stashOpen) return;
    const version = ++request.current; setPreview(null); setHunks(null); setLoading(false); setSubmodule(null);
    if (!entry || review && !reviewPerFile) return;
    setLoading(true); setError('');
    if (entry.file.blockedReason?.includes('Submodule')) void api.submodules(membershipId).then(items => { if (version === request.current) setSubmodule(items.find(sub => sub.path.hex === entry.file.path.hex) ?? null); }).catch(e => { if (version === request.current) setError(errorMessage(e)); });
    (reviewPerFile&&review?api.commitFilePreview(membershipId,entry.file.path,review.token):!entry.file.blockedReason?api.previewHunks(membershipId,entry.file.path,entry.staged).then(p=>{if(version===request.current)setHunks(p);return p.file;}).catch(()=>api.filePreview(membershipId,entry.file.path,entry.staged)):api.filePreview(membershipId, entry.file.path, entry.staged)).then(value => { if (version === request.current) setPreview(value); }).catch(e => { if (version === request.current) setError(errorMessage(e)); }).finally(() => { if (version === request.current) setLoading(false); });
    return () => { request.current++; };
  }, [membershipId, selection, changes?.token, review, reviewPerFile, busy, stashOpen]);
  useEffect(() => {
    if (review && reviewedState !== changes?.token) { setReview(null); setAmendReview(null); setReviewInvalidated(true); }
  }, [changes?.token, review, reviewedState]);
  useEffect(() => {
    if (stashOpen) return;
    onDiffChange({ membershipId, loading, source: {
      hunks: !review&&hunks&&visiblePreview?.token===hunks.file.token?{headers:hunks.headers,staged:hunks.file.staged,disabled:busy||externalBusy||Boolean(discard)||stageAllOpen,onApply:(index:number)=>void perform(()=>api.changeHunk(membershipId,hunks.file.path,hunks.file.staged,index,hunks.token))}:undefined,
      diff: reviewPerFile?visiblePreview?.diff??'':review?.diff??visiblePreview?.diff??'', truncated: reviewPerFile?visiblePreview?.truncated??false:review?.truncated??visiblePreview?.truncated??false,
      label: amendReview ? t("Amend delta preview") : review ? t("Staged commit preview") : t("Working file diff"), title: review ? t("Commit preview") : entry && isSubmodule(entry.file) ? t("Submodule pointer") : entry?.staged ? t("Staged file") : t("Unstaged file"),
      comparison: amendReview ? t("Original HEAD → Staged replacement") : review || entry?.staged ? t("HEAD → Staged") : t("Index → Working tree"), before: review || entry?.staged ? changes?.status.head?.slice(0, 8) ?? t("Empty tree") : t("Index"),
      after: review || entry?.staged ? t("Staged") : t("Working tree"), selectedPath: review && review.files.length===0 ? null : entry?.file.path ?? null,
    } });
  }, [locale, membershipId, preview, hunks, busy, externalBusy, discard, stageAllOpen, review, reviewPerFile, amendReview, loading, entry, changes?.status.head, onDiffChange, stashOpen]);
  useEffect(() => () => onDiffChange(null), [onDiffChange]);
  const perform = async (operation: () => Promise<OperationReceipt>, after?: () => void) => {
    if (writing.current) return; writing.current = true; onBusyChange(true); setBusy(true); setError(''); setReviewInvalidated(false);
    try {
      const result = await operation(); onOperation(result); setReceipt(result);
      if(result.status==='succeeded'&&result.kind==='amend_commit'){setAmend(false);setAmendHead(null);setAmendDraft('');setAmendReview(null);setReview(null);}
      else if (result.status === 'succeeded' && result.kind === 'commit') { onDraft(''); setReview(null); }
      else if (result.status !== 'succeeded') {setReview(null);setAmendReview(null);}
      try { await onRefresh(); } catch { setError('Operation result recorded. Refresh Working changes to inspect the current state.'); }
      if (result.status === 'succeeded') after?.();
    } catch (e) { onOperationError(membershipId, errorMessage(e)); setReview(null);setAmendReview(null); }
    finally { writing.current = false; onBusyChange(false); setBusy(false); }
  };
  const onlyChildFiles = (item: Entry) => !item.staged && isSubmodule(item.file) && submodule?.path.hex === item.file.path.hex && submodule.indexOid === submodule.checkedOutOid && Boolean(submodule.dirty);
  const changeIndex = (item: Entry) => {
    if (busy || externalBusy || review) return;
    if(isSubmodule(item.file)&&['A','D'].includes(item.file.indexStatus)){setSelection(item.key);setRegistration({path:item.file.path.display,action:item.staged?'unstage':'stage'});onInspect();return;}
    if (onlyChildFiles(item)) { onOpenRelated(item.file.path); return; }
    if (isSubmodule(item.file)) { setSelection(item.key);setPointerIntent({action:item.staged?'unstage':'stage',nonce:Date.now()});onInspect();return; }
    if (item.file.blockedReason) return;
    void perform(async () => {
      const filePreview = preview?.path.hex === item.file.path.hex && preview.staged === item.staged ? preview : await api.filePreview(membershipId, item.file.path, item.staged);
      return api.changeIndex(membershipId, item.file.path, !item.staged, filePreview.token);
    }, () => setSelection(`${item.file.path.hex}:${item.staged ? 'working' : 'staged'}`));
  };
  const startReview = async () => {
    if (busy || externalBusy || (!amend && stagedCount === 0) || !summary.trim()) return;
    const state = changes?.token; setError(''); setReviewInvalidated(false); setBusy(true);
    try {
      setAmendReview(null);setAcknowledgeRewrite(false);
      const amendment=amend&&amendHead?await api.previewAmend(membershipId,{oid:amendHead,message}):null;
      const result=amendment?.commit??await api.commitPreview(membershipId);
      if (latest.current?.token !== state) throw new Error('Working changes changed. Review the current staged content again.');
      setReviewedState(state ?? null); setSelection(entries.find(item => item.staged)?.key ?? null); setAmendReview(amendment); setReview(result); onInspect();
    } catch (e) { setError(errorMessage(e)); }
    finally { setBusy(false); }
  };
  const group = (staged: boolean) => {
    const items = entries.filter(item => item.staged === staged);
    return <section className={`change-group ${staged ? 'staged-group' : 'unstaged-group'}`} aria-label={staged ? t("Staged files") : t("Unstaged files")}>
      <div className="change-group-heading"><strong>{staged ? t("Staged") : t("Unstaged")} <span>{items.length}</span></strong><small>{staged ? t("In next commit") : (review||discard||stageAllOpen) ? t("Not in next commit") : null}</small>{!staged&&!review&&!discard&&!stageAllOpen&&<button className="stage-all-action" disabled={busy||externalBusy||!items.length} onClick={()=>setStageAllOpen(true)}>{t('Stage all changes…')}</button>}</div>
      <VirtualFileRows items={items} itemKey={item=>item.key} selection={selection} empty={<p className="group-empty">{staged ? t("Stage a file to include it here.") : t("No unstaged files.")}</p>}>{item => <div className={`change-file-row ${item.key === selection ? 'active' : ''}`} key={item.key}>
        <button data-path-hex={item.file.path.hex} className={`changed-file ${item.key === selection ? 'active' : ''}`} aria-pressed={item.key === selection} disabled={busy || externalBusy || Boolean(review && !staged)} onContextMenu={event => { event.preventDefault(); if (busy || externalBusy || review || discard) return; setSelection(item.key); setFileMenu({item,x:event.clientX,y:event.clientY}); }} onKeyDown={event => { if (event.key === 'F10' && event.shiftKey && !busy && !externalBusy && !review && !discard) { event.preventDefault(); const box=event.currentTarget.getBoundingClientRect(); setSelection(item.key); setFileMenu({item,x:box.left,y:box.bottom}); } }} onDoubleClick={() => { if(isSubmodule(item.file)&&!busy&&!externalBusy)onOpenRelated(item.file.path); }} onClick={() => { setSelection(item.key); setPointerIntent(null); setRegistration(null); onInspect(); }} title={item.file.path.display}>
          <span className={`file-status status-${staged ? item.file.indexStatus : item.file.worktreeStatus}`}>{staged ? item.file.indexStatus : item.file.worktreeStatus === '?' ? 'A' : item.file.worktreeStatus}</span><span className="changed-path">{isSubmodule(item.file)&&<Icon name="layers" size={13}/>} {item.file.path.display}</span>{isSubmodule(item.file)&&<span className="submodule-type">{t("Submodule")}</span>}
        </button>
        {!review && <button className="file-stage-action" aria-label={isSubmodule(item.file) ? onlyChildFiles(item) ? t("Open submodule") : t(staged ? "Unstage submodule pointer" : "Stage submodule pointer") : item.key === selection ? staged ? t("Unstage file") : t("Stage file") : `${t(staged ? 'Unstage' : 'Stage')} ${item.file.path.display}`} title={isSubmodule(item.file) ? onlyChildFiles(item) ? t("Open submodule") : t(staged ? "Unstage submodule pointer" : "Stage submodule pointer") : item.file.blockedReason ?? `${t(staged ? 'Unstage' : 'Stage')} ${item.file.path.display}`} disabled={busy || externalBusy || stageAllOpen || Boolean(discard) || Boolean(item.file.blockedReason && !isSubmodule(item.file))} onClick={() => changeIndex(item)}><Icon name={isSubmodule(item.file) ? "layers" : staged ? 'back' : 'plus'} size={14} /></button>}
      </div>}</VirtualFileRows>
    </section>;
  };
  const next = amend ? 'Edit the message or stage additional changes, then review the replacement.' : stagedCount === 0 ? 'Stage files to include them in your next commit.' : !summary.trim() ? 'Add a summary, then review the staged changes.' : 'Review the staged diff before creating your commit.';
  if (stashOpen) return <aside className="detail working-detail stash-detail" aria-label={t('Stashes')}><div className="detail-heading"><div><Icon name="layers" /><span>{t('Stashes')}</span></div><button className="text-button" disabled={busy || externalBusy} onClick={() => { setStashOpen(false); onStashBack?.(); }}>{t('Back to changes')}</button></div><StashControls operationState={operationState} key={stashLaunch?.nonce ?? 'graph'} initialKind={initialStash ? 'pop' : stashLaunch?.kind} initialEntry={initialStash} membershipId={membershipId} changes={changes} disabled={externalBusy} onBusyChange={onBusyChange} onOperation={value => { setReceipt(value); onOperation(value); }} onFailure={onOperationError} onRefresh={onRefresh} onDiffChange={onDiffChange} onClose={() => { setStashOpen(false); onStashBack?.(); }} /></aside>;
  return <aside className="detail working-detail" aria-label={t("Working changes")}>
    <div className="detail-heading"><div><Icon name="layers" /><span>{t("Working changes")}</span></div><div className="working-heading-actions"><button className="icon-button danger-action" title={t('Discard changes…')} aria-label={t('Discard changes…')} disabled={busy || externalBusy || Boolean(review) || Boolean(discard) || !entries.length} onClick={() => { setDiscard({path:null,scope:'all'}); setFileMenu(null); }}><Icon name="trash" /></button><button className="icon-button" aria-label={t("Close working changes")} disabled={busy} onClick={onClose}><Icon name="close" /></button></div></div>
    <div className="working-context"><strong>{changes?.files.length ?? 0} {changes?.files.some(isSubmodule) ? changes.files.length === 1 ? t("changed item") : t("changed items") : changes?.files.length === 1 ? t("changed file") : t("changed files")}</strong><span>{t("on")} <b>{changes?.status.branch ?? t("detached HEAD")}</b></span></div>
    {(error || reviewInvalidated) && <div className="inline-error" role="alert">{t(error || 'Working changes changed. Review staged content again before committing.')}</div>}
    {receipt?.status === 'succeeded' && <div className={`operation-receipt ${receipt.status === 'succeeded' ? 'success' : 'failure'}`} role="status"><Icon name={receipt.status === 'succeeded' ? 'check' : 'commit'} /><div>{t(receipt.summary)}{receipt.recovery && !receipt.kind.endsWith('_submodule_pointer') && <p>{t(receipt.recovery)}</p>}</div></div>}
    {(stashConflict || receipt?.kind?.startsWith('stash_') && receipt.status === 'conflicted') && <p className="notice" role="status">{t('Saved work is retained for recovery. Resolve stash conflicts with Git before retrying.')}</p>}
    {!review && !discard && <RecordedActivityView onBusyChange={onBusyChange} onRefresh={onRefresh} membershipId={membershipId} refreshKey={receipt ? `${receipt.id}:${receipt.status}` : ''} disabled={busy || externalBusy} />}
    <div className="working-files">{changes ? <>{group(false)}{group(true)}</> : <div className="quiet-state" role="status">{t("Reading working changes…")}</div>}</div>
    {fileMenu && <div className="working-file-menu" role="menu" aria-label={t('File actions')} style={{left:Math.max(8,Math.min(fileMenu.x,window.innerWidth-230)),top:Math.max(8,Math.min(fileMenu.y,window.innerHeight-130))}} onPointerDown={event => event.stopPropagation()}><button role="menuitem" disabled={onlyChildFiles(fileMenu.item) || Boolean(fileMenu.item.file.blockedReason && !isSubmodule(fileMenu.item.file))} onClick={() => { changeIndex(fileMenu.item); setFileMenu(null); }}>{isSubmodule(fileMenu.item.file) ? fileMenu.item.staged ? t('Unstage pointer…') : t('Stage pointer…') : fileMenu.item.staged ? t('Unstage file') : t('Stage file')}</button>{isSubmodule(fileMenu.item.file) ? <><button role="menuitem" onClick={() => {onOpenRelated(fileMenu.item.file.path);setFileMenu(null);}}>{t('Open submodule')}</button><button role="menuitem" onClick={() => {setPointerIntent({action:'restore',nonce:Date.now()});setFileMenu(null);}}>{t('Restore recorded…')}</button></> : <button role="menuitem" onClick={() => { setDiscard({path:fileMenu.item.file.path,scope:fileMenu.item.staged?'all':'unstaged'});setFileMenu(null); }}>{t('Discard file…')}</button>}</div>}
    {!review&&!discard&&entry?.file.path.display==='.gitmodules'&&!registration&&<div className="working-submodule"><button disabled={busy||externalBusy} onClick={()=>setRegistration({action:entry.staged?'unstage':'stage'})}>{t(entry.staged?'Unstage registration…':'Stage registration…')}</button><p className="hint">{t('Registration changes should stage or unstage the manifest and selected gitlink together.')}</p></div>}
    {registration&&!review&&!discard&&<SubmoduleLifecycle key={`${registration.action}:${registration.path??''}`} membershipId={membershipId} removePath={registration.path} indexAction={registration.action} disabled={busy||externalBusy} onBusyChange={onBusyChange} onClose={()=>setRegistration(null)} onDone={async result=>{onOperation(result);setReceipt(result);setRegistration(null);await onRefresh();}}/>}
    {submodule && !registration && !discard && <div className="working-submodule"><SubmoduleEntry detailed pointerIntent={pointerIntent} membershipId={membershipId} submodule={submodule} disabled={busy || externalBusy || Boolean(review)} onOpen={onOpenRelated} onBusyChange={onBusyChange} onFailure={onOperationError} onDone={async result => { setPointerIntent(null); onOperation(result); setReceipt(result); await onRefresh(); }} /></div>}
    {entry?.file.blockedReason && !submodule && <div className="notice working-blocked">{t(entry.file.blockedReason)}</div>}
    {stageAllOpen?<StageAllControls membershipId={membershipId} onClose={()=>setStageAllOpen(false)} onBusyChange={onBusyChange} onDone={async result=>{onOperation(result);setReceipt(result);await onRefresh();}}/>:discard && changes ? <DiscardControls key={`${discard.path?.hex ?? 'all'}:${discard.scope}`} membershipId={membershipId} changes={changes} initialPath={discard.path} initialScope={discard.scope} disabled={busy || externalBusy} onBusyChange={onBusyChange} onInspect={(path, staged) => { setSelection(`${path.hex}:${staged ? 'staged' : 'working'}`); onInspect(); }} onClose={() => { setDiscard(null); requestAnimationFrame(() => document.querySelector<HTMLButtonElement>('.change-file-row.active .changed-file')?.focus()); }} onDone={async result => { onOperation(result); setReceipt(result); try { await onRefresh(); } catch { setError('Discard result recorded. Refresh Working changes to inspect the current state.'); } }} /> : <form className={amend ? "commit-compose bounded-compose amending" : "commit-compose bounded-compose"} aria-label={review ? t("Review commit") : t("Prepare commit")} onSubmit={event => { event.preventDefault(); if (review) { if (!busy && !externalBusy && canCommitReview && !unmatchedReview && !amendBlocked) void perform(() => amendReview ? api.amendCommit(membershipId,amendReview.request,amendReview.token,acknowledgeRewrite) : api.commit(membershipId, draft, review.token)); } else void startReview(); }}>
      <div className="commit-compose-fields"><CommitIdentityNotice membershipId={membershipId} refreshKey={amendReview?.token??review?.token??changes?.token}/><ol className="commit-sequence" aria-label={t("Commit progress")}><li className={stagedCount === 0 ? 'current' : 'complete'}>{t("Changes")}</li><li className={stagedCount > 0 && !review ? 'current' : review ? 'complete' : ''}>{t("Staged")} <span>{stagedCount}</span></li><li className={review ? 'current' : ''}>{t("Commit")}</li></ol>
      <label className="amend-toggle"><input type="checkbox" checked={amend} disabled={busy||externalBusy||Boolean(review)||!amend&&(!changes?.status.head||!changes.status.branch)} onChange={event=>void toggleAmend(event.target.checked)}/>{t('Amend last commit')}</label>
      {amend&&<p className="amend-context">{t('Replace last commit')} <code>{amendHead?.slice(0,8)}</code>{amendHead!==changes?.status.head&&<span className="notice">{t('HEAD changed. Select Amend again for the current commit.')}</span>}</p>}
      {amendReview&&<div className="amend-review"><p>{t('Original author and all parents are preserved.')}</p><p>{amendReview.original.author} · {amendReview.original.parents.length ? amendReview.original.parents.map(p=>p.slice(0,8)).join(', ') : t('Root commit')}</p><p className="hint">{t('The diff shows changes added to the original commit. Unstaged work is excluded.')}</p>{!amendReview.commit.files.length&&<p>{t('Message-only amend; the file tree stays unchanged.')}</p>}{amendReview.emptyReplacement&&<p className="notice">{amendReview.original.parents.length?t('No file changes from the first parent; the commit remains in history.'):t('This root commit contains no files; it remains in history.')}</p>}{amendReview.originalSigned&&<p className="notice">{t('The original signature cannot carry over. Configured signing runs for the replacement.')}</p>}{amendReview.referencedBy.length>0&&<><p className="notice">{t('The original commit is referenced by tags or fetched remote branches. Amending rewrites history.')}</p><p className="hint">{t('For already-pushed commits, normal Push may be refused. GitNova will not force-push.')}</p><div className="amend-references">{amendReview.referencedBy.map(ref=><code key={ref}>{ref}</code>)}</div><label className="amend-toggle"><input type="checkbox" checked={acknowledgeRewrite} disabled={busy||externalBusy} onChange={event=>setAcknowledgeRewrite(event.target.checked)}/>{t('I understand this rewrites referenced history.')}</label></>}<p className="hint">{t('Remote checks use cached refs. No fetch, push or force push is performed.')}</p><code className="recovery-ref">{amendReview.recoveryRef}</code>{amendReview.blockedReason&&<p className="notice">{t(amendReview.blockedReason)}</p>}</div>}
      <label htmlFor="commit-summary">{t("Summary")}</label><input id="commit-summary" placeholder={t("What changed?")} value={summary} onChange={event => updateDraft(event.target.value, description)} maxLength={4096} disabled={busy || externalBusy || Boolean(review)} />
      <label htmlFor="commit-description">{t("Description")} <span>{t("optional")}</span></label><textarea id="commit-description" placeholder={t("Why was this change needed?")} value={description} onChange={event => updateDraft(summary, event.target.value)} rows={2} maxLength={60000} disabled={busy || externalBusy || Boolean(review)} />
      <p className="commit-next" role="status">{review ? unmatchedReview ? t("Some staged paths could not be matched to their patches. Refresh and review again; commit is disabled.") : review.truncated ? reviewPerFile?t("The complete staged file list is selected. Code loads one file at a time; all listed staged content will be committed."):t("The staged preview is incomplete. Inspect the full diff with Git before committing.") : amendReview ? t("Ready to amend the reviewed commit. Hooks and configured signing will run.") : t("Ready to commit {0} {1} to {2}. Only staged content is included; configured hooks and signing will run.", [review.files.length, review.files.length === 1 ? t("file") : t("files"), review.status.branch ?? 'detached HEAD']) : t(next)}</p>
      </div>
      {review ? <><div className="compose-footer"><button type="button" className="text-button" disabled={busy} onClick={() => {setReview(null);setAmendReview(null);setAcknowledgeRewrite(false);}}><Icon name="back" size={14} />{t("Back to changes")}</button><button className="primary commit-confirm" disabled={busy || externalBusy || !canCommitReview || unmatchedReview || amendBlocked}><Icon name="commit" size={14} />{busy ? t("Committing…") : amendReview ? t("Amend reviewed commit") : t("Commit staged changes")}</button></div></> : <div className="compose-footer"><span>{stagedCount} {stagedCount === 1 ? t("file") : t("files")} {t("staged")}</span><button className="primary" disabled={busy || externalBusy || !amend && stagedCount === 0 || !summary.trim() || amend && amendHead !== changes?.status.head}>{busy ? t("Working…") : amend ? t("Review amend") : t("Review commit")}<Icon name="arrow" size={14} /></button></div>}
    </form>}
  </aside>;
}
