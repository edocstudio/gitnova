import {CommitIdentityNotice} from './CommitIdentity';
import { t } from './i18n';
import { useEffect, useState } from 'react';
import { api, errorMessage } from './api';
import type { RebaseEdit, ChangeOperation, MergeMethod, MergeStartPreview, OperationReceipt } from './contracts';
function sameReplay(a:RebaseEdit[]|undefined,b:RebaseEdit[]|null){return (a??[]).length===(b??[]).length&&(a??[]).every((step,i)=>step.oid===b?.[i].oid&&step.action===b?.[i].action);}
export function MergeStartReview({ expectedDestination, membershipId, oid, sourceRef, operation = 'merge', blocked, onBack, onBusyChange, onDone, onClose, onFailure, onReviewWork }: {
  expectedDestination?: {branch: string; oid: string};
  membershipId: string; oid: string; sourceRef: string | null; operation?: ChangeOperation; blocked: boolean; onBack: () => void; onBusyChange: (value: boolean) => void; onDone: (receipt: OperationReceipt) => Promise<void>; onClose: () => void; onFailure: (id: string, message: string) => void; onReviewWork: () => void;
}) {
  const [preview, setPreview] = useState<MergeStartPreview | null>(null); const [error, setError] = useState(''); const [busy, setBusy] = useState(false);
  const [parents, setParents] = useState<string[]>([]); const [mainline, setMainline] = useState<number | null>(null);
  const [cleanupSelection, setCleanupSelection] = useState<string | null>(null);
  const cleanupKey = `${sourceRef}:${oid}`;
  const deleteSourceBranch = operation === 'merge' && cleanupSelection === cleanupKey;
  const localSource = sourceRef?.startsWith('refs/heads/') ? sourceRef.slice(11) : null;
  const [acknowledgeRewrite,setAcknowledgeRewrite]=useState(false);
  const [editSubjects,setEditSubjects]=useState<Record<string,string>>({});
  const [steps,setSteps]=useState<RebaseEdit[]|null>(null);const [predicting,setPredicting]=useState(false);
  const [method, setMethod] = useState<MergeMethod>('mergeCommit');
  useEffect(()=>{setAcknowledgeRewrite(false);setMainline(null);setSteps(null);},[membershipId,oid,sourceRef,operation]);
  const label = operation === 'rebase' ? t("Rebase") : operation === 'cherryPick' ? t("Cherry-pick") : operation === 'revert' ? t("Revert") : t("Merge");
  const lower = label.toLowerCase();
  const isPatchOperation = operation === 'cherryPick' || operation === 'revert';
  const selectedMainline = isPatchOperation ? mainline : null;
  useEffect(() => {
    let cancelled = false; if(!steps)setPreview(null);setPredicting(true); setError('');
    api.previewMerge(membershipId, { rebaseSteps:operation==='rebase'?steps??[]:[], acknowledgeRewrite, deleteSourceBranch, method: operation === 'merge' ? method : 'auto', operation, mainline: selectedMainline, sourceOid: oid, sourceRef, sessionId: '' }).then(value => {
      if (expectedDestination && (value.branch !== expectedDestination.branch || value.originalOid !== expectedDestination.oid)) throw new Error('Branch or upstream changed. Close this review and try again.');
      if (operation==='rebase'&&!sameReplay(value.request.rebaseSteps,steps)) throw new Error('Restart GitNova to load the native backend for this operation. Nothing has been applied.');
      if ((value.request.operation ?? 'merge') !== operation) throw new Error('Restart GitNova to load the native backend for this operation. Nothing has been applied.');
      if (operation === 'merge' && value.request.method !== method) throw new Error('Restart GitNova to load the native backend for this operation. Nothing has been applied.');
      if (Boolean(value.request.deleteSourceBranch) !== deleteSourceBranch) throw new Error('Restart GitNova to load the native backend for this operation. Nothing has been applied.');
      if (!cancelled) { setPreview(value); setParents(value.parents ?? []); }
    }).catch(e => { if (!cancelled){setPreview(null);setError(errorMessage(e));} }).finally(()=>{if(!cancelled)setPredicting(false);}); return () => { cancelled = true; };
  }, [membershipId, oid, sourceRef, operation, mainline, method, deleteSourceBranch, acknowledgeRewrite, steps, expectedDestination?.branch, expectedDestination?.oid]);
  const staleSelection = Boolean(preview && (Boolean(preview.request.deleteSourceBranch) !== deleteSourceBranch || preview.request.sourceOid !== oid || (preview.request.operation ?? 'merge') !== operation || (operation === 'rebase' && (Boolean(preview.request.acknowledgeRewrite) !== acknowledgeRewrite || !sameReplay(preview.request.rebaseSteps,steps))) || (operation === 'merge' ? preview.request.method !== method : (preview.request.mainline ?? null) !== selectedMainline)));
  const needsMainline = isPatchOperation && parents.length > 1 && mainline === null;
  const apply = async () => {
    if (!preview || predicting || blocked || busy || staleSelection || needsMainline || preview.blockedReason || operation==='rebase'&&!acknowledgeRewrite || operation === 'merge' && preview.request.method !== method) return;
    setBusy(true); onBusyChange(true); setError('');
    try { const receipt = await api.beginMerge(membershipId, preview.request, preview.token); await onDone(receipt); if (['succeeded', 'pending', 'conflicted'].includes(receipt.status)) onClose(); else { setError(`${t(receipt.summary)} ${receipt.recovery ?? ''}`); setPreview(null); } }
    catch (e) { setError(errorMessage(e)); onFailure(membershipId, errorMessage(e)); setPreview(null); }
    finally { setBusy(false); onBusyChange(false); }
  };
  const reason = preview?.blockedReason ?? error;
  const resume = reason.includes('Finish or recover the current GitNova operation') || reason.includes('Finish or recover the existing merge');
  const reviewWork = resume || reason.startsWith('Commit or preserve uncommitted work');
  return <div className="merge-start-review"><CommitIdentityNotice membershipId={membershipId} refreshKey={preview?.token}/><strong>{label} {operation === 'merge' ? t("into") : operation === 'cherryPick' ? t("onto") : t("on")} {preview?.branch ?? t("current branch")}</strong>
    {operation==='rebase'&&preview&&<><p><b>{preview.branch}</b> → <code>{sourceRef?.replace(/^refs\/(heads|remotes|tags)\//,'')??oid.slice(0,12)}</code></p><p>{t('Replay {0} commits onto the selected target. Original tips are retained; no force push is performed.',[preview.rebase?.commits.length??0])}</p>{preview.rebase?.preserveMerges&&<p className="notice">{t('Preserves reviewed merge topology by recreating merges. Original manual merge resolutions may need to be resolved again. No history is flattened.')}</p>}{preview.rebase?.root&&<p className="notice">{t('No common ancestor: replay the entire branch, including its root, onto this target.')}</p>}{preview.rebase&&<details><summary>{t('Commits to replay')}</summary>{preview.rebase.commits.map(commit=><code className="maintenance-path" key={commit}>{commit}</code>)}</details>}<p className="hint">{t('Publication evidence uses fetched refs only. Unlisted commits may also be shared.')}</p>{!!preview.rebase?.referencedBy.length&&<p className="notice">{t('These cached remote refs contain commits being rewritten: {0}',[preview.rebase.referencedBy.join(', ')])}</p>}<label className="checkbox-label"><input type="checkbox" checked={acknowledgeRewrite} onChange={e=>setAcknowledgeRewrite(e.target.checked)} disabled={busy||blocked}/>{t('I understand rebase changes commit IDs; shared history needs coordination.')}</label></>}
    {operation==='rebase'&&(steps||preview?.rebase&&!preview.rebase.preserveMerges)&&<div className="replay-edit">
    {!steps?<button disabled={busy||blocked||predicting} onClick={()=>{setAcknowledgeRewrite(false);setEditSubjects(preview!.rebase!.subjects??{});setSteps(preview!.rebase!.commits.map(oid=>({oid,action:'pick'})));}}>{t('Customize linear replay…')}</button>:<>
    <strong>{t('Replay order · oldest first')}</strong><p className="hint">{t('Pick keeps a commit; Drop omits its changes; Fixup folds its changes into the earlier kept commit and keeps that commit’s message and author.')}</p>
    {steps.map((step,index)=><div className="replay-step" key={step.oid}><code>{step.oid.slice(0,8)}</code><span title={editSubjects[step.oid]}>{editSubjects[step.oid]}</span><select aria-label={t('Replay action for {0}',[step.oid.slice(0,8)])} disabled={busy||blocked} value={step.action} onChange={e=>{const action=e.currentTarget.value as RebaseEdit['action'];setAcknowledgeRewrite(false);setSteps(rows=>rows!.map((r,i)=>i===index?{...r,action}:r));}}><option value="pick">{t('Pick')}</option><option value="drop">{t('Drop')}</option><option value="fixup">{t('Fixup')}</option></select>{[-1,1].map(delta=><button key={delta} aria-label={t(delta<0?'Move {0} earlier':'Move {0} later',[step.oid.slice(0,8)])} disabled={busy||blocked||index+delta<0||index+delta>=steps.length} onClick={()=>{setAcknowledgeRewrite(false);setSteps(rows=>{const next=[...rows!];[next[index],next[index+delta]]=[next[index+delta],next[index]];return next;});}}>{delta<0?'↑':'↓'}</button>)}</div>)}
    <button disabled={busy||blocked} onClick={()=>{setSteps(null);setAcknowledgeRewrite(false);}}>{t('Restore original replay plan')}</button>
    </>}{predicting&&<p role="status">{t('Updating replay prediction…')}</p>}
    </div>}
    {operation === 'merge' && <><label htmlFor="merge-method">{t('Merge method')}</label><select id="merge-method" value={method} disabled={busy || blocked} onChange={event => { setPreview(null); setMethod(event.target.value as MergeMethod); }}><option value="mergeCommit">{t('Create merge commit')}</option><option value="fastForwardOnly">{t('Fast-forward only')}</option></select>{preview?.mode !== 'up_to_date' && <p className="hint">{method === 'mergeCommit' ? t('Create a commit with both branch tips as parents. The graph will retain the merge junction.') : t('Move the current branch to the source tip without creating a commit. The graph stays linear; diverged branches are refused.')}</p>}</>}
    {operation === 'merge' && localSource && localSource !== preview?.branch && <><label className="checkbox-label"><input type="checkbox" checked={deleteSourceBranch} disabled={busy || blocked} onChange={event => { setPreview(null); setCleanupSelection(event.target.checked ? cleanupKey : null); }}/>{t('Delete local source branch after merge: {0}', [localSource])}</label><p className="hint">{t('Delete only after a successful merge. The source tip is kept for recovery. Remote branches stay unchanged.')}</p></>}
    {isPatchOperation && parents.length > 1 && <><label htmlFor="operation-mainline">{t("Mainline parent · change baseline")}</label><select id="operation-mainline" value={mainline ?? ''} disabled={busy || blocked} onChange={e => { setPreview(null); setMainline(e.target.value ? Number(e.target.value) : null); }}><option value="">{t("Choose a parent…")}</option>{parents.map((parent, i) => <option key={`${i}:${parent}`} value={i + 1}>{t("Parent")} {i + 1} · {parent.slice(0, 12)}</option>)}</select><p className="hint">{t('The selected parent is the baseline. Review its changes to this merge commit before starting.')}</p>{mainline !== null && <p><code>{parents[mainline - 1]?.slice(0, 12)} → {oid.slice(0, 12)}</code></p>}{operation === 'revert' && <p className="notice">{t("Reverting a merge also affects what later merges will reintroduce. The selected parent defines which changes are reversed.")}</p>}</>}
    {!preview && !error && <p role="status">{t("Predicting")} {lower} {t("in an isolated object store…")}</p>}{preview && <>
    {operation !== 'rebase' && <p><code>{operation === 'merge' ? sourceRef?.replace(/^refs\/(heads|remotes|tags)\//, '') ?? oid.slice(0, 8) : `${oid.slice(0, 8)}${sourceRef ? ' · ' + sourceRef.replace(/^refs\/(heads|remotes|tags)\//, '') : ''}`}</code> → <b>{preview.branch}</b></p>}
    {!preview.blockedReason && <p>{operation === 'rebase' ? t(preview.rebase?.preserveMerges?'Merge topology is predicted in an isolated object store. Conflicts pause in Working changes.':'A linear replay is predicted without changing this checkout. Conflicts pause in Working changes.') : operation === 'cherryPick' ? t("Apply only this commit’s changes. Review the staged result before creating a new single-parent commit; original author and author date are retained.") : operation === 'revert' ? t("Apply the inverse changes as a new single-parent commit. Existing commits remain in history.") : preview.mode === 'fast_forward' ? t("Fast-forward: advance this branch to the selected commit.") : preview.mode === 'up_to_date' ? t("This commit is already included.") : t("Merge commit: inspect the result before creating the commit.")}</p>}
    {!preview.blockedReason && <div className="branch-preview-files">{preview.files.map(file => <div key={file.path.hex}><span>{file.status}</span><code>{file.path.display}</code></div>)}</div>}
    {!!preview.preservedSubmodules?.length && <div className="merge-submodule-evidence">
      <strong>{t('Keep existing submodule checkouts')}</strong>
      {preview.preservedSubmodules.map(item => <div key={item.path.hex}><code>{item.path.display}</code><dl>
        <div><dt>{t('Recorded in current branch')}</dt><dd><code title={item.recordedOid}>{item.recordedOid.slice(0, 12)}</code></dd></div>
        <div><dt>{t('Keep checked-out commit')}</dt><dd><code title={item.checkoutOid}>{item.checkoutOid.slice(0, 12)}</code></dd></div>
      </dl></div>)}
      <p className="hint">{t('These submodules are clean. This operation leaves their checkouts and recorded pointers unchanged.')}</p>
    </div>}
    {!!preview.submodules?.length && <div className="merge-submodule-evidence">
      <strong>{t('Submodules in this merge')}</strong>
      {preview.submodules.map(item=><div key={item.path.hex}><code>{item.path.display}</code><dl>
        <div><dt>{t('Current branch')}</dt><dd><code title={item.currentOid??''}>{item.currentOid?.slice(0,12)??t('Not recorded')}</code></dd></div>
        <div><dt>{t('Incoming branch')}</dt><dd><code title={item.incomingOid??''}>{item.incomingOid?.slice(0,12)??t('Not recorded')}</code></dd></div>
        <div><dt>{t('Predicted merge')}</dt><dd>{item.conflicted?t('Submodule conflict'):item.resultOid===item.currentOid?<span>{t('Keep current pointer')} · <code title={item.resultOid??''}>{item.resultOid?.slice(0,12)??t('Not recorded')}</code></span>:<code title={item.resultOid??''}>{item.resultOid?.slice(0,12)??t('Not recorded')}</code>}</dd></div>
      </dl></div>)}
      <p className="hint">{t('Merge records submodule commits in the parent. It does not update child checkouts to the latest remote version.')}</p>
    </div>}
    {!!preview.conflicts.length && !preview.blockedReason && <p className="notice">{preview.conflicts.length} {t("predicted conflicts. Resolve them in Working changes after starting.")}</p>}
    {preview.blockedReason ? <p className="notice" role="status">{t(preview.blockedReason)}</p> : <p className="hint">{t("Both tips are retained for recovery. Configured Git hooks and checkout filters may run when applying.")}</p>}
  </>}{error && <p className="inline-error" role="alert">{t(error)}</p>}<div className="graph-action-footer"><button disabled={busy} onClick={reviewWork ? onReviewWork : onBack}>{resume ? t('Review current operation') : reviewWork ? t("Review working changes") : t("Back")}</button><button className="primary" disabled={predicting || busy || blocked || staleSelection || needsMainline || !preview || Boolean(preview.blockedReason)||operation==='rebase'&&!acknowledgeRewrite} onClick={() => void apply()}>{busy ? t("Applying…") : preview?.mode === 'up_to_date' ? t('Already included') : operation === 'merge' && method === 'fastForwardOnly' || preview?.mode === 'fast_forward' ? t("Fast-forward branch") : t("Start {0}", [lower])}</button></div></div>;
}
