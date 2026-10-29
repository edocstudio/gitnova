import {ciStatusLabel} from './ci-requests';
import type {GraphCiValue,GraphCiLink} from './ci-requests';
import {useGraphCi} from './useGraphCi';
import { t, dateLocale } from './i18n';
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import type { StashList, MergeState, GraphPage, GraphRow } from './contracts';
import { stashLanes, stashTitle, historyItems, graphLabelsByCommit, ROW_HEIGHT, windowRange } from './graph-state';
import { Icon } from './Icons';
import { BranchTrackingBadge } from './BranchTrackingBadge';
export const WORKING = 'working';
export type GraphIntent = { oid: string; x: number; y: number; refName?: string; quick: boolean; action?: 'push' | 'fetch' | 'pull'; pipeline?: GraphCiLink; workflows?: GraphCiLink[] };
export type GraphSession = { page: GraphPage; rows: GraphRow[]; selected: string | null; scroll: number; base: number; revision?: number; notice?: string; anchor?: string; anchorOffset?: number };
const colors = ['#4b91ff', '#54ced7', '#b17cff', '#f4b64d', '#46c88d', '#ef779a'].map((value,index)=>`var(--graph-${index}, ${value})`);
const color = (lane: number) => colors[lane % colors.length];
const initials = (author: string) => author.split(/\s+/).map(part => part[0]).slice(0, 2).join('');
function authorColor(author: string) { return colors[[...author].reduce((hash, c) => (hash * 31 + c.charCodeAt(0)) >>> 0, 0) % colors.length]; }
function date(timestamp: number) {
  const elapsed = Date.now() / 1000 - timestamp;
  if (elapsed >= 0 && elapsed < 3600) return t('Just now');
  if (elapsed >= 3600 && elapsed < 86400) { const hours = Math.floor(elapsed / 3600); return t(hours === 1 ? '{0} hour ago' : '{0} hours ago', [hours]); }
  if (elapsed >= 86400 && elapsed < 604800) { const days = Math.floor(elapsed / 86400); return t(days === 1 ? '{0} day ago' : '{0} days ago', [days]); }
  return new Date(timestamp * 1000).toLocaleDateString(dateLocale(), { month: 'short', day: 'numeric' });
}
function GraphCiCell({value,observed}:{value?:GraphCiValue;observed?:string}){
 const status=value?.status==='CI unavailable'&&observed?observed:value?.status??observed;
 const pipeline=value?.pipeline;
 const title=[pipeline?`${t(pipeline.kind==='workflow'?'Workflow':'Pipeline')} #${pipeline.id} · ${t(ciStatusLabel(status??'unknown'))}${pipeline.reference?` · ${pipeline.reference}`:''}`:status?t(ciStatusLabel(status)):t('Connect a matching host account in Settings → Connections to read CI.'),...(value?.workflows??[]).map(run=>`${run.name||t('Workflow')} #${run.id} · ${t(ciStatusLabel(run.status??'unknown'))} · ${run.reference}`),value?.partial?t('Some CI results are unavailable or incomplete.'):'',value?.stale?t('Showing cached CI status. Refresh failed.'):'',value?.checkedAt?`${t('Last checked')}: ${new Date(value.checkedAt*1000).toLocaleTimeString(dateLocale())}`:''].filter(Boolean).join('\n');
 return <span className="commit-ci" title={title}>{status?<span className={`ci-badge ci-${status}`}><span aria-hidden="true">{status==='success'?'✓':status==='failed'?'✕':['running','pending','created','preparing','waiting_for_resource','Reading CI…'].includes(status)?'◌':'•'}</span> {t(ciStatusLabel(status))}{pipeline?<span className="ci-pipeline-id"> · #{pipeline.id}</span>:!!value?.workflows?.length&&<span> · {t(value.workflows.length===1?'{0} workflow':'{0} workflows',[value.workflows.length])}</span>}</span>:<span className="ci-empty">—</span>}</span>;
}
function GraphMark({ row, width, head, labelled, savedLanes }: { row: GraphRow; width: number; head: boolean; labelled: boolean; savedLanes: number[] }) {
  const x = (lane: number) => 18 + lane * 24; const middle = ROW_HEIGHT / 2;
  const bend = (from: number, to: number) => {
    if (from === to) return `M${x(from)},${middle} V${ROW_HEIGHT}`;
    const direction = to > from ? 1 : -1; const radius = 7;
    return `M${x(from)},${middle} Q${x(from)},${middle + radius} ${x(from) + direction * radius},${middle + radius} H${x(to) - direction * radius} Q${x(to)},${middle + radius} ${x(to)},${middle + radius * 2} V${ROW_HEIGHT}`;
  };
  return <svg aria-hidden="true" width={width} height={ROW_HEIGHT} className="graph-mark" data-lane={row.lane} data-color={row.color}>
    {savedLanes.map(lane => <path key={`saved-${lane}`} d={`M${x(lane)},0 V5 Q${x(lane)},${middle} ${x(lane)-12},${middle} H${x(row.lane)}`} stroke="var(--graph-stash, #e5b35c)" strokeDasharray="3 3"/>)}
    {labelled && <path className="ref-connector" d={`M0,${middle} H${x(row.lane)}`} stroke={color(row.color)} />}
    {row.incoming.map(stroke => <path key={`top-${stroke.lane}`} d={`M${x(stroke.lane)},0 V${middle}`} stroke={color(stroke.color)} />)}
    {row.edges.map((edge, i) => <path key={i} d={bend(edge.from, edge.to)} stroke={color(edge.color)} />)}
    {head && <circle className="head-ring" cx={x(row.lane)} cy={middle} r={8} fill="var(--graph-node-bg, #101a23)" stroke={color(row.color)} />}
    <circle className={row.commit.parents.length > 1 ? 'merge-node' : 'commit-node'} cx={x(row.lane)} cy={middle} r={row.commit.parents.length > 1 ? 5 : 4} fill={row.commit.parents.length > 1 ? 'var(--graph-node-bg, #101a23)' : color(row.color)} stroke={color(row.color)} />
  </svg>;
}
export function History({ membershipId, ciBadges, submodules, stashes, stashError, mergeState, session, onStash, onPop, stashBusy, onSelect, onScroll, onMore, busy, onAction }: { membershipId:string; ciBadges?:Record<string,string>; submodules?: ReactNode; onStash: () => void; onPop: () => void; stashBusy: boolean; stashes?: StashList; stashError?: string; mergeState?: MergeState; onAction: (intent: GraphIntent) => void; session: GraphSession; onSelect: (oid: string) => void; onScroll: (top: number, anchor: string, offset: number) => void; onMore: () => void; busy: boolean }) {
  // Relative dates still advance when equal poll observations skip rerenders.
  // Repaint only the visible rows once a minute; keep graph geometry memoized.
  const [,setMinute]=useState(0);
  useEffect(()=>{const tick=()=>{if(!document.hidden)setMinute(n=>n+1);};const timer=window.setInterval(tick,60000);document.addEventListener('visibilitychange',tick);return()=>{window.clearInterval(timer);document.removeEventListener('visibilitychange',tick);};},[]);
  const items = useMemo(() => historyItems(session.rows, stashes?.entries ?? []), [session.rows, stashes]);
  const tracking = useMemo(() => new Map((session.page.tracking ?? []).map(value => [value.branch, value])), [session.page.tracking]);
  const viewport = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState(600); const [scroll, setScroll] = useState(session.scroll);
  useLayoutEffect(() => { const at = session.anchor ? items.findIndex(item => item.id === session.anchor) : -1; const top = session.scroll > 0 && at >= 0 ? at * ROW_HEIGHT + (session.anchorOffset ?? 0) : session.scroll; if (viewport.current) viewport.current.scrollTop = top; setScroll(top); }, [session.base, session.revision, items]);
  useEffect(() => { const node = viewport.current; if (!node) return; const observer = new ResizeObserver(() => setHeight(node.clientHeight)); observer.observe(node); return () => observer.disconnect(); }, []);
  const range = windowRange(scroll, height, items.length);
  const visibleOids = items.slice(Math.floor(scroll / ROW_HEIGHT), Math.ceil((scroll + height) / ROW_HEIGHT)).flatMap(item => item.row ? [item.row.commit.oid] : []).join(',');
  const graphCi = useGraphCi(membershipId, visibleOids);
  const savedGeometry = useMemo(() => stashLanes(session.rows, stashes?.entries ?? []), [session.rows, stashes]);
  const laneWidth = savedGeometry.width;
  const labels = useMemo(() => graphLabelsByCommit(session.page, session.rows), [session.page.refs, session.page.status.branch, session.page.status.head, session.rows]);
  const selectionVisible = items.some(item => item.id === session.selected);
  const choose = (index: number) => {
    const item = items[index]; if (!item) return;
    onSelect(item.id); viewport.current?.scrollTo({ top: Math.max(0, index * ROW_HEIGHT - height / 2) });
    requestAnimationFrame(() => document.getElementById(`commit-${item.id}`)?.focus());
  };
  return <div className="history-wrap" style={{ '--graph-width': `${laneWidth}px` } as CSSProperties}>
    {!session.page.status.bare && <div className="working-row-wrap"><button className={`working-row ${session.selected === WORKING ? 'selected' : ''}`} onClick={() => onSelect(WORKING)}><span className="working-node"><Icon name="layers" size={17} /></span><span className="working-row-copy"><strong>{t("Working changes")}</strong><span>{mergeState && mergeState.phase !== 'none' ? mergeState.phase !== 'pending' ? t('Inspect current operation') : mergeState.conflicts.length ? t("Resolve {0} {1} conflicts", [mergeState.conflicts.length, mergeState.operation === 'cherryPick' ? t("cherry-pick") : mergeState.operation === 'revert' ? t("revert") : t("merge")]) : t("Review staged {0} result", [mergeState.operation === 'cherryPick' ? t("cherry-pick") : mergeState.operation === 'revert' ? t("revert") : t("merge")]) : session.page.status.dirty ? t("Review and prepare your next commit") : t("Your working tree is clean")}</span></span><span className="change-count">{session.page.status.changedEntries}</span><Icon name="chevron" size={16} /></button><div className="working-stash-actions" role="group" aria-label={t('Save and restore work')}><button disabled={stashBusy || Boolean(mergeState && mergeState.phase !== 'none') || !session.page.status.dirty} onClick={onStash}>{t('Stash…')}</button><button disabled={stashBusy || Boolean(mergeState && mergeState.phase !== 'none') || !stashes?.entries.length} title={mergeState && mergeState.phase !== 'none' ? t('Open Working changes to finish, abort or inspect the current operation before using Pop.') : t('Restore the latest stash onto {0}', [session.page.status.branch ?? t('Detached HEAD')])} onClick={onPop}>{t('Pop…')}</button></div></div>}
    {submodules}
    <div className="history-heading-viewport" ref={heading}><div className="history-heading"><span>{t("Branch / Tag")}</span><span>{t("Graph")}</span><span>{t("Commit message")}</span><span className="ci-heading">{t("CI")}</span><span>{t("Author")}</span><span>{t("Date")}</span></div></div>
    {stashError && <div className="notice compact" role="status">{t('Saved work could not be refreshed:')} {stashError}</div>}
    {stashes?.more && <div className="notice compact">{t('Showing the latest 200 stashes. Inspect older entries with Git.')}</div>}
    {session.notice && <div className="notice compact" role="status">{t(session.notice)}</div>}
    {session.base > 0 && <div className="notice compact">{t("Showing older history from commit")} {session.base + 1}{t(". Refresh to return to the latest commits.")}</div>}
    <div className="history-scroll" ref={viewport} aria-label={t("Commit history")} onScroll={event => { const top = event.currentTarget.scrollTop; if (heading.current) heading.current.scrollLeft = event.currentTarget.scrollLeft; setScroll(top); const item = items[Math.floor(top / ROW_HEIGHT)]; onScroll(top, item?.id ?? '', top % ROW_HEIGHT); }}>
      <div className="history-content"><div style={{ height: range.start * ROW_HEIGHT }} />
        {items.slice(range.start, range.end).map((item, local) => {
          const index = range.start + local;
          if (item.stash) {
            const entry = item.stash; const lane = savedGeometry.byEntry.get(item.id) ?? savedGeometry.gitLanes; const sx = 18 + lane * 24; const baseLanes = item.base ? savedGeometry.byBase.get(item.base.commit.oid) ?? [] : [];
            return <button key={item.id} id={`commit-${item.id}`} data-base-oid={entry.baseOid ?? ''} className={`commit-row stash-graph-row ${session.selected === item.id ? 'selected' : ''}`} style={{ height: ROW_HEIGHT }} aria-pressed={session.selected === item.id} tabIndex={session.selected === item.id || (!selectionVisible && index === 0) ? 0 : -1} onClick={() => onSelect(item.id)} onContextMenu={event => { event.preventDefault(); onSelect(item.id); }} onKeyDown={event => { const next = event.key === 'ArrowDown' ? index + 1 : event.key === 'ArrowUp' ? index - 1 : event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : null; if (next !== null) { event.preventDefault(); choose(next); } }}>
              <span className="graph-refs"><span className="graph-ref stash"><Icon name="layers" size={12}/>{`stash@{${entry.index}}`}</span></span>
              <svg className="graph-mark stash-mark" aria-hidden="true" width={laneWidth} height={ROW_HEIGHT} data-stash-lane={lane} data-git-lanes={savedGeometry.gitLanes}>{item.base?.incoming.map(stroke => <path key={stroke.lane} d={`M${18 + stroke.lane * 24},0 V${ROW_HEIGHT}`} stroke={color(stroke.color)}/>)}{baseLanes.filter(value => value < lane).map(value => <path key={`carry-${value}`} d={`M${18 + value * 24},0 V36`} stroke="var(--graph-stash, #e5b35c)" strokeDasharray="3 3"/>)}{item.base && <path d={`M${sx},18 V36`} stroke="var(--graph-stash, #e5b35c)" strokeDasharray="3 3"/>}<rect x={sx - 5} y={13} width={10} height={10} rx={2} fill="var(--panel)" stroke="var(--graph-stash, #e5b35c)"/></svg>
              <span className="commit-description" title={entry.message}><span className="subject">{stashTitle(entry.message)}</span><span className="head-label">{t('Saved work')}</span></span><span className="commit-ci" aria-hidden="true">—</span><span className="author" title={entry.baseOid ?? ''}>{item.base ? t('Base {0}', [entry.baseOid?.slice(0, 8)]) : t('Base outside loaded history')}</span><span className="commit-date">{date(entry.timestamp)}</span>
            </button>;
          }
          const row = item.row!; const refs = labels.get(row.commit.oid) ?? []; const head = row.commit.oid === session.page.status.head;
          return <button id={`commit-${row.commit.oid}`} key={row.commit.oid} className={`commit-row ${session.selected === row.commit.oid ? 'selected' : ''}`} style={{ height: ROW_HEIGHT, '--lane-color': color(row.color) } as CSSProperties} title={`${row.commit.oid} · ${row.commit.subject}`} aria-pressed={session.selected === row.commit.oid} tabIndex={session.selected === row.commit.oid || (!selectionVisible && index === 0) ? 0 : -1}
            onContextMenu={event => { event.preventDefault(); onSelect(row.commit.oid); const box = event.currentTarget.getBoundingClientRect(); const pointer = Number.isFinite(event.clientX) && Number.isFinite(event.clientY) && (event.clientX !== 0 || event.clientY !== 0); onAction({ pipeline: graphCi[row.commit.oid]?.pipeline, workflows:graphCi[row.commit.oid]?.workflows, oid: row.commit.oid, x: pointer ? event.clientX : box.left + 180, y: pointer ? event.clientY : box.top, refName: (event.target as Element).closest<HTMLElement>('[data-ref-name]')?.dataset.refName, quick: false }); }}
            onDoubleClick={event => { if (!(event.target as Element).closest('.graph-mark, .graph-ref')) return; onAction({ pipeline: graphCi[row.commit.oid]?.pipeline, workflows:graphCi[row.commit.oid]?.workflows, oid: row.commit.oid, x: event.clientX, y: event.clientY, refName: (event.target as Element).closest<HTMLElement>('[data-ref-name]')?.dataset.refName, quick: true }); }}
            onClick={() => onSelect(row.commit.oid)} onKeyDown={event => { if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) { event.preventDefault(); const box = event.currentTarget.getBoundingClientRect(); onAction({ pipeline: graphCi[row.commit.oid]?.pipeline, workflows:graphCi[row.commit.oid]?.workflows, oid: row.commit.oid, x: box.left + 180, y: box.top, quick: false }); return; } const next = event.key === 'ArrowDown' ? index + 1 : event.key === 'ArrowUp' ? index - 1 : event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : null; if (next !== null) { event.preventDefault(); choose(next); } }}>
            <span className={`graph-refs ${refs.length ? 'has-refs' : ''}`} title={refs.flatMap(ref => ref.fullNames).join('\n')}>
              <span className="ref-stack">{refs.slice(0, 2).map(ref => <span key={ref.fullNames[0]} data-ref-name={ref.fullNames[0]} className={`graph-ref ${ref.kind} ${ref.current ? 'current' : ''}`}>
                <Icon name={ref.current ? 'check' : ref.kind === 'tag' ? 'tag' : 'branch'} size={12} /><span>{ref.label}</span>{ref.tracked && <Icon name="remote" size={11} />}{ref.fullNames.some(name => name.startsWith('refs/heads/')) && <BranchTrackingBadge value={tracking.get(ref.label)} oid={row.commit.oid}/>}
              </span>)}</span>{refs.length > 2 && <span className="extra-refs">+{refs.length - 2}</span>}
            </span>
            <GraphMark savedLanes={savedGeometry.byBase.get(row.commit.oid) ?? []} row={row} width={laneWidth} head={head} labelled={refs.length > 0} />
            <span className="commit-description"><span className="subject">{row.commit.subject || t("(no commit message)")}</span>{head && <span className="head-label">{t("HEAD")}</span>}</span>
            <GraphCiCell value={graphCi[row.commit.oid]} observed={ciBadges?.[row.commit.oid]}/>
            <span className="author" title={`${row.commit.author} <${row.commit.email}>`}><span className="row-avatar" aria-hidden="true" style={{ '--author-color': authorColor(row.commit.author) } as CSSProperties}>{initials(row.commit.author)}</span><span>{row.commit.author}</span></span>
            <span className="commit-date" title={new Date(row.commit.timestamp * 1000).toLocaleString(dateLocale())}>{date(row.commit.timestamp)}</span>
          </button>;
        })}<div style={{ height: (items.length - range.end) * ROW_HEIGHT }} />
      </div>
      {session.page.next && <button className="load-more" disabled={busy} onClick={onMore}>{busy ? t("Reading history…") : t("Load older commits")}<Icon name="chevron" size={15} /></button>}
      {!session.rows.length && <div className="quiet-state"><Icon name="commit" size={26} /><h2>{t("No commits yet")}</h2><p>{t("Stage your first files from Working changes.")}</p></div>}
      {!!session.rows.length && !session.page.next && <p className="history-end"><span />{t("Beginning of available history")}<span /></p>}
    </div>
  </div>;
}
