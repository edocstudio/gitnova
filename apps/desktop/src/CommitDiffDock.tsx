import { t } from './i18n';
import {ConflictResolver} from './ConflictResolver';
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import type { CommitDetail, NativePath } from './contracts';
import { findPatch, parseDiff, splitDiffLines } from './diff';
import type { DiffLine, PatchFile } from './diff';
import { Icon } from './Icons';
import { api, errorMessage } from './api';

type HunkControls={headers:string[];staged:boolean;disabled:boolean;onApply:(index:number)=>void};
export type DockPatch = { hunks?:HunkControls; diff: string; truncated: boolean; label: string; title: string; comparison: string; before: string; after: string; selectedPath: NativePath | null; emptyMessage?: string; editor?: { before: string; incoming?: string | null; beforeUnavailable?: boolean; currentDeleted?: boolean; incomingDeleted?: boolean; currentLabel?: string; incomingLabel?: string; readOnly?: boolean; value: string; onChange: (value: string) => void; disabled: boolean } };

function MergeEditor({ editor }: { editor: NonNullable<DockPatch['editor']> }) {
  const left = useRef<HTMLPreElement>(null); const right = useRef<HTMLTextAreaElement>(null);
  return <div className="merge-editor" role="region" aria-label={t("Merge editor columns")}><div className="merge-editor-label">{t("Current branch (read only)")}</div><div className="merge-editor-label">{t("Editable result · review before saving")}</div><pre ref={left} tabIndex={0} onScroll={e => { if (right.current && Math.abs(e.currentTarget.scrollTop - Math.min(right.current.scrollTop, Math.max(0, e.currentTarget.scrollHeight - e.currentTarget.clientHeight))) > 1) right.current.scrollTop = e.currentTarget.scrollTop; }}>{editor.before}</pre><textarea ref={right} aria-label={t("Edit merge result")} value={editor.value} disabled={editor.disabled} maxLength={524288} spellCheck={false} wrap="off" onChange={e => editor.onChange(e.target.value)} onScroll={e => { if (left.current && Math.abs(e.currentTarget.scrollTop - Math.min(left.current.scrollTop, Math.max(0, e.currentTarget.scrollHeight - e.currentTarget.clientHeight))) > 1) left.current.scrollTop = e.currentTarget.scrollTop; }} /></div>;
}
type Mode = 'split' | 'unified';
const LINE_HEIGHT = 24;
function DiffCell({ line, side, action }: { action?:React.ReactNode; line: DiffLine | null; side: 'left' | 'right' }) {
  return <div className={`split-cell ${line?.kind ?? 'empty'}`}>
    <span className="dock-line-number">{line ? side === 'left' ? line.old : line.next : ''}</span>
    <span className="dock-line-sign">{line?.kind === 'add' ? '+' : line?.kind === 'remove' ? '−' : ''}</span>
    <code>{line?.text || ' '}</code>{action}
  </div>;
}
function PatchCode({ file, mode, before, after, hunks }: { hunks?:HunkControls; file: PatchFile; mode: Mode; before: string; after: string }) {
  const region = useRef<HTMLDivElement>(null);
  const left = useRef<HTMLDivElement>(null); const right = useRef<HTMLDivElement>(null); const unified = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState(0); const [height, setHeight] = useState(300);
  const hunkLines=file.lines.filter(line=>line.kind==='hunk');
  const action=(line:DiffLine|null)=>{const index=line?hunkLines.indexOf(line):-1;return index>=0&&hunks&&hunks.headers[index]===line?.text?<button className="hunk-action" disabled={hunks.disabled} aria-label={t(hunks.staged?'Unstage hunk {0}':'Stage hunk {0}',[index+1])} onClick={()=>hunks.onApply(index)}>{t(hunks.staged?'Unstage hunk':'Stage hunk')}</button>:null;};
  const pairs = useMemo(() => splitDiffLines(file.lines), [file.lines]);
  // Keep full-file widths stable when the longest line leaves the virtual window.
  const widths = useMemo(() => {
    const context = document.createElement('canvas').getContext('2d');
    if (context) context.font = '12px "SFMono-Regular", Consolas, monospace';
    const measured = new Map<string, number>();
    const width = (line: DiffLine | null) => {
      const text = line?.text.replace(/\t/g, '    ') ?? '';
      const cached = measured.get(text); if (cached !== undefined) return cached;
      const value = context?.measureText(text).width ?? text.length * 8; measured.set(text, value); return value;
    };
    const result = { left: 82, right: 82 };
    for (const pair of pairs) { result.left = Math.max(result.left, Math.ceil(width(pair.left) + 82)); result.right = Math.max(result.right, Math.ceil(width(pair.right) + 82)); }
    return result;
  }, [pairs]);
  const count = mode === 'split' ? pairs.length : file.lines.length;
  const start = Math.max(0, Math.min(count, Math.floor(scroll / LINE_HEIGHT) - 8));
  const end = Math.min(count, Math.ceil((scroll + height) / LINE_HEIGHT) + 8);
  useLayoutEffect(() => {
    setScroll(0); for (const reference of [left, right, unified]) if (reference.current) { reference.current.scrollTop = 0; reference.current.scrollLeft = 0; }
  }, [file, mode]);
  useEffect(() => {
    const node = region.current; if (!node) return;
    const measure = () => setHeight(mode === 'split' ? Math.max(left.current?.clientHeight ?? 0, right.current?.clientHeight ?? 0) : unified.current?.clientHeight ?? 0);
    const observer = new ResizeObserver(measure); observer.observe(node); measure(); return () => observer.disconnect();
  }, [mode]);
  const synchronize = (node: HTMLDivElement, side: 'left' | 'right') => {
    const other = side === 'left' ? right.current : left.current;
    if (other && Math.abs(other.scrollTop - node.scrollTop) > .5) other.scrollTop = node.scrollTop;
    setScroll(node.scrollTop);
  };
  return <div ref={region} className={`patch-viewport ${mode}`} role="region" aria-label={mode === 'split' ? t("Side-by-side patch") : t("Unified patch")}>
    {mode === 'split' ? <>
      <div className="split-labels"><span>{t("Before")}<code>{before}</code></span><span>{t("After")}<code>{after}</code></span></div>
      <div className="split-panes">{(['left', 'right'] as const).map(side => <div key={side} ref={side === 'left' ? left : right} className={`split-pane-scroll ${side}`} role="region" aria-label={side === 'left' ? t("Before code") : t("After code")} tabIndex={0} onScroll={event => synchronize(event.currentTarget, side)}>
        <div className="split-pane-content" style={{ minWidth: `max(100%, ${widths[side]}px)` }}><div style={{ height: start * LINE_HEIGHT }} />{pairs.slice(start, end).map((pair, i) => <div className="split-code-row" key={start + i} style={{ height: LINE_HEIGHT }}><DiffCell line={side === 'left' ? pair.left : pair.right} side={side} action={side==='right'?action(pair.right):undefined} /></div>)}<div style={{ height: (count - end) * LINE_HEIGHT }} /></div>
      </div>)}</div>
    </> : <div ref={unified} className="dock-code-scroll unified" tabIndex={0} onScroll={event => setScroll(event.currentTarget.scrollTop)}><div className="dock-code-content"><div style={{ height: start * LINE_HEIGHT }} />{file.lines.slice(start, end).map((line, i) => <div key={start + i} className={`dock-unified-row ${line.kind}`} style={{ height: LINE_HEIGHT }}><span className="dock-line-number">{line.old}</span><span className="dock-line-number">{line.next}</span><span className="dock-line-sign">{line.kind === 'add' ? '+' : line.kind === 'remove' ? '−' : ''}</span><code>{line.text || ' '}</code>{action(line)}</div>)}<div style={{ height: (count - end) * LINE_HEIGHT }} /></div></div>}
    {!count && <div className="quiet-state">{t("No text lines in this patch.")}</div>}
  </div>;
}

