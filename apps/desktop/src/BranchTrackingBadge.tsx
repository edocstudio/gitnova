import { t } from './i18n';
import type { BranchTracking } from './contracts';
const validCount = (count: number | null) => count !== null && Number.isSafeInteger(count) && count >= 0;
function matchingTracking(value?: BranchTracking, oid?: string | null) {
  const matched = value && value.oid === oid ? value : undefined;
  return matched?.state === 'tracked' && (!validCount(matched.ahead) || !validCount(matched.behind) || !matched.upstream) ? undefined : matched;
}
function comparisonFor(current?: BranchTracking) {
  if (current?.state === 'tracked') return {ahead: current.ahead!, behind: current.behind!, reference: current.upstream!, fallback: false};
  const fallback = current?.state === 'noUpstream' ? current.comparison : undefined;
  return fallback && validCount(fallback.ahead) && validCount(fallback.behind) ? {...fallback, fallback: true} : undefined;
}
function comparisonTitle(current?: BranchTracking) {
  const comparison = comparisonFor(current);
  if (comparison?.fallback) return t('{0} commits ahead · {1} behind {2}. Display comparison only; no upstream is configured. Pull and Push still use their reviewed destinations.', [comparison.ahead, comparison.behind, comparison.reference]);
  return !current ? t('Tracking comparison is unavailable. Refresh or inspect with Git.')
    : current.state === 'noUpstream' ? t('No upstream configured. Choose a destination when pushing.')
    : current.state === 'gone' ? t('Configured upstream {0} is missing. Fetch or review upstream settings.', [current.upstream])
    : current.state === 'shallow' ? t('Limited history: ahead/behind counts are unavailable.')
    : t('{0} commits ahead · {1} behind {2}. Compared with locally fetched refs; Fetch to check for newer remote commits.', [current.ahead, current.behind, current.upstream]);
}
export function BranchTrackingBadge({ value, oid }: { value?: BranchTracking; oid?: string | null }) {
  const current = matchingTracking(value, oid);
  const comparison = comparisonFor(current);
  const title = comparison && comparison.ahead === 0 && comparison.behind === 0 ? `${t('Same commit as {0}', [comparison.reference])}. ${comparisonTitle(current)}` : comparisonTitle(current);
  return <span className={`branch-tracking ${current?.state ?? 'unknown'}`} role="img" aria-label={title} title={title}>
    {comparison ? <>{comparison.ahead > 0 && <span className="ahead">{comparison.ahead}↑</span>}{comparison.behind > 0 && <span className="behind">{comparison.behind}↓</span>}{comparison.ahead === 0 && comparison.behind === 0 && <span className="in-sync">=</span>}</> : current?.state === 'noUpstream' ? '—' : '?' }
  </span>;
}

// Counts describe fetched refs, not write authorization or upload object counts.
export function BranchSyncCount({value, oid, direction}: {value?: BranchTracking; oid?: string | null; direction: 'pull' | 'push'}) {
  const current = matchingTracking(value, oid);
  const comparison = comparisonFor(current);
  const known = Boolean(comparison);
  const title = comparisonTitle(current);
  return <span className={`sync-count ${known ? direction : 'unknown'}`} role="img" aria-label={title} title={title}>
    {comparison ? direction === 'push' ? comparison.ahead : comparison.behind : current?.state === 'noUpstream' ? '—' : '?'}
  </span>;
}
