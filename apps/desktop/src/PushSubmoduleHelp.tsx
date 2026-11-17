import type {NativePath,PushSubmoduleReport} from './contracts';
import {t} from './i18n';

export function PushSubmoduleHelp({report,blocked,onOpen,onRecheck}:{report:PushSubmoduleReport;blocked:boolean;onOpen?:(path:NativePath,oid?:string)=>void;onRecheck:()=>void}){
  if(!report.issues.length&&report.complete)return null;
  return <div className="notice push-submodule-help" role="region" aria-label={t('Submodule publication checks')}>
    <strong>{t('Check the exact submodule commits before pushing')}</strong>
    <p>{t('Push checks historical submodule versions too. Publishing the latest child commit may not publish an older commit from a different history.')}</p>
    {!report.complete&&<p>{t('Submodule inspection is incomplete. Native Git still performs its final availability check.')}</p>}
    {report.issues.map(issue=><article key={`${issue.path.hex}:${issue.oid}`}>
      <strong>{issue.path.display}</strong><p>{t('Required child commit: {0}',[issue.oid])}</p><p>{t('Referenced by parent commit: {0}',[issue.parentOid])}</p>
      {issue.currentOid&&issue.currentOid!==issue.oid&&<p>{t('Current child checkout: {0}',[issue.currentOid])}</p>}
      <p>{t(issue.evidence==='unknown'&&issue.objectAvailable?'Remote reachability could not be fully inspected. Fetch in the child checkout and recheck; publication is not confirmed.':issue.objectAvailable?'Not reachable from this checkout’s cached remote branches. If already published, fetch in the submodule and recheck. Otherwise publish this exact commit on a reviewed branch.':'The required commit or checkout could not be inspected. Open the submodule and fetch or initialize it before rechecking.')}</p>
      {issue.canOpen&&onOpen&&<button disabled={blocked} onClick={()=>onOpen(issue.path,issue.objectAvailable&&issue.evidence!=='unknown'?issue.oid:undefined)}>{t(issue.objectAvailable&&issue.evidence!=='unknown'?'Review branch at required commit {0}':'Open submodule to fetch',[issue.oid.slice(0,8)])}</button>}
    </article>)}
    <p className="hint">{t('Opening a commit does not check it out or create a branch. Review branch creation at that commit, publish it, then return to the parent repository and recheck Push.')}</p>
    <button disabled={blocked} onClick={onRecheck}>{t('Recheck submodule availability')}</button>
  </div>;
}
