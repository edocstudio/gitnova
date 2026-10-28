import { t } from './i18n';
import type { BranchTracking } from './contracts';
export function BranchTrackingBadge({ value, oid }: { value?: BranchTracking; oid?: string | null }) {
  const matched = value && value.oid === oid ? value : undefined;
  const current = matched?.state === 'tracked' && (matched.ahead === null || matched.behind === null || !matched.upstream) ? undefined : matched;
  const title = !current ? t('Tracking comparison is unavailable. Refresh or inspect with Git.')
    : current.state === 'noUpstream' ? t('No upstream configured. Choose a destination when pushing.')
    : current.state === 'gone' ? t('Configured upstream {0} is missing. Fetch or review upstream settings.', [current.upstream])
    : current.state === 'shallow' ? t('Limited history: ahead/behind counts are unavailable.')
    : t('{0} commits ahead · {1} behind {2}. Compared with locally fetched refs; Fetch to check for newer remote commits.', [current.ahead, current.behind, current.upstream]);
  return <span className={`branch-tracking ${current?.state ?? 'unknown'}`} role="img" aria-label={title} title={title}>
    {current?.state === 'tracked' ? <>{!!current.ahead && <span className="ahead">{current.ahead}↑</span>}{!!current.behind && <span className="behind">{current.behind}↓</span>}{!current.ahead && !current.behind && <span>✓</span>}</> : current?.state === 'noUpstream' ? '—' : '?' }
  </span>;
}
