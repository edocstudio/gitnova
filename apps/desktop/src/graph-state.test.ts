import { describe, expect, it } from 'vitest';
import { appendPage, MAX_ROWS, graphRefLabels, graphLabelsByCommit, retainRecent, retainGraphSessions, refLabels, ROW_HEIGHT, windowRange } from './graph-state';
import type { GraphRow } from './contracts';
function row(index: number): GraphRow { return { commit: { oid: String(index), parents: [], author: 'Fixture', email: 'test@example.invalid', subject: 'Fixture', timestamp: 0 }, lane: 0, color: 0, incoming: [], edges: [], width: 1 }; }
describe('bounded history state', () => {
  it('evicts heavy inactive graphs before reaching eight repositories without truncating commit data', () => {
    const make = (index: number) => { const item = row(index); item.commit.subject = 'x'.repeat(6 * 1024 * 1024); return { rows: [item], page: { rows: [item], refs: [], status: { head: null, branch: null, dirty: false, changedEntries: 0, bare: false }, next: null, fingerprint: String(index) } }; };
    const first = make(1); const second = make(2); const active = make(3);
    const cache = retainGraphSessions({ first, second }, 'active', active);
    expect(Object.keys(cache)).toEqual(['second', 'active']);
    expect(cache.active.rows[0].commit.subject).toHaveLength(6 * 1024 * 1024);
  });
  it('bounds repeated repository and search navigation while preserving the active entry', () => {
    let state: Record<string, number> = {};
    for (let i = 0; i < 100; i++) state = retainRecent(state, `repo-${i}`, i);
    expect(Object.keys(state)).toHaveLength(8);
    state = retainRecent(state, 'repo-92', 999);
    state = retainRecent(state, 'repo-100', 100);
    expect(state['repo-92']).toBe(999); expect(state['repo-93']).toBeUndefined();
    expect(Object.keys(state)).toHaveLength(8);
  });
  it('indexes many refs with identical branch, remote, tag and detached labels', () => {
    const rows = Array.from({ length: 1000 }, (_, i) => row(i));
    const page = { status: { branch: 'main', head: '1', dirty: false, changedEntries: 0, bare: false }, refs: [
      ...rows.map(r => ({ oid: r.commit.oid, name: `refs/heads/branch-${r.commit.oid}` })),
      { oid: '1', name: 'refs/heads/main' }, { oid: '1', name: 'refs/remotes/origin/main' },
      { oid: '2', name: 'refs/tags/v1' },
    ] };
    const indexed = graphLabelsByCommit(page, rows);
    for (const oid of ['0', '1', '2', '999']) expect(indexed.get(oid)).toEqual(graphRefLabels(page, oid));
  });
  it('evicts oldest rows and reports the scroll adjustment when loading older history', () => {
    const result = appendPage(Array.from({ length: MAX_ROWS }, (_, i) => row(i)), Array.from({ length: 100 }, (_, i) => row(MAX_ROWS + i)));
    expect(result.rows).toHaveLength(MAX_ROWS); expect(result.dropped).toBe(100); expect(result.rows[0].commit.oid).toBe('100'); expect(result.rows.at(-1)?.commit.oid).toBe('1099');
  });
  it('renders only the viewport and overscan rows', () => {
    expect(windowRange(500 * ROW_HEIGHT, 10 * ROW_HEIGHT, 1000)).toEqual({ start: 492, end: 518 });
    expect(windowRange(0, 100, 2)).toEqual({ start: 0, end: 2 });
  });
  it('keeps branch and tag labels attached to their actual commit', () => {
    expect(refLabels({ refs: [{ oid: 'a', name: 'refs/heads/main' }, { oid: 'a', name: 'refs/tags/v1' }, { oid: 'b', name: 'refs/heads/other' }] }, 'a')).toEqual(['main', 'v1']);
  });
});

it('groups matching remote refs only at the same tip and preserves tags and detached HEAD', () => {
  const page = { status: { branch: 'main', head: 'a', dirty: false, changedEntries: 0, bare: false }, refs: [
    { oid: 'a', name: 'refs/heads/main' }, { oid: 'a', name: 'refs/remotes/origin/main' },
    { oid: 'a', name: 'refs/tags/main' }, { oid: 'a', name: 'refs/remotes/origin/HEAD' },
    { oid: 'b', name: 'refs/remotes/upstream/main' },
  ] };
  const labels = graphRefLabels(page, 'a');
  expect(labels).toHaveLength(2); expect(labels[0]).toMatchObject({ label: 'main', current: true, tracked: true, kind: 'local' });
  expect(labels[0].fullNames).toEqual(['refs/heads/main', 'refs/remotes/origin/main']);
  expect(labels[1].kind).toBe('tag'); expect(graphRefLabels(page, 'b')[0].label).toBe('upstream/main');
  expect(graphRefLabels({ ...page, status: { ...page.status, branch: null } }, 'a')[0].label).toBe('HEAD');
});

it('places each saved entry at its exact loaded base and labels missing bases separately', async () => {
  const { historyItems, stashSelection } = await import('./graph-state');
  const entries = [{ index: 0, oid: 'stash-a', baseOid: '2', message: 'Saved', timestamp: 1 }, { index: 1, oid: 'stash-b', baseOid: 'outside', message: 'Older', timestamp: 0 }];
  const items = historyItems([row(1), row(2), row(3)], entries);
  expect(items.map(item => item.id)).toEqual([stashSelection(entries[1]), '1', stashSelection(entries[0]), '2', '3']);
  expect(items[0].base).toBeNull(); expect(items[2].base?.commit.oid).toBe('2');
  expect(items.filter(item => item.row).map(item => item.row!.commit.oid)).toEqual(['1', '2', '3']);
  expect(windowRange(0, 36, items.length).end).toBeLessThanOrEqual(items.length);
});

it('reserves distinct stash lanes beyond every real node and stroke, including occupied neighbor branches', async () => {
  const { stashLanes, stashSelection } = await import('./graph-state');
  const first = { ...row(1), lane: 0, width: 2, incoming: [{lane: 1, color: 1}, {lane: 4, color: 2}], edges: [{ from: 0, to: 6, color: 0 }] };
  const entries = [0, 1].map(index => ({ index, oid: `stash-${index}`, baseOid: '1', message: 'saved', timestamp: 0 }));
  const layout = stashLanes([first, row(2)], entries);
  expect(layout.gitLanes).toBe(7); expect(layout.byBase.get('1')).toEqual([7, 8]);
  for (const entry of entries) { const lane = layout.byEntry.get(stashSelection(entry))!; expect(lane).toBeGreaterThan(6); expect(18 + lane * 24 + 5).toBeLessThan(layout.width); }
  expect(stashLanes([first], []).byBase.size).toBe(0);
});
it('keeps original stash messages intact while leaving generated recovery identifiers in details', async () => {
  const { stashTitle } = await import('./graph-state');
  const original = 'On main: work ภาษาไทย [GitNova 12345678-1234-1234-1234-123456789abc]';
  expect(stashTitle(original)).toBe('On main: work ภาษาไทย'); expect(original).toContain('[GitNova'); expect(stashTitle('User [GitNova custom]')).toBe('User [GitNova custom]');
});
