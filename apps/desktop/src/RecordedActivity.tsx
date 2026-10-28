import { useCallback, useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import type { RecordedActivity } from './contracts';
import { dateLocale, t } from './i18n';
const kinds: Record<string, string> = {stage_hunk:'Stage hunk',unstage_hunk:'Unstage hunk',worktree_maintenance:'Worktree metadata',sync_submodule:'Sync URLs',rebase_begin:'Rebase',rebase_resolve:'Resolve conflict',rebase_finish:'Continue rebase',rebase_skip:'Skip empty commit',rebase_abort:'Abort rebase',rebase_release:'Close session', create_hosted_repository:'Create hosted repository', add_remote:'Add remote', stage_submodule_pointer: 'Stage submodule pointer', unstage_submodule_pointer: 'Unstage submodule pointer', stage: 'Stage', unstage: 'Unstage', commit: 'Commit', discard_changes: 'Discard', stash_push: 'Stash', stash_pop: 'Pop', stash_apply: 'Apply stash', stash_drop: 'Drop stash', merge_discard: 'Discard', merge_unstage: 'Unstage', merge_begin: 'Start merge', merge_resolve: 'Resolve conflict', merge_finish: 'Finish merge', merge_abort: 'Abort merge', merge_release: 'Close session', switch_branch: 'Switch branch', switch_remote_branch: 'Switch remote branch', fetch_remote: 'Fetch', push_branch: 'Push', initialize_submodule: 'Initialize submodule', restore_submodule: 'Restore recorded version', create_worktree: 'Create worktree', remove_worktree: 'Remove worktree', ref_operation: 'Branch or tag operation', graph_action: 'Graph action' };
const statuses: Record<string, string> = { succeeded: 'Succeeded', failed: 'Failed', unknown: 'Unconfirmed', started: 'Completion unconfirmed', pending: 'Pending', conflicted: 'Conflicts' };
export function RecordedActivityView({ membershipId, refreshKey, disabled }: { membershipId: string; refreshKey: string; disabled: boolean }) {
  const [open, setOpen] = useState(false);
  const [data, setData] = useState<RecordedActivity | null>(null); const [loading, setLoading] = useState(false); const [error, setError] = useState(''); const sequence = useRef(0);
  const load = useCallback(async () => {
    const request=++sequence.current; setLoading(true); setData(null); setError('');
    try { const value=await api.recordedActivity(membershipId); if (request===sequence.current) setData(value); }
    catch (e) { if (request===sequence.current) { setData(null); setError(errorMessage(e)); } }
    finally { if (request===sequence.current) setLoading(false); }
  }, [membershipId]);
  useEffect(() => { if (open) void load(); return () => { sequence.current++; }; }, [load, refreshKey, open]);
  const copy = async (text: string) => { try { await navigator.clipboard.writeText(text); } catch { setError('Could not copy the recovery location.'); } };
  const time = (value: number) => { const date=new Date(value*1000); return Number.isFinite(date.getTime()) ? date.toLocaleString(dateLocale()) : t('Unknown time'); };
  return <details className="recorded-activity" open={open} onToggle={event => { if(event.target===event.currentTarget) setOpen(event.currentTarget.open); }}><summary>{t('Recent activity & recovery')}</summary>{open && <div role="region" aria-label={t('Recorded GitNova operations')}>
    <p>{t('Recorded activity includes linked checkouts. Current Git state is shown separately above.')}</p>
    <button className="text-button" disabled={disabled || loading} onClick={() => void load()}>{loading ? t('Reading recorded activity…') : t('Refresh recorded activity')}</button>
    {error && <div role="alert" className="inline-error">{t(error)}</div>}
    {data?.incomplete && <p className="notice">{t('Recent activity is incomplete; some records were not inspected.')}</p>}
    {data && !data.entries.length && <p>{data.incomplete ? t('No matching records in the inspected activity.') : t('No recorded GitNova activity for this repository yet.')}</p>}
    {data?.entries.map(entry => <article className="recorded-operation" key={entry.id}>
      <div><strong>{t(kinds[entry.kind] ?? 'Git operation')}</strong><span className={entry.status==='succeeded' ? 'recorded-success' : 'recorded-status'}>{t(statuses[entry.status] ?? 'Recorded')}</span></div>
      <small>{time(entry.finishedAt ?? entry.startedAt)} · {entry.currentCheckout ? t('Current checkout') : entry.source?.display ?? t('Other or retired checkout')}</small>
      <p>{t(entry.summary)}</p>
      {(entry.status==='started'||entry.status==='unknown') && <p className="notice">{t('Completion was not confirmed. Inspect Git and the recorded recovery before retrying.')}</p>}
      <details><summary>{t('Recorded details')}</summary>{entry.headAfter && <p>{t('Recorded HEAD')}: <code>{entry.headAfter}</code></p>}{entry.recovery && <p>{t(entry.recovery)}</p>}
      {entry.recoveryDir && <div className="recorded-copy"><code>{entry.recoveryDir.display}</code><button disabled={disabled} onClick={() => void copy(entry.recoveryDir!.display)}>{t('Copy recorded location')}</button></div>}
      {entry.recoveryRef && <div className="recorded-copy"><code>{entry.recoveryRef}</code><button disabled={disabled} onClick={() => void copy(entry.recoveryRef!)}>{t('Copy recorded ref')}</button></div>}
      <p>{t('Recorded pointers do not guarantee that recovery data is complete.')}</p></details>
    </article>)}
    {data && <button className="text-button" disabled={disabled} title={data.directory.display} onClick={() => void copy(data.directory.display)}>{t('Copy activity folder')}</button>}
  </div>}</details>;
}
