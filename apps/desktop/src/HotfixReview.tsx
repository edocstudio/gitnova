import { useRef, useState } from 'react';
import { configuredJenkinsJob } from './JenkinsStatus';
import { api, errorMessage } from './api';
import { t } from './i18n';
import type { GitRef, OperationReceipt } from './contracts';
import { WorktreeCreateReview } from './WorktreeCreateReview';

export interface HotfixRequest {
  tag: string; base: string; remote: string; productionTag: string | null;
}
export interface HotfixPlan {
  branch: string; release: string; saved: string; fix: string; tree: string;
  checkpoint: string | null; changes: string;
}
interface Props {
  membershipId: string; tag: string; base: string; refs: GitRef[]; blocked: boolean;
  onBusyChange: (value: boolean) => void; onDone: (receipt: OperationReceipt) => Promise<void>;
  onTag: (oid: string) => void; onInspect: (oid: string) => void; onPublish: (ref: GitRef) => void; onBack: () => void; onClose: () => void; onFailure: (id: string, message: string) => void;
}

export function HotfixReview(props: Props) {
  const {membershipId, tag, base, refs, blocked} = props;
  const remotes = refs.filter(ref => ref.name.startsWith('refs/remotes/') && ref.name.endsWith('/release'))
    .map(ref => ref.name.slice(13, -8));
  const [remote, setRemote] = useState(remotes.includes('origin') ? 'origin' : remotes[0] ?? '');
  const jenkinsJob = configuredJenkinsJob(membershipId, remote);
  const [mode, setMode] = useState('auto');
  const [deployed, setDeployed] = useState('');
  const [confirmed, setConfirmed] = useState(false);
  const [plan, setPlan] = useState<HotfixPlan | null>(null);
  const [candidate, setCandidate] = useState('');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const active = useRef(false);
  const branch = 'hotfix/' + tag;
  const checkpoint = 'uat-hotfix/' + remote + '/' + tag;
  const resume = 'resume-release/' + remote + '/' + tag;
  const hasBranch = (name: string) => refs.some(ref => ref.name === 'refs/heads/' + name);
  const anchor = 'refs/gitnova/hotfix/' + remote + '/' + tag;
  const restoring = refs.some(ref => ref.name === anchor + '/uat');
  const restored = refs.some(ref => ref.name === anchor + '/resume');
  const release = refs.find(ref => ref.name === 'refs/remotes/' + remote + '/release');
  const productionTags = refs.filter(ref => ref.name.startsWith('refs/tags/') && ref.name !== 'refs/tags/' + tag && ref.oid === release?.oid);
  const act = async (verifyOnly = false) => {
    if (active.current || blocked) return;
    active.current = true;
    setBusy(true); props.onBusyChange(true); setMessage('');
    const request = {tag, base, remote, productionTag: restoring ? deployed : null};
    try {
      if (verifyOnly) {
        setCandidate('');
        setCandidate(await api.hotfixCandidate(membershipId, request));
      } else if (!plan) setPlan(await api.previewHotfix(membershipId, request));
      else {
        const receipt = await api.prepareHotfix(membershipId, request, plan);
        setPlan(null);
        await props.onDone(receipt);
        if (receipt.status !== 'succeeded') setMessage(receipt.summary);
      }
    } catch (error) { setPlan(null); setMessage(errorMessage(error)); }
    finally { active.current = false; setBusy(false); props.onBusyChange(false); }
  };
  if (!hasBranch(branch) && !restoring) return <>
    <strong>{t('Start hotfix from {0}', [tag])}</strong>
    <p>{t('Edit and commit in the new worktree, then reopen Production hotfix on this tag.')}</p>
    <WorktreeCreateReview {...props} oid={base} locals={[]} fixedBranch={branch} />
  </>;
  const publishRef = refs.find(ref => ref.name === 'refs/heads/' + (restored ? resume : checkpoint));
  return <div className="worktree-create-review">
    <strong>{t('Production hotfix')} · {tag}</strong>
    <label>{t('Release remote')}<select disabled={busy || !!plan} value={remote} onChange={e => {
      setRemote(e.target.value); setCandidate(''); setDeployed(''); setConfirmed(false); setMessage('');
    }}>{remotes.map(name => <option key={name}>{name}</option>)}</select></label>
    {!release && <p className="notice">{t('Fetch the remote release branch to continue.')}</p>}
    {publishRef && <button disabled={busy || blocked} onClick={() => props.onPublish(publishRef)}>{t('Review publish actions')}</button>}
    {restored ? <p>{t('Push {0} and open a request into release to resume the saved work with the fix.', [resume])}</p> : <>
      {restoring ? <>
        <p>{t('Push {0} and merge its reviewed request into release. Verify the resulting release commit in UAT.', [checkpoint])}</p>

        <label>{t('UAT deployment')}<select value={mode} onChange={e => setMode(e.target.value)}>
          <option value="auto">{t('Automatic deploy')}</option><option value="manual">{t('Manual deploy job')}</option><option value="jenkins">{t('Manual Jenkins build')}</option>
        </select></label>
        <p className="hint">{mode === 'auto' ? t('Check the deployment result and SHA on release.') : mode === 'manual' ? t('Open the release pipeline, run its deploy job and verify the deployed SHA.') : t('Open the Jenkins job, build release and verify the SHA in the build result.')}</p>
        <p>{t('After UAT passes, create and push a new tag on the tested release commit. After production succeeds, fetch and select that tag below.')}</p>
        {mode === 'jenkins' && jenkinsJob && <button onClick={() => void api.openCiUrl(jenkinsJob).catch(e => setMessage(errorMessage(e)))}>{t('Open configured Jenkins job')}</button>}
        <button disabled={blocked || busy || !!plan} onClick={() => void act(true)}>{t('Check release against hotfix')}</button>
        {candidate && candidate === release?.oid && <div>
          <code>{candidate.slice(0,12)}</code>
          <button disabled={blocked || busy} onClick={() => props.onInspect(candidate)}>{t('Inspect release commit and CI')}</button>
          <button disabled={blocked || busy} onClick={() => props.onTag(candidate)}>{t('UAT passed · review new tag')}</button>
        </div>}
        <label>{t('Deployed production tag')}<select disabled={busy || !!plan} value={deployed} onChange={e => {setDeployed(e.target.value); setConfirmed(false);}}>
          <option value="">{t('Select a tag on the fetched release tip')}</option>
          {productionTags.map(ref => <option key={ref.name}>{ref.name.slice(10)}</option>)}
        </select></label>
        <label><input type="checkbox" disabled={!deployed || busy || !!plan} checked={confirmed} onChange={e => setConfirmed(e.target.checked)} /> {t('I verified UAT and production for this commit.')}</label>
      </> : <p>{t('Fetch release first. Preview the complete hotfix tree and keep the current release commit for recovery.')}</p>}
      {plan && <><p>{t('New local review branch')} <code>{plan.branch}</code></p>
        <p>{t('Saved release')} <code>{plan.saved.slice(0,12)}</code> · {t('Hotfix')} <code>{plan.fix.slice(0,12)}</code></p>
        <pre style={{maxHeight:'12em', overflow:'auto'}}>{plan.changes || t('No file changes')}</pre>
        <p className="hint">{t('Before merging, verify the request still targets release at {0}.', [plan.release.slice(0,12)])}</p></>}
      {message && <p role="alert" className="inline-error">{t(message)}</p>}
      <div className="graph-action-footer"><button disabled={busy} onClick={() => plan ? setPlan(null) : props.onBack()}>{t('Back')}</button>
        <button className="primary" disabled={blocked || busy || !remote || (restoring && !confirmed)} onClick={() => void act()}>
          {plan ? t('Create review branch') : restoring ? t('Preview restored release') : t('Preview UAT snapshot')}</button></div>
    </>}
  </div>;
}
