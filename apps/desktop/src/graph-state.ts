import type { GraphPage, GraphRow } from './contracts';
export const ROW_HEIGHT = 36;
export const MAX_ROWS = 1000;
// Refreshing an existing entry also makes it most recent. All insertion paths
// (including search navigation) share this bound.
export function retainRecent<T>(current: Record<string, T>, id: string, value: T): Record<string, T> {
  return Object.fromEntries([...Object.entries(current).filter(([key]) => key !== id).slice(-7), [id, value]]);
}
export const GRAPH_CACHE_BYTES = 32 * 1024 * 1024;
export function retainGraphSessions<T extends { rows: GraphRow[]; page: GraphPage }>(current: Record<string, T>, id: string, value: T): Record<string, T> {
  const result = retainRecent(current, id, value);
  const weight = (session: T) => {
    const unique = new Set([...session.rows, ...session.page.rows]);
    let bytes = 1024 + session.page.refs.reduce((n, ref) => n + 128 + 2 * (ref.oid.length + ref.name.length), 0);
    bytes += (session.page.tracking ?? []).reduce((n, item) => n + 256 + 2 * [item.branch, item.oid, item.upstream, item.remote, item.remoteBranch, item.pushRemote, item.pushBranch].reduce<number>((sum, text) => sum + (text?.length ?? 0), 0), 0);
    for (const row of unique) bytes += 512 + row.incoming.length * 32 + row.edges.length * 48
      + 2 * (row.commit.oid.length + row.commit.subject.length + row.commit.author.length + row.commit.email.length)
      + row.commit.parents.reduce((n, parent) => n + 32 + parent.length * 2, 0);
    return bytes;
  };
  let total = Object.values(result).reduce((n, session) => n + weight(session), 0);
  for (const key of Object.keys(result)) {
    if (total <= GRAPH_CACHE_BYTES) break;
    if (key === id) continue; // The active repository remains visible, even if it alone exceeds the estimate.
    total -= weight(result[key]); delete result[key];
  }
  return result;
}
export function appendPage(current: GraphRow[], incoming: GraphRow[]) {
  const combined = [...current, ...incoming];
  const dropped = Math.max(0, combined.length - MAX_ROWS);
  return { rows: combined.slice(dropped), dropped };
}
export function windowRange(scrollTop: number, viewportHeight: number, count: number) {
  const start = Math.max(0, Math.min(count - 1, Math.floor(scrollTop / ROW_HEIGHT) - 8));
  return { start, end: Math.min(count, Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + 8) };
}
export function refLabels(page: Pick<GraphPage, 'refs'>, oid: string) {
  return page.refs.filter(ref => ref.oid === oid).map(ref => ref.name.replace(/^refs\/(heads|remotes|tags)\//, ''));
}

export type GraphRefLabel = { label: string; kind: 'local' | 'remote' | 'tag'; tracked: boolean; current: boolean; fullNames: string[] };
// Co-located local and remote refs share a badge; divergent tips always stay separate.
export function graphRefLabels(page: Pick<GraphPage, 'refs' | 'status'>, oid: string): GraphRefLabel[] {
  const refs = page.refs.filter(ref => ref.oid === oid && !(ref.name.startsWith('refs/remotes/') && ref.name.endsWith('/HEAD')));
  const local = refs.filter(ref => ref.name.startsWith('refs/heads/'));
  const remotesByBranch = new Map<string, typeof refs>();
  const grouped = new Set<string>();
  for (const ref of refs) {
    if (!ref.name.startsWith('refs/remotes/')) continue;
    const branch = ref.name.slice(13).split('/').slice(1).join('/');
    const matches = remotesByBranch.get(branch) ?? []; matches.push(ref); remotesByBranch.set(branch, matches);
  }
  const result: GraphRefLabel[] = local.map(ref => {
    const label = ref.name.slice(11);
    const remotes = remotesByBranch.get(label) ?? [];
    grouped.add(ref.name); for (const remote of remotes) grouped.add(remote.name);
    return { label, kind: 'local', tracked: remotes.length > 0, current: page.status.branch === label && page.status.head === oid, fullNames: [ref.name, ...remotes.map(remote => remote.name)] };
  });
  for (const ref of refs) {
    if (grouped.has(ref.name)) continue;
    const kind = ref.name.startsWith('refs/tags/') ? 'tag' : 'remote';
    result.push({ label: ref.name.replace(/^refs\/(heads|remotes|tags)\//, ''), kind, tracked: false, current: false, fullNames: [ref.name] });
  }
  if (page.status.head === oid && !page.status.branch) result.unshift({ label: 'HEAD', kind: 'local', tracked: false, current: true, fullNames: ['Detached HEAD'] });
  return result.sort((a, b) => Number(b.current) - Number(a.current));
}

export type HistoryItem = { id: string; row: GraphRow | null; stash: import('./contracts').StashEntry | null; base: GraphRow | null };
export const stashSelection = (stash: Pick<import('./contracts').StashEntry, 'index' | 'oid'>) => `stash:${stash.index}:${stash.oid}`;
export function historyItems(rows: GraphRow[], stashes: import('./contracts').StashEntry[]): HistoryItem[] {
  const loaded = new Set(rows.map(row => row.commit.oid));
  const byBase = new Map<string, typeof stashes>();
  for (const stash of stashes) {
    if (stash.baseOid && loaded.has(stash.baseOid)) {
      const entries = byBase.get(stash.baseOid) ?? []; entries.push(stash); byBase.set(stash.baseOid, entries);
    }
  }
  const items: HistoryItem[] = stashes.filter(stash => !stash.baseOid || !loaded.has(stash.baseOid)).map(stash => ({ id: stashSelection(stash), row: null, stash, base: null }));
  for (const row of rows) {
    for (const stash of byBase.get(row.commit.oid) ?? []) items.push({ id: stashSelection(stash), row: null, stash, base: row });
    items.push({ id: row.commit.oid, row, stash: null, base: null });
  }
  return items;
}

export function graphLabelsByCommit(page: Pick<GraphPage, 'refs' | 'status'>, rows: GraphRow[]) {
  const byOid = new Map<string, GraphPage['refs']>();
  for (const ref of page.refs) {
    const refs = byOid.get(ref.oid) ?? []; refs.push(ref); byOid.set(ref.oid, refs);
  }
  return new Map(rows.map(row => [row.commit.oid, graphRefLabels({ refs: byOid.get(row.commit.oid) ?? [], status: page.status }, row.commit.oid)]));
}

// Saved work never borrows a real Git lane, including lanes occupied only by strokes.
export function stashLanes(rows: GraphRow[], stashes: import('./contracts').StashEntry[]) {
  const gitLanes = Math.max(1, ...rows.flatMap(row => [row.width, row.lane + 1, ...row.incoming.map(stroke => stroke.lane + 1), ...row.edges.flatMap(edge => [edge.from + 1, edge.to + 1]) ]));
  const byBase = new Map<string, number[]>(); const byEntry = new Map<string, number>();
  const loaded = new Set(rows.map(row => row.commit.oid));
  for (const stash of stashes) {
    if (!stash.baseOid || !loaded.has(stash.baseOid)) { byEntry.set(stashSelection(stash), gitLanes); continue; }
    const lanes = byBase.get(stash.baseOid) ?? [];
    byEntry.set(stashSelection(stash), gitLanes + lanes.length); lanes.push(gitLanes + lanes.length); byBase.set(stash.baseOid, lanes);
  }
  const count = stashes.length ? Math.max(1, ...[...byBase.values()].map(lanes => lanes.length)) : 0;
  return { gitLanes, width: Math.max(88, (gitLanes + count) * 24 + 24), byBase, byEntry };
}
export function stashTitle(message: string) { return message.replace(/ \[GitNova [0-9a-f-]{36}\]$/i, ''); }