export function CommitDiffDock({ membershipId, preferredPath, detail = null, source = null, busy, collapsed, onToggle, size, onResize }: { membershipId?: string; preferredPath?: string; detail?: CommitDetail | null; source?: DockPatch | null; busy: boolean; collapsed: boolean; onToggle: () => void; size: number | null; onResize: (height: number) => void }) {
  const [historical, setHistorical] = useState<{key: string; diff: string; truncated: boolean} | null>(null);
  const [patchError, setPatchError] = useState('');
  const diff = source?.diff ?? historical?.diff ?? detail?.diff ?? '';
  const files = useMemo(() => parseDiff(diff), [diff]);
  const [fileIndex, setFileIndex] = useState(0);
  const [mode, setMode] = useState<Mode>(() => { try { return localStorage.getItem('gitnova.diffMode') === 'unified' ? 'unified' : 'split'; } catch { return 'split'; } });
  const [measuredHeight, setMeasuredHeight] = useState(300);
  const dock = useRef<HTMLElement>(null); const drag = useRef<{ y: number; height: number } | null>(null);
  const controlledIndex = source?.selectedPath ? findPatch(files, source.selectedPath) : -1;
  const manifest = !source ? detail?.files ?? [] : [];
  useEffect(()=>{
    if(!preferredPath || !detail)return;
    const index=detail.files.findIndex(file=>file.path.display===preferredPath || file.path.display.startsWith(`${preferredPath}/`));
    if(index>=0)setFileIndex(index);
  },[preferredPath,detail?.commit.oid]);
  const historicalFile = manifest[Math.min(fileIndex, Math.max(0, manifest.length - 1))];
  const patchKey = `${detail?.commit.oid}:${historicalFile?.path.hex}`;
  const needsLoad = Boolean(!source && detail?.truncated && historicalFile && membershipId);
  const patchLoading = needsLoad && historical?.key !== patchKey && !patchError;
  useEffect(() => {
    let cancelled = false; setHistorical(null); setPatchError('');
    if (!needsLoad || !historicalFile || !detail || !membershipId) return;
    api.historicalPatch(membershipId, detail.commit.oid, historicalFile.path).then(value => { if (!cancelled) setHistorical({key: patchKey, ...value}); }).catch(e => { if (!cancelled) setPatchError(errorMessage(e)); });
    return () => { cancelled = true; };
  }, [patchKey, needsLoad, membershipId]);
  const activeIndex = controlledIndex >= 0 ? controlledIndex : Math.min(fileIndex, Math.max(0, files.length - 1));
  const unmatched = Boolean(source?.selectedPath && controlledIndex < 0 && !busy);
  const manifestIndex = historicalFile ? findPatch(files, historicalFile.path) : -1;
  const file = unmatched || patchError || patchLoading ? undefined : !source ? files[manifestIndex] : files[activeIndex];
  useEffect(() => {
    const node = dock.current; if (!node) return;
    const observer = new ResizeObserver(() => setMeasuredHeight(Math.round(node.getBoundingClientRect().height)));
    observer.observe(node); return () => observer.disconnect();
  }, []);
  const changeMode = (next: Mode) => { setMode(next); try { localStorage.setItem('gitnova.diffMode', next); } catch { /* Layout preferences are optional. */ } };
  const resize = (height: number) => {
    const available = dock.current?.parentElement?.clientHeight ?? window.innerHeight - 150;
    onResize(Math.round(Math.max(140, Math.min(Math.max(140, available - 190), height))));
  };
  useEffect(() => {
    const onResize = () => { if (size !== null) resize(size); };
    window.addEventListener('resize', onResize); return () => window.removeEventListener('resize', onResize);
  }, [size, onResize]);
  return <section ref={dock} className={`commit-diff-dock ${collapsed ? 'collapsed' : ''}`} aria-label={source?.label ?? t("Commit diff")} style={size !== null && !collapsed ? { '--dock-height': `${size}px` } as CSSProperties : undefined}>
    {!collapsed && <div className="dock-resize" role="separator" aria-label={t("Resize diff area")} aria-orientation="horizontal" aria-valuemin={140} aria-valuemax={Math.max(140, (dock.current?.parentElement?.clientHeight ?? 700) - 190)} aria-valuenow={measuredHeight} tabIndex={0}
      onKeyDown={event => { if (event.key === 'ArrowUp' || event.key === 'ArrowDown') { event.preventDefault(); resize((dock.current?.clientHeight ?? 300) + (event.key === 'ArrowUp' ? 30 : -30)); } }}
      onPointerDown={event => { drag.current = { y: event.clientY, height: dock.current?.clientHeight ?? 300 }; event.currentTarget.setPointerCapture(event.pointerId); }}
      onPointerMove={event => { if (drag.current) resize(drag.current.height + drag.current.y - event.clientY); }}
      onPointerUp={event => { drag.current = null; event.currentTarget.releasePointerCapture(event.pointerId); }} onLostPointerCapture={() => { drag.current = null; }} />}
    <div className="dock-toolbar"><div className="dock-title"><Icon name="file" size={15} /><strong>{source?.title ?? t("Changes")}</strong>{!busy && <span className="count-badge">{source ? files.length : manifest.length}</span>}<span className="dock-comparison">{t(source?.comparison ?? detail?.comparison ?? '')}</span></div>
      <div className="dock-actions">{!collapsed && !source?.editor && <div className="view-switch" role="group" aria-label={t("Diff layout")}><button aria-pressed={mode === 'split'} onClick={() => changeMode('split')}>{t("Side by side")}</button><button aria-pressed={mode === 'unified'} onClick={() => changeMode('unified')}>{t("Unified")}</button></div>}<button className="icon-button dock-collapse" aria-label={collapsed ? t("Expand diff") : t("Collapse diff")} aria-expanded={!collapsed} onClick={onToggle}><Icon name="chevron" size={17} /></button></div>
    </div>
    <div className="dock-content" hidden={collapsed}>{source?.truncated && <div className="notice">{t("This patch exceeds the preview budget and is incomplete. Open the full diff in Git before approving a commit.")}</div>}{!source && historical?.truncated && <div className="notice">{t("This file’s patch exceeds the preview budget. Some code is not shown.")}</div>}{patchError && <div className="inline-error" role="alert">{patchError}</div>}
      {busy ? <div className="quiet-state" role="status">{t("Reading changes…")}</div> : <div className={`dock-body ${source ? 'single-file' : ''}`}>{!source && <nav className="dock-files" aria-label={t("Changed files")}>{manifest.map((entry, i) => <button key={entry.path.hex} aria-pressed={i === fileIndex} className={i === fileIndex ? 'active' : ''} onClick={() => setFileIndex(i)} title={entry.path.display}><Icon name="file" size={15} /><span className="dock-file-path">{entry.path.display}</span><span className="patch-stats"><span className="added">{entry.additions === null ? t("Binary") : `+${entry.additions}`}</span><span className="removed">{entry.deletions === null ? '' : `−${entry.deletions}`}</span></span></button>)}{!manifest.length && <div className="quiet-state">{t("No changed files in this comparison.")}</div>}</nav>}
        <div className="dock-patch">{patchLoading ? <div className="quiet-state" role="status">{t("Reading file patch…")}</div> : source?.editor ? source.editor.incoming!==undefined?<ConflictResolver key={source.selectedPath?.hex} editor={source.editor}/>:<MergeEditor editor={source.editor} /> : file ? <><div className="dock-file-heading"><Icon name="file" size={15} /><code title={file.name}>{file.name}</code><span className="patch-stats"><span className="added">{!source && historicalFile?.additions === null ? t("Binary") : `+${!source ? historicalFile?.additions ?? file.additions : file.additions}`}</span><span className="removed">{!source && historicalFile?.deletions === null ? '' : `−${!source ? historicalFile?.deletions ?? file.deletions : file.deletions}`}</span></span></div>{file.metadata.length > 0 && <div className="patch-meta">{file.metadata.join(' · ')}</div>}<PatchCode key={file.name} hunks={source?.hunks} file={file} mode={mode} before={source?.before ?? detail?.commit.parents[0]?.slice(0, 8) ?? t("Empty tree")} after={source?.after ?? detail?.commit.oid.slice(0, 8) ?? ''} /></> : <div className="quiet-state"><Icon name="commit" size={24} /><p>{source ? source.emptyMessage ?? (unmatched ? t('The selected path could not be matched to this patch. No other file is shown. Refresh and review again.') : t('Select a file from Working changes to inspect its diff.')) : t("This commit has no text patch against the selected comparison.")}</p></div>}</div>
      </div>}
    </div>
  </section>;
}
