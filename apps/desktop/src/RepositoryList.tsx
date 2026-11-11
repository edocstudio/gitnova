import { useEffect, useMemo, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import type { Catalog, RepoStatus, Workspace } from './contracts';
import { Icon } from './Icons';
import { t } from './i18n';
const PAGE_SIZE = 5;
const searchable = (value: string) => value.normalize('NFC').toLowerCase();

export function RepositoryList({ catalog, workspace, selectedStatus, blocked, onSelect, onPin, initialView, onViewChange, onCatalog, onBusyChange, managementBlock, terminalActive, onEndTerminal }: {
  catalog: Catalog; workspace: Workspace; selectedStatus?: RepoStatus; blocked: boolean;
  initialView?: {query:string;page:number}; onViewChange?: (view:{query:string;page:number})=>void;
  onSelect: (id: string) => void; onPin: (id: string) => void;
  onCatalog:(catalog:Catalog)=>void; onBusyChange:(busy:boolean)=>void;
  managementBlock:(id:string)=>string;
  terminalActive:(id:string)=>boolean; onEndTerminal:(id:string)=>void;
}) {
  const [query, setQuery] = useState(initialView?.query ?? '');
  const [editing,setEditing]=useState<{id:string;name:string;path:string;originalName:string;originalPath:string;remove:boolean}|null>(null);
  const [saving,setSaving]=useState(false);
  const [endReview,setEndReview]=useState(false);
  const dialog=useRef<HTMLDialogElement>(null);
  const managementOpen=Boolean(editing);
  useEffect(()=>{
    if(!managementOpen)return;
    const node=dialog.current,previous=document.activeElement as HTMLElement|null;
    node?.showModal();
    return()=>{node?.close();requestAnimationFrame(()=>{if(previous?.isConnected&&!previous.hasAttribute('disabled'))previous.focus();else document.querySelector<HTMLButtonElement>('.repo-picker')?.focus();});};
  },[managementOpen]);
  const [problem,setProblem]=useState('');
  const mounted=useRef(true);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const openEditor=(entry:{id:string;name:string;path:string})=>{
    if(blocked||saving)return;
    setProblem('');setEndReview(false);setEditing({id:entry.id,name:entry.name,path:entry.path,originalName:entry.name,originalPath:entry.path,remove:false});
  };
  const save=async()=>{
    if(!editing||blocked||saving)return;
    const guard=managementBlock(editing.id);
    if(guard&&(editing.remove||editing.path!==editing.originalPath)){setProblem(guard);return;}
    setSaving(true);setProblem('');onBusyChange(true);
    try{
      const value=await (editing.remove?api.removeMembership(editing.id):api.updateMembership(editing.id,editing.name,editing.path));
      onCatalog(value);
      if(mounted.current){setEditing(null);setStatuses({});}
    }catch(error){if(mounted.current)setProblem(errorMessage(error));}
    finally{if(mounted.current)setSaving(false);onBusyChange(false);}
  };
  const entries = useMemo(() => {
    const repositories = new Map(catalog.repositories.map(repo => [repo.id, repo.commonDir.display]));
    const worktrees = new Map(catalog.worktrees.map(worktree => [worktree.id, worktree.root.display]));
    return workspace.memberships.map(membership => {
      const path = (membership.worktreeId ? worktrees.get(membership.worktreeId) : undefined) ?? repositories.get(membership.repositoryId) ?? '';
      return { ...membership, path, search: searchable(`${membership.name}\n${path}`) };
    }).sort((a, b) => Number(b.pinned) - Number(a.pinned));
  }, [catalog.repositories, catalog.worktrees, workspace.memberships]);
  const [page, setPage] = useState(() => initialView?.page ?? Math.floor(Math.max(0, entries.findIndex(entry => entry.id === workspace.selectedId)) / PAGE_SIZE));
  const viewCallback=useRef(onViewChange);viewCallback.current=onViewChange;
  useEffect(()=>{viewCallback.current?.({query,page});},[query,page]);
  const matches = useMemo(() => { const text = searchable(query.trim()); return entries.filter(entry => entry.search.includes(text)); }, [entries, query]);
  const pageCount = Math.max(1, Math.ceil(matches.length / PAGE_SIZE));
  const currentPage = Math.min(page, pageCount - 1);
  const visible = matches.slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE);
  const visibleKey = visible.map(entry => entry.id).join(',');
  const [statuses, setStatuses] = useState<Record<string, RepoStatus | string>>({});
  useEffect(() => {
    let cancelled = false;
    setStatuses({});
    if (blocked) return;
    const queue = [...visible];
    const worker = async () => {
      while (!cancelled && queue.length) {
        const entry = queue.shift()!;
        if (entry.id === workspace.selectedId && selectedStatus) continue;
        try { const status = await api.status(entry.id); if (!cancelled) setStatuses(old => ({ ...old, [entry.id]: status })); }
        catch (error) { if (!cancelled) setStatuses(old => ({ ...old, [entry.id]: errorMessage(error) })); }
      }
    };
    queueMicrotask(() => { if (!cancelled) void Promise.all([worker(), worker()]); });
    return () => { cancelled = true; };
    // Only the visible membership identities need status reads. Typing a query
    // with the same results and graph polling must not restart these workers.
  }, [visibleKey, blocked]);
  return <div className="repo-list">
    <p className="field-label">{t('Repositories')} <span>{t('Pinned & recent')}</span></p>
    {entries.length > 0 && <div className="repo-filter"><Icon name="search" size={14}/><input aria-label={t('Search repositories')} placeholder={t('Name or local path…')} value={query} maxLength={512} disabled={blocked} onChange={event => { setQuery(event.target.value); setPage(0); }}/>{query && <button className="icon-button" aria-label={t('Clear repository search')} disabled={blocked} onClick={() => { setQuery(''); setPage(0); }}><Icon name="close" size={13}/></button>}</div>}
    <div className="repo-page">{visible.map(entry => {
      const status = entry.id === workspace.selectedId && selectedStatus ? selectedStatus : statuses[entry.id];
      return <div className={`repo-option ${entry.id === workspace.selectedId ? 'active' : ''}`} key={entry.id} onContextMenu={event=>{event.preventDefault();openEditor(entry);}}>
        <button className="repo-option-main" title={entry.path} disabled={blocked} onClick={() => onSelect(entry.id)}><Icon name="folder"/><span><strong>{entry.name}</strong><small>{typeof status === 'string' ? t('Unavailable · {0}', [status]) : status ? `${status.bare ? t('bare') : status.branch ?? t('detached HEAD')}${status.dirty ? t(' · {0} changes', [status.changedEntries]) : t(' · clean')}` : t('Reading status…')}</small><small className="repo-option-path">{entry.path}</small></span>{entry.id === workspace.selectedId && <Icon name="check" size={14}/>}</button>
        <button className={`icon-button pin ${entry.pinned ? 'pinned' : ''}`} aria-label={t(entry.pinned ? 'Unpin {0}' : 'Pin {0}', [entry.name])} aria-pressed={entry.pinned} disabled={blocked} onClick={() => { setPage(0); onPin(entry.id); }}><Icon name="star" size={15}/></button>
        <button className="icon-button repo-manage" aria-label={t('Manage repository {0}',[entry.name])} aria-expanded={editing?.id===entry.id} disabled={blocked} onClick={()=>openEditor(entry)}>⋯</button>
      </div>;
    })}</div>
    {!entries.length ? <p className="hint">{t('No repositories yet. Add one below.')}</p> : !matches.length ? <p className="hint" role="status">{t('No matching repositories. Try another name or local path.')}</p> : <div className="repo-pagination"><span className="hint" role="status">{t('{0}–{1} of {2} repositories', [currentPage * PAGE_SIZE + 1, Math.min((currentPage + 1) * PAGE_SIZE, matches.length), matches.length])}</span>{pageCount > 1 && <nav aria-label={t('Repository pages')}><button aria-label={t('Previous repository page')} disabled={blocked || currentPage === 0} onClick={() => setPage(currentPage - 1)}>←</button><span>{t('Page {0} of {1}', [currentPage + 1, pageCount])}</span><button aria-label={t('Next repository page')} disabled={blocked || currentPage === pageCount - 1} onClick={() => setPage(currentPage + 1)}>→</button></nav>}</div>}
    {editing&&<dialog ref={dialog} className="settings-dialog repo-management-dialog" aria-label={t('Manage repository')} onKeyDown={event=>event.stopPropagation()} onCancel={event=>{event.preventDefault();if(!blocked&&!saving)setEditing(null);}}><form className="repo-entry-editor" aria-label={t('Manage repository')} onSubmit={event=>{event.preventDefault();void save();}}>
      <h2>{t('Manage repository')} · {workspace.name}</h2><p className="repo-management-context"><strong>{editing.originalName}</strong><small>{editing.originalPath}</small></p>
      {editing.remove?<><p>{t('Remove {0} from this workspace?',[editing.originalName])}</p><p className="hint">{t('Files, Git history and other workspaces stay unchanged. You can add this repository again.')}</p></>:<>
        <label>{t('Display name')}<input autoFocus autoComplete="off" value={editing.name} maxLength={80} disabled={blocked||saving} onChange={event=>setEditing({...editing,name:event.target.value})}/></label>
        <p className="hint">{t('Display name applies only to this workspace. The folder is not renamed.')}</p>
        <label>{t('Repository folder')}<input autoComplete="off" value={editing.path} disabled={blocked||saving} onChange={event=>setEditing({...editing,path:event.target.value})}/></label>
        <p className="hint">{t('Choose the repository root if it moved. Updating this entry does not move or copy files.')}</p>
      </>}
      {managementBlock(editing.id)&&<p className="hint">{t(managementBlock(editing.id))}</p>}
      {terminalActive(editing.id)&&<div className="repo-terminal-review">{endReview?<><p>{t('End this shell and its running commands?')}</p><button type="button" className="danger-action" disabled={blocked||saving} onClick={()=>{onEndTerminal(editing.id);setEndReview(false);setProblem('');}}>{t('End session')}</button><button type="button" disabled={blocked||saving} onClick={()=>setEndReview(false)}>{t('Cancel')}</button></>:<button type="button" disabled={blocked||saving} onClick={()=>setEndReview(true)}>{t('End Terminal session…')}</button>}</div>}
      {problem&&<p role="alert" className="error-banner">{t(problem)}</p>}
      {saving&&<p role="status"><Icon name="refresh" className="loading-spin"/>{t('Updating…')}</p>}
      <div className="repo-entry-actions"><button type="button" disabled={blocked||saving} onClick={()=>{setEditing(null);setProblem('');}}>{t('Cancel')}</button>
      {!editing.remove&&<button type="button" disabled={blocked||saving} onClick={()=>{setEditing({...editing,remove:true});setProblem('');}}><Icon name="trash"/>{t('Remove from workspace…')}</button>}
      <button className={editing.remove?'danger-action':'primary'} disabled={blocked||saving||(!editing.remove&&(!editing.name.trim()||!editing.path.trim()))||Boolean(managementBlock(editing.id)&&(editing.remove||editing.path!==editing.originalPath))}>{t(editing.remove?'Remove from workspace':'Save changes')}</button></div>
    </form></dialog>}
  </div>;
}
