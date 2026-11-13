import { t } from './i18n';
import { useMemo, useState } from 'react';
import { parseDiff } from './diff';
import type { DiffLine, PatchFile } from './diff';
import { Icon } from './Icons';
function Code({ lines }: { lines: DiffLine[] }) {
  const [scroll, setScroll] = useState(0); const row = 23;
  const start = Math.max(0, Math.floor(scroll / row) - 6); const end = Math.min(lines.length, start + 34);
  return <div className="code-scroll" style={{ maxHeight: 460 }} onScroll={event => setScroll(event.currentTarget.scrollTop)}>
    <div className="code-content"><div style={{ height: start * row }} />{lines.slice(start, end).map((line, i) => <div key={start + i} className={`code-line ${line.kind}`} style={{ height: row }}><span className="line-number">{line.old}</span><span className="line-number">{line.next}</span><span className="line-sign">{line.kind === 'add' ? '+' : line.kind === 'remove' ? '−' : ''}</span><code>{line.text || ' '}</code></div>)}<div style={{ height: (lines.length - end) * row }} /></div>
  </div>;
}
// Closed sections keep their summary, but do not mount virtual code rows until
// opened. Keep Code mounted after first use so collapsing retains its scroll.
function PatchSection({file, initiallyOpen}: {file: PatchFile; initiallyOpen: boolean}) {
  const [opened, setOpened] = useState(initiallyOpen);
  return <details className="patch-file" open={initiallyOpen} onToggle={event=>{if(event.currentTarget.open)setOpened(true);}}><summary><Icon name="chevron" size={14}/><Icon name="file" size={14}/><span className="patch-name" title={file.name}>{file.name}</span><span className="patch-stats"><span className="added">+{file.additions}</span><span className="removed">−{file.deletions}</span></span></summary>{opened&&<>{file.metadata.length>0&&<div className="patch-meta">{file.metadata.join(' · ')}</div>}<Code lines={file.lines}/></>}</details>;
}
export function DiffView({ diff, truncated = false, label = 'Commit diff' }: { diff: string; truncated?: boolean; label?: string }) {
  const files = useMemo(() => parseDiff(diff), [diff]);
  return <div className="patches" aria-label={label}>{truncated && <div className="notice">{t("This patch exceeds the preview budget and is incomplete. Open the full diff in Git before approving a commit.")}</div>}
    {files.map((file,i)=><PatchSection key={`${i}:${file.name}`} file={file} initiallyOpen={i<3}/>)}
    {!files.length && <div className="quiet-state">{t("No text patch. This may be a binary or empty change.")}</div>}
  </div>;
}
