import { CommitLink } from './CommitLink';
import { t, dateLocale } from './i18n';
import { useEffect, useState } from 'react';
import type { CommitDetail, GitRef } from './contracts';
import { Icon } from './Icons';
export function CommitInspector({ membershipId, detail, refs, busy, onClose, onShowChanges }: { membershipId:string; detail: CommitDetail | null; refs: GitRef[]; busy: boolean; onClose: () => void; onShowChanges: () => void }) {
  const files = detail?.files ?? [];
  const additions = files.reduce((sum, file) => sum + (file.additions ?? 0), 0); const deletions = files.reduce((sum, file) => sum + (file.deletions ?? 0), 0);
  const body = detail?.message.split('\n').slice(1).join('\n').trim();
  const [copyState, setCopyState] = useState<'ready' | 'copied' | 'unavailable'>('ready');
  useEffect(() => setCopyState('ready'), [detail?.commit.oid]);
  const copy = async () => { if (!detail) return; try { await navigator.clipboard.writeText(detail.commit.oid); setCopyState('copied'); } catch { setCopyState('unavailable'); } };
  return <aside className="detail commit-inspector" aria-label={t("Selected commit")}>
    <div className="detail-heading"><div><Icon name="commit" size={16} /><span>{t("Selected commit")}</span></div><button className="icon-button" aria-label={t("Close commit details")} onClick={onClose}><Icon name="close" /></button></div>
    {busy ? <div className="quiet-state" role="status">{t("Reading commit…")}</div> : detail && <div className="detail-scroll">
      <div className="commit-summary"><div className="commit-identity"><span className="commit-type">{detail.commit.parents.length > 1 ? t("Merge commit") : t("Commit")}</span><code>{detail.commit.oid.slice(0, 8)}</code></div>
        <h2>{detail.commit.subject || t("(no commit message)")}</h2><div className="author-block"><span className="avatar" aria-hidden="true">{detail.commit.author.split(/\s+/).map(s => s[0]).slice(0, 2).join('')}</span><div><strong>{detail.commit.author}</strong><span>{new Date(detail.commit.timestamp * 1000).toLocaleString(dateLocale(), { dateStyle: 'medium', timeStyle: 'short' })}</span></div></div>
        {refs.length > 0 && <div className="inspector-refs">{refs.map(ref => <span className="ref" key={ref.name} title={ref.name}><Icon name="branch" size={12} />{ref.name.replace(/^refs\/(heads|remotes|tags)\//, '')}</span>)}</div>}
        {body && <pre className="message">{body}</pre>}
        <button className="commit-change-summary" aria-label={t("View {0} changed files", [files.length])} onClick={onShowChanges}><Icon name="file" size={17} /><span>{files.length} {files.length === 1 ? t("file changed") : t("files changed")}</span><span className="added">+{additions}</span><span className="removed">−{deletions}</span><Icon name="chevron" size={15} /></button>
        {detail.truncated && <p className="hint">{t("Full file list and line counts. Select a file to load its patch.")}</p>}
        <button className="copy-sha" onClick={() => void copy()}><Icon name={copyState === 'copied' ? 'check' : 'copy'} size={15} />{copyState === 'copied' ? t("SHA copied") : t("Copy SHA")}</button>
        <CommitLink key={`link:${membershipId}:${detail.commit.oid}`} membershipId={membershipId} oid={detail.commit.oid}/>
        {copyState === 'unavailable' && <p className="hint" role="status">{t("Clipboard unavailable. Select the full SHA below to copy it.")}</p>}
        <dl className="revision-facts"><div><dt>{t("Commit")}</dt><dd><code className="full-sha">{detail.commit.oid}</code></dd></div><div><dt>{t("Parents")}</dt><dd>{detail.commit.parents.length ? detail.commit.parents.map(parent => <code title={parent} key={parent}>{parent.slice(0, 8)}</code>) : t("Root commit")}</dd></div><div><dt>{t("Comparison")}</dt><dd>{t(detail.comparison)}</dd></div><div><dt>{t("Author")}</dt><dd title={detail.commit.email}>{detail.commit.email}</dd></div></dl>
      </div>
    </div>}
  </aside>;
}
