import { useEffect, useMemo, useState } from 'react';
import { api, errorMessage } from './api';
import type { Catalog, RepoStatus, Workspace } from './contracts';
import { Icon } from './Icons';
import { t } from './i18n';
const PAGE_SIZE = 5;
const searchable = (value: string) => value.normalize('NFC').toLowerCase();

export function RepositoryList({ catalog, workspace, selectedStatus, blocked, onSelect, onPin }: {
  catalog: Catalog; workspace: Workspace; selectedStatus?: RepoStatus; blocked: boolean;
  onSelect: (id: string) => void; onPin: (id: string) => void;
}) {
  const [query, setQuery] = useState('');
  const entries = useMemo(() => {
    const repositories = new Map(catalog.repositories.map(repo => [repo.id, repo.commonDir.display]));
    const worktrees = new Map(catalog.worktrees.map(worktree => [worktree.id, worktree.root.display]));
    return workspace.memberships.map(membership => {
      const path = (membership.worktreeId ? worktrees.get(membership.worktreeId) : undefined) ?? repositories.get(membership.repositoryId) ?? '';
      return { ...membership, path, search: searchable(`${membership.name}\n${path}`) };
    }).sort((a, b) => Number(b.pinned) - Number(a.pinned));
  }, [catalog.repositories, catalog.worktrees, workspace.memberships]);
  const [page, setPage] = useState(() => Math.floor(Math.max(0, entries.findIndex(entry => entry.id === workspace.selectedId)) / PAGE_SIZE));
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
      return <div className={`repo-option ${entry.id === workspace.selectedId ? 'active' : ''}`} key={entry.id}>
        <button className="repo-option-main" title={entry.path} disabled={blocked} onClick={() => onSelect(entry.id)}><Icon name="folder"/><span><strong>{entry.name}</strong><small>{typeof status === 'string' ? t('Unavailable · {0}', [status]) : status ? `${status.bare ? t('bare') : status.branch ?? t('detached HEAD')}${status.dirty ? t(' · {0} changes', [status.changedEntries]) : t(' · clean')}` : t('Reading status…')}</small><small className="repo-option-path">{entry.path}</small></span>{entry.id === workspace.selectedId && <Icon name="check" size={14}/>}</button>
        <button className={`icon-button pin ${entry.pinned ? 'pinned' : ''}`} aria-label={t(entry.pinned ? 'Unpin {0}' : 'Pin {0}', [entry.name])} aria-pressed={entry.pinned} disabled={blocked} onClick={() => { setPage(0); onPin(entry.id); }}><Icon name="star" size={15}/></button>
      </div>;
    })}</div>
    {!entries.length ? <p className="hint">{t('No repositories yet. Add one below.')}</p> : !matches.length ? <p className="hint" role="status">{t('No matching repositories. Try another name or local path.')}</p> : <div className="repo-pagination"><span className="hint" role="status">{t('{0}–{1} of {2} repositories', [currentPage * PAGE_SIZE + 1, Math.min((currentPage + 1) * PAGE_SIZE, matches.length), matches.length])}</span>{pageCount > 1 && <nav aria-label={t('Repository pages')}><button aria-label={t('Previous repository page')} disabled={blocked || currentPage === 0} onClick={() => setPage(currentPage - 1)}>←</button><span>{t('Page {0} of {1}', [currentPage + 1, pageCount])}</span><button aria-label={t('Next repository page')} disabled={blocked || currentPage === pageCount - 1} onClick={() => setPage(currentPage + 1)}>→</button></nav>}</div>}
  </div>;
}
