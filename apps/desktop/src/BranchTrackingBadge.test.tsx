import { expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { BranchTrackingBadge, BranchSyncCount } from './BranchTrackingBadge';
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
    expect(html).not.toContain('↑');expect(html).not.toContain('class="in-sync">=</span>');
  }
  expect(renderToStaticMarkup(<BranchTrackingBadge value={tracked} oid="other"/>)).toContain('unavailable');
  expect(renderToStaticMarkup(<BranchTrackingBadge value={{...tracked,ahead:null}} oid="commit"/>)).not.toContain('class="in-sync">=</span>');
});
it('represents known synchronization and translates the comparison without changing ref data', () => {
  expect(renderToStaticMarkup(<BranchTrackingBadge value={{...tracked,ahead:0,behind:0}} oid="commit"/>)).toContain('class="in-sync">=</span>');
  expect(renderToStaticMarkup(<BranchTrackingBadge value={{...tracked,ahead:0,behind:0}} oid="commit"/>)).toContain('Same commit as');
  setLocale('th');try { const html=renderToStaticMarkup(<BranchTrackingBadge value={tracked} oid="commit"/>);expect(html).toContain('3↑');expect(html).toContain('ตาม 2');expect(html).toContain(tracked.upstream!); } finally {setLocale('en');}
});

it('keeps toolbar counts explicit at zero and never reuses a different revision', () => {
  for (const direction of ['pull','push'] as const) {
    const render=(value:BranchTracking,oid='commit')=>renderToStaticMarkup(<BranchSyncCount value={value} oid={oid} direction={direction}/>);
    expect(render(tracked)).toContain(direction==='push'?'>3<':'>2<');
    expect(render({...tracked,ahead:0,behind:0})).toContain('>0<');
    expect(render(tracked,'old-commit')).toContain('>?<');
    for(const state of ['gone','shallow'] as const) expect(render({...tracked,state})).toContain('>?<');
    expect(render({...tracked,state:'noUpstream',ahead:null,behind:null})).toContain('>—<');
    for(const ahead of [NaN,-1,Infinity,Number.MAX_SAFE_INTEGER+1]) expect(render({...tracked,ahead})).toContain('>?<');
  }
});
it('keeps upstream divergence visible even when a different push destination is configured', () => {
  for(const value of [{...tracked,pushRemote:'publish'}, {...tracked,pushBranch:'release'}, {...tracked,pushBranch:null}]) {
    const html=renderToStaticMarkup(<BranchSyncCount value={value} oid="commit" direction="push"/>);
    expect(html).toContain('>3<');expect(html).toContain('locally fetched refs');
    expect(renderToStaticMarkup(<BranchSyncCount value={value} oid="commit" direction="pull"/>)).toContain('>2<');
  }
});

it('shows display-only origin/main counts without turning them into a configured upstream', () => {
  const value:BranchTracking={...tracked,state:'noUpstream',upstream:null,ahead:null,behind:null,comparison:{reference:'refs/remotes/origin/main',oid:'remote-commit',ahead:1,behind:4}};
  const badge=renderToStaticMarkup(<BranchTrackingBadge value={value} oid="commit"/>);
  expect(badge).toContain('1↑');expect(badge).toContain('4↓');expect(badge).toContain('Display comparison only');expect(badge).toContain('refs/remotes/origin/main');
  expect(renderToStaticMarkup(<BranchSyncCount value={value} oid="commit" direction="pull"/>)).toContain('>4<');
  expect(renderToStaticMarkup(<BranchSyncCount value={value} oid="commit" direction="push"/>)).toContain('>1<');
  expect(renderToStaticMarkup(<BranchSyncCount value={value} oid="other" direction="pull"/>)).toContain('>?<');
  expect(value.upstream).toBeNull();
});

it('shows equality and zero pending commits for a published branch without upstream',()=>{
 const value:BranchTracking={...tracked,state:'noUpstream',upstream:null,ahead:null,behind:null,comparison:{reference:'refs/remotes/origin/feature/published',oid:'commit',ahead:0,behind:0}};
 const badge=renderToStaticMarkup(<BranchTrackingBadge value={value} oid="commit"/>);
 expect(badge).toContain('class="in-sync">=</span>');expect(badge).not.toContain('↑');expect(badge).not.toContain('↓');
 for(const direction of ['pull','push'] as const){const count=renderToStaticMarkup(<BranchSyncCount value={value} oid="commit" direction={direction}/>);expect(count).toContain('>0<');expect(count).toContain('refs/remotes/origin/feature/published');}
 expect(value.upstream).toBeNull();
});
