import { expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { BranchTrackingBadge } from './BranchTrackingBadge';
import { setLocale } from './i18n';
import type { BranchTracking } from './contracts';
const tracked: BranchTracking = { branch:'main', oid:'commit', upstream:'refs/remotes/origin/main', remote:'origin', remoteBranch:'main', pushRemote:'origin', pushBranch:'main', ahead:3, behind:2, state:'tracked' };
it('shows independent ahead and behind commit counts with fetched-ref scope', () => {
  const html=renderToStaticMarkup(<BranchTrackingBadge value={tracked} oid="commit"/>);
  expect(html).toContain('3↑');expect(html).toContain('2↓');expect(html).toContain('locally fetched refs');
});
it('does not turn absent, gone, shallow or mismatched metadata into zero counts', () => {
  for (const state of ['noUpstream','gone','shallow'] as const) {
    const html=renderToStaticMarkup(<BranchTrackingBadge value={{...tracked,state,ahead:null,behind:null}} oid="commit"/>);
    expect(html).not.toContain('↑');expect(html).not.toContain('✓');
  }
  expect(renderToStaticMarkup(<BranchTrackingBadge value={tracked} oid="other"/>)).toContain('unavailable');
  expect(renderToStaticMarkup(<BranchTrackingBadge value={{...tracked,ahead:null}} oid="commit"/>)).not.toContain('✓');
});
it('represents known synchronization and translates the comparison without changing ref data', () => {
  expect(renderToStaticMarkup(<BranchTrackingBadge value={{...tracked,ahead:0,behind:0}} oid="commit"/>)).toContain('✓');
  setLocale('th');try { const html=renderToStaticMarkup(<BranchTrackingBadge value={tracked} oid="commit"/>);expect(html).toContain('3↑');expect(html).toContain('ตาม 2');expect(html).toContain(tracked.upstream!); } finally {setLocale('en');}
});
