import { t, dateLocale } from './i18n';
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import type { Catalog, InvestigationPage, CommitSearchPage, CommitSearchRequest, SearchCursor, SearchLocation, Workspace } from './contracts';
import { Icon } from './Icons';
export function CommitSearch({ catalog, workspace, currentId, disabled, open, onOpenChange, onNavigate }: {
  catalog: Catalog; workspace: Workspace; currentId: string; disabled: boolean; open: boolean; onOpenChange: (value: boolean) => void;
  onNavigate: (location: SearchLocation, oid: string, path?: string) => Promise<void>;
}) {
  const root = useRef<HTMLDivElement>(null); const trigger = useRef<HTMLButtonElement>(null); const input = useRef<HTMLInputElement>(null);
  const sequence = useRef(0); const opening = useRef(false); const run = useRef<(cursor: SearchCursor | null) => void>(() => {});
  const [query, setQuery] = useState(''); const [scope, setScope] = useState<CommitSearchRequest['scope']>('current'); const [selected, setSelected] = useState<string[]>([currentId]);
  const [page, setPage] = useState<CommitSearchPage | null>(null); const [offset, setOffset] = useState(0); const [loading, setLoading] = useState(false); const [navigating, setNavigating] = useState(false); const [error, setError] = useState('');
  const [mode,setMode] = useState<'metadata'|'history'|'compare'>('metadata');
  const [path,setPath] = useState(''); const [source,setSource] = useState('develop'); const [target,setTarget] = useState('release');
  const [investigation,setInvestigation] = useState<InvestigationPage|null>(null);
  const [position, setPosition] = useState({ left: 12, top: 64, width: 640 });
  const count = scope === 'workspace' ? workspace.memberships.length : scope === 'selected' ? selected.length : 1;
  const valid = (mode==='compare' ? Boolean(source.trim() && target.trim()) : mode==='history' ? Boolean(path.trim() || query.trim()) : Boolean(query.trim())) && new TextEncoder().encode(query.trim()).length <= 512 && count > 0 && count <= 32;
  const close = useCallback((restoreFocus = true) => { if (opening.current) return; sequence.current++; onOpenChange(false); if (restoreFocus) requestAnimationFrame(() => trigger.current?.focus()); }, [onOpenChange]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); if (!disabled && !opening.current) open ? close() : onOpenChange(true); }
      if (open && event.key === 'Escape' && !opening.current) { event.preventDefault(); event.stopImmediatePropagation(); close(); }
    };
    const outside = (event: PointerEvent) => { if (open && !root.current?.contains(event.target as Node)) close(false); };
    document.addEventListener('keydown', key, true); document.addEventListener('pointerdown', outside);
    return () => { document.removeEventListener('keydown', key, true); document.removeEventListener('pointerdown', outside); };
  }, [open, disabled, onOpenChange, close]);
  useLayoutEffect(() => {
    if (!open) return;
    const fit = () => { const bounds = trigger.current?.getBoundingClientRect(); const width = Math.min(640, window.innerWidth - 24); setPosition({ left: Math.max(12, Math.min(bounds?.left ?? 12, window.innerWidth - width - 12)), top: Math.min(bounds?.bottom ?? 52, 70) + 8, width }); };
    fit(); input.current?.focus(); window.addEventListener('resize', fit); return () => window.removeEventListener('resize', fit);
  }, [open]);
  run.current = cursor => {
    if (!open || !valid || disabled || opening.current) return;
    const version = ++sequence.current; setLoading(true); setError('');
    const request: CommitSearchRequest = { workspaceId: workspace.id, currentMembershipId: currentId, scope, selectedIds: [...selected].sort(), query: query.trim(), cursor };
    const read = async () => {
      try {
        if(mode==='metadata') {
          const result = await api.searchCommits(request); if(version===sequence.current){setPage(result);setOffset(cursor?.offset??0);setLoading(false);}
        } else {
          const result=await api.investigateHistory({selection:request,mode,path:path.trim(),needle:query.trim(),source:source.trim(),target:target.trim()});
          if(version===sequence.current){setInvestigation(result);setPage({hits:result.hits,coverage:[],partial:result.partial,totalMatches:result.hits.length,next:null,token:''});setOffset(0);setLoading(false);}
        }
      } catch (e) {
        if (version !== sequence.current) return;
        if (errorMessage(e).includes('Search index is busy')) { window.setTimeout(() => { if (version === sequence.current) void read(); }, 600); return; }
        setError(errorMessage(e)); setLoading(false); setPage(null);
      }
    };
    void read();
  };
  useEffect(() => {
    sequence.current++; setPage(null); setInvestigation(null); setError(''); setOffset(0); setLoading(false);
    if (!open || !valid || mode!=='metadata') return;
    setLoading(true); const timer = window.setTimeout(() => run.current(null), 400);
    return () => { window.clearTimeout(timer); sequence.current++; };
  }, [open, query, scope, selected, currentId, workspace.id, valid, mode, path, source, target]);
  const navigate = async (location: SearchLocation, oid: string) => {
    if (opening.current || disabled) return; opening.current = true; setNavigating(true); setError(''); sequence.current++;
    try { await onNavigate(location, oid, mode==='history'?path.trim()||undefined:undefined); onOpenChange(false); } catch (e) { setError(errorMessage(e)); }
    finally { opening.current = false; setNavigating(false); setLoading(false); }
  };
  const rootPath = (id: string) => {
    const m = workspace.memberships.find(item => item.id === id);
    return m?.worktreeId ? catalog.worktrees.find(w => w.id === m.worktreeId)?.root.display : catalog.repositories.find(r => r.id === m?.repositoryId)?.commonDir.display;
  };
  return <div ref={root} className="commit-search-root"><button ref={trigger} className="commit-search-trigger" aria-label={t("Search commits…")} disabled={disabled} aria-expanded={open} aria-controls="commit-search" onClick={() => onOpenChange(!open)}><Icon name="search" /><span>{t("Search commits…")}</span><kbd>{t("⌘ / Ctrl K")}</kbd></button>
    {open && <section id="commit-search" className="commit-search-popover" role="region" aria-label={t("Commit search")} style={position}>
      <form onSubmit={event => { event.preventDefault(); run.current(null); }}>
        <label className="search-scope">{t("Search mode")}<select aria-label={t("Search mode")} value={mode} disabled={navigating} onChange={e=>setMode(e.target.value as typeof mode)}><option value="metadata">{t("Commit metadata")}</option><option value="history">{t("File / code history")}</option><option value="compare">{t("Compare branches across repositories")}</option></select></label>
        {mode==='history' && <label className="investigation-field">{t("Literal repository path (optional)")}<input aria-label={t("Literal repository path (optional)")} value={path} maxLength={1024} disabled={navigating} onChange={e=>setPath(e.target.value)} placeholder="src/example.ts" /></label>}
        {mode==='compare' ? <div className="investigation-refs"><label>{t("Source branch")}<input aria-label={t("Source branch")} value={source} maxLength={256} disabled={navigating} onChange={e=>setSource(e.target.value)}/></label><span>→</span><label>{t("Target branch")}<input aria-label={t("Target branch")} value={target} maxLength={256} disabled={navigating} onChange={e=>setTarget(e.target.value)}/></label><button className="primary" disabled={!valid || disabled || navigating || loading}>{t("Search")}</button></div> : <div className="search-query-row"><Icon name="search" /><input ref={input} aria-label={mode==='history'?t("Literal code phrase (optional)"):t("Search commit text, author or SHA")} value={query} maxLength={128} spellCheck={false} autoComplete="off" disabled={navigating} onChange={event => setQuery(event.target.value)} onKeyDown={event => { if (event.key === 'ArrowDown') { event.preventDefault(); root.current?.querySelector<HTMLButtonElement>('.commit-search-result')?.focus(); } }} placeholder={mode==='history'?t("Literal code phrase (optional)"):t("Message, author, email or commit SHA…")} /><button className="primary" disabled={!valid || disabled || navigating || loading}>{t("Search")}</button></div>}
        <label className="search-scope">{t("Scope")}<select aria-label={t("Commit search scope")} value={scope} disabled={navigating} onChange={event => setScope(event.target.value as CommitSearchRequest['scope'])}><option value="current">{t("Current repository")}</option><option value="selected">{t("Selected repositories")}</option><option value="workspace">{t("Entire workspace")}</option></select><span>{count} {count === 1 ? t("location") : t("locations")}</span></label>
        {scope === 'selected' && <div className="search-repository-choices" aria-label={t("Repositories to search")}>{workspace.memberships.map(m => <label key={m.id} title={rootPath(m.id)}><input type="checkbox" checked={selected.includes(m.id)} disabled={navigating} onChange={event => setSelected(old => event.target.checked ? [...old, m.id] : old.filter(id => id !== m.id))} /><span><strong>{m.name}</strong><small>{rootPath(m.id)}</small></span></label>)}</div>}
        <p className="hint">{mode==='metadata'?t("All search words must match. Results open history and diff without checkout."):mode==='history'?t("Searches up to 2,000 recent local commits per location for a literal path or changes in a phrase occurrence count. Merge changes use the first parent. Renames are not followed."):t("Source-only commits and exact tip differences from local refs. This does not certify release readiness. Use full refs/remotes names for cached remote branches.")}</p>
      </form>
      {count === 0 && <p className="notice">{t("Select at least one repository.")}</p>}{count > 32 && <p className="notice">{t("Choose up to 32 repository/worktree locations for one search.")}</p>}
      {error && <div className="inline-error" role="alert">{t(error)}</div>}
      {(loading || navigating) && <p className="search-progress" role="status">{navigating ? t("Opening matching history…") : mode==='metadata'?t("Searching local history and updating shared commit metadata…"):t("Reading bounded local history evidence…")}</p>}
      {investigation && <div className="investigation-evidence" aria-label={t("Repository comparison evidence")}>{investigation.evidence.map(e=><div key={e.location.membershipId}><strong>{e.location.name}</strong>{e.issue?<p className="notice">{t(e.issue)}</p>:mode==='compare'?<p>{source} → {target} · {t("{0} source-only · {1} target-only · {2} files differ",[e.sourceOnly??0,e.targetOnly??0,e.changedFiles??0])}<br/><code>{e.sourceOid?.slice(0,10)} → {e.targetOid?.slice(0,10)}</code></p>:<p>{t("{0} recent commits considered",[e.scanned])}</p>}{e.dirty&&<p className="notice">{t("Uncommitted work present; comparison uses committed history.")}</p>}{(e.limited||e.shallow)&&<p className="notice">{e.shallow?t("Shallow history: evidence may be incomplete."):t("Bounded results: narrow the scope to investigate further.")}</p>}</div>)}</div>}
      {page?.partial && <div className="notice search-coverage" role="status"><strong>{t("Results are incomplete.")}</strong>{page.coverage.filter(c => c.limited).map(c => <p key={c.repositoryId}>{c.membershipIds.map(id => workspace.memberships.find(m => m.id === id)?.name ?? 'Repository').join(', ')} · {c.indexed} {t("of")} {c.scanned} {t("scanned commits indexed.")} {c.issue}</p>)}</div>}
      <div className="commit-search-results" aria-label={t("Commit search results")} aria-busy={loading || navigating} onKeyDown={event => {
        if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
        const buttons = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')]; const index = buttons.indexOf(document.activeElement as HTMLButtonElement); if (!buttons.length) return;
        event.preventDefault(); buttons[event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length].focus();
      }}>
        {page?.hits.map(hit => {
          const target = hit.locations.find(l => l.membershipId === currentId) ?? hit.locations[0];
          return <button key={`${target.repositoryId}:${hit.commit.oid}`} className="commit-search-result" disabled={loading || navigating || disabled} onClick={() => void navigate(target, hit.commit.oid)} title={`${target.path.display}\n${hit.commit.oid}`}><span className="search-result-context"><span><Icon name="folder" size={13} />{target.name}{hit.locations.length > 1 ? t(" · {0} locations", [hit.locations.length]) : ''}</span><code>{hit.commit.oid.slice(0, 10)}</code></span><strong>{hit.commit.subject || t("(no commit message)")}</strong><span className="search-result-excerpt">{hit.excerpt}</span><small>{hit.commit.author} · {new Date(hit.commit.timestamp * 1000).toLocaleDateString(dateLocale())} · {target.path.display}</small></button>;
        })}
        {page && !page.hits.length && !loading && <p className="quiet-state">{mode!=='metadata'?t("No matching commits in the inspected history."):page.partial ? t("No matches in the indexed portion of local history.") : t("No matching commits in this scope.")}</p>}
        {!page && !loading && !error && <p className="quiet-state">{t("Search the history of your project. Select a scope above.")}</p>}
      </div>
      {investigation && <p className="hint">{t("{0} commits shown. Select a result to inspect its historical files and diff.",[investigation.hits.length])}</p>}
      {page && mode==='metadata' && <footer className="commit-search-footer"><span>{page.totalMatches ? t("{0}–{1} of {2}", [offset + 1, offset + page.hits.length, page.totalMatches]) : t("0 matches")}{page.partial ? t(" · partial coverage") : ''}</span><button disabled={!offset || loading || navigating} onClick={() => run.current(offset === 100 ? null : { offset: offset - 100, token: page.token })}>{t("Previous")}</button><button disabled={!page.next || loading || navigating} onClick={() => run.current(page.next)}>{t("Next")}</button></footer>}
    </section>}
  </div>;
}
