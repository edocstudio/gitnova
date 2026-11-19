import { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from './api';
import { connections } from './connection-store';
import { t } from './i18n';
import type { HostedRequest, HostingPreview, PromotionOptions, PromotionReview, PublishedPromotion, RemoteBranches } from './contracts';

type Result = { kind: 'form' } | { kind: 'unchanged' } | { kind: 'review'; review: PromotionReview; options: PromotionOptions }
  | { kind: 'published'; published: PublishedPromotion; request?: HostingPreview; created?: HostedRequest }
  | { kind: 'publicationUnknown' };

function PromotionPatch({diff}: {diff: string}) {
  const lines = diff.split('\n');
  return <>
    <pre className="promotion-patch" tabIndex={0} aria-label={t('Promotion diff')}>{lines.slice(0, 1500).map((line, index) => {
      const tone = line.startsWith('@@') ? 'hunk'
        : line.startsWith('+') && !line.startsWith('+++ ') ? 'add'
        : line.startsWith('-') && !line.startsWith('--- ') ? 'remove' : undefined;
      return <span key={index} className={tone}>{line}{'\n'}</span>;
    })}</pre>
    {lines.length > 1500 && <p className="hint">{t('Showing the first 1,500 lines of the diff.')}</p>}
  </>;
}

export function PromotionDialog({membershipId, currentBranch, candidates, initialFile, onClose, onBusyChange}: {
  membershipId: string;
  currentBranch: string;
  candidates: string[];
  initialFile: string;
  onClose: () => void;
  onBusyChange: (value: boolean) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const inFlight = useRef(false);
  const [remotes, setRemotes] = useState<RemoteBranches | null>(null);
  const [remote, setRemote] = useState('');
  const [source, setSource] = useState(currentBranch);
  const [target, setTarget] = useState('');
  const [branch, setBranch] = useState('');
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [files, setFiles] = useState<string[]>(initialFile ? [initialFile] : []);
  const [newFile, setNewFile] = useState('');
  const [allowDeletions, setAllowDeletions] = useState(false);
  const [createRequest, setCreateRequest] = useState(true);
  const [result, setResult] = useState<Result>({kind: 'form'});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    dialog.current?.showModal();
  }, []);
  useEffect(() => {
    let cancelled = false;
    void api.remoteBranches(membershipId).then(value => {
      if (cancelled) return;
      setRemotes(value);
      setRemote(value.remotes.includes('origin') ? 'origin' : value.remotes.length === 1 ? value.remotes[0] : '');
    }).catch(error => { if (!cancelled) setError(errorMessage(error)); });
    return () => { cancelled = true; };
  }, [membershipId]);
  useEffect(() => {
    if (result.kind !== 'form') heading.current?.focus();
  }, [result.kind]);

  const defaults = source && target ? `promote/${source.replaceAll('/', '-')}-to-${target.replaceAll('/', '-')}` : '';
  const branchName = branch.trim() || defaults;
  const titleValue = title.trim() || t('Promote selected files from {0} to {1}', [source, target]);
  const knownBranches = remotes?.branches.filter(item => item.remote === remote).map(item => item.name) ?? [];
  const canPreview = Boolean(remote && source.trim() && target.trim() && source.trim() !== target.trim() && branchName && files.length);
  const toggle = (file: string) => setFiles(current =>
    current.includes(file) ? current.filter(value => value !== file) : current.length < 200 ? [...current, file] : current);
  const addFile = () => {
    if (!newFile || files.length >= 200) return;
    setFiles(current => current.includes(newFile) ? current : [...current, newFile]);
    setNewFile('');
  };
  const edit = () => { setError(''); setResult({kind: 'form'}); };
  const run = async (task: () => Promise<void>, writes = false, onError?: (failure: unknown) => void) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setError('');
    setBusy(true);
    if (writes) onBusyChange(true);
    try { await task(); }
    catch (failure) {
      if (onError) onError(failure);
      else setError(errorMessage(failure));
    } finally {
      inFlight.current = false;
      setBusy(false);
      if (writes) onBusyChange(false);
    }
  };
  const preview = () => {
    if (!canPreview) return;
    const options: PromotionOptions = {
      request: {remote, source: source.trim(), target: target.trim(), branch: branchName, files, allowDeletions},
      title: titleValue, body, createRequest,
      profiles: connections().filter(connection => connection.enabled).map(({provider, origin}) => ({provider, origin})),
    };
    void run(async () => {
      const review = await api.previewPromotion(membershipId, options);
      setResult({kind: 'review', review, options});
    }, false, failure => {
      if (failure && typeof failure === 'object' && 'code' in failure && failure.code === 'promotion_no_changes') {
        setResult({kind: 'unchanged'});
      } else setError(errorMessage(failure));
    });
  };
  const publish = (review: PromotionReview, options: PromotionOptions) => void run(async () => {
    const published = await api.publishPromotion(membershipId, options, review.token);
    setResult({kind: 'published', published});
  }, true, failure => {
    if (failure && typeof failure === 'object' && 'code' in failure && failure.code === 'operation_unknown') {
      setResult({kind: 'publicationUnknown'});
    } else {
      setResult({kind: 'form'});
    }
    setError(errorMessage(failure));
  });
  const reviewRequest = (published: PublishedPromotion) => void run(async () => {
    setResult({kind: 'published', published});
    const request = await api.reviewPromotionRequest(membershipId, published);
    setResult({kind: 'published', published, request});
  });
  const create = (published: PublishedPromotion, request: HostingPreview) => void run(async () => {
    setResult({kind: 'published', published});
    const created = await api.createPromotionRequest(membershipId, published, request.token);
    setResult({kind: 'published', published, created});
  }, true);
  const close = () => {
    if (inFlight.current) return;
    dialog.current?.close();
    onClose();
  };
  const requestLabel = (provider: string) => t(provider === 'gitlab' ? 'Create merge request' : 'Create pull request');
  const headingLabel = result.kind === 'published' ? t('Review branch published') : t('Promote selected files');

  return <dialog ref={dialog} className="settings-dialog promotion-dialog" aria-labelledby="promotion-title"
    onCancel={event => { event.preventDefault(); close(); }}>
    <header className="settings-header"><h2 id="promotion-title" ref={heading} tabIndex={-1}>{headingLabel}</h2></header>
    <div className="promotion-scroll" aria-busy={busy}>
      {result.kind === 'form' && <form id="promotion-form" onSubmit={event => {event.preventDefault(); preview();}}>
        <p>{t('Copy whole files from a remote branch into a new review branch. Your current checkout and staging stay in place.')}</p>
        <fieldset disabled={busy}>
          <div className="promotion-grid">
            <label><span id="promotion-remote-label">{t('Remote')}</span><select aria-labelledby="promotion-remote-label" value={remote} onChange={event => {setRemote(event.target.value);setTarget('');}} required>
              <option value="">{t('Choose a remote')}</option>{remotes?.remotes.map(name => <option key={name}>{name}</option>)}
            </select></label>
            <label><span id="promotion-result-label">{t('After publishing')}</span><select aria-labelledby="promotion-result-label" value={createRequest ? 'request' : 'branch'} onChange={event=>setCreateRequest(event.target.value === 'request')}>
              <option value="request">{t('Review a pull or merge request')}</option><option value="branch">{t('Publish branch only')}</option>
            </select></label>
            <label>{t('Source branch')}<input list="promotion-branches" value={source} required maxLength={256} onChange={event=>setSource(event.target.value)} placeholder="main" /></label>
            <label>{t('Target branch')}<input list="promotion-branches" value={target} required maxLength={256} onChange={event=>setTarget(event.target.value)} placeholder="release" /></label>
            <datalist id="promotion-branches">{knownBranches.map(name=><option key={name} value={name}/>)}</datalist>
            <label>{t('New review branch')}<input value={branch} maxLength={256} placeholder={defaults || 'promote/selected-files'} onChange={event=>setBranch(event.target.value)} /></label>
            <label>{t('Title')}<input value={title} maxLength={1024} placeholder={titleValue} onChange={event=>setTitle(event.target.value)} /></label>
          </div>
          <details className="promotion-options"><summary>{t('More options')}</summary>
            {createRequest && <label>{t('Description')}<textarea value={body} maxLength={65536} onChange={event=>setBody(event.target.value)} /></label>}
            <label className="promotion-check"><input type="checkbox" checked={allowDeletions} onChange={event=>setAllowDeletions(event.target.checked)} />{t('Allow selected files missing from source to be removed')}</label>
          </details>
          <h3>{t('Files to promote')} <small>{files.length}/200</small></h3>
          {candidates.length > 0 && <div className="promotion-files">{candidates.map(file => <label className="promotion-check" key={file} title={file}>
            <input type="checkbox" checked={files.includes(file)} disabled={files.length >= 200 && !files.includes(file)} onChange={()=>toggle(file)} /><code>{file}</code>
          </label>)}</div>}
          {files.filter(file=>!candidates.includes(file)).map(file=><div className="promotion-added" key={file}><code>{file}</code>
            <button type="button" aria-label={t('Remove {0}', [file])} onClick={()=>toggle(file)}>{t('Remove')}</button></div>)}
          <div className="promotion-add"><input aria-label={t('Repository-relative file path')} placeholder={t('Repository-relative file path')} value={newFile} maxLength={4096} onChange={event=>setNewFile(event.target.value)}
            onKeyDown={event=>{if(event.key === 'Enter' && newFile){event.preventDefault();addFile();}}} />
            <button type="button" disabled={!newFile || files.length >= 200} onClick={addFile}>{t('Add file')}</button></div>
          {!canPreview && <p className="hint">{t('Choose a remote, different source and target branches, and at least one file.')}</p>}
        </fieldset>
      </form>}
      {result.kind === 'unchanged' && <p className="notice" role="status">{t('Selected files already match the destination; no request is needed.')}</p>}
      {result.kind === 'review' && <section>
        <p className="promotion-route"><strong>{result.options.request.source}</strong> <code>{result.review.sourceOid.slice(0,8)}</code> → <strong>{result.options.request.target}</strong> <code>{result.review.targetOid.slice(0,8)}</code></p>
        <dl className="promotion-facts">
          <dt>{t('Remote')}</dt><dd>{result.options.request.remote} · {result.review.destination}</dd>
          <dt>{t('New review branch')}</dt><dd><code>{result.options.request.branch}</code></dd>
          <dt>{t('Title')}</dt><dd>{result.options.title}</dd>
          <dt>{t('Committer')}</dt><dd>{result.review.identity.name} &lt;{result.review.identity.email}&gt;</dd>
          <dt>{t('After publishing')}</dt><dd>{result.review.hosting ? `${result.review.hosting.origin} · ${result.review.hosting.account}` : t(result.options.createRequest ? 'Review a pull or merge request' : 'Publish branch only')}</dd>
        </dl>
        <p className="hint">{t('Selected files replace the target versions. The review branch starts from the target branch.')}</p>
        <ul className="promotion-summary">{result.review.files.map(({path,action})=><li key={path}><span>{t(action === 'delete' ? 'Delete file' : action === 'add' ? 'Add file' : 'Replace file')}</span><code>{path}</code></li>)}</ul>
        {result.review.patch.truncated && <p className="notice">{t('The diff is partial. The file list shows the full scope.')}</p>}
        <PromotionPatch diff={result.review.patch.diff} />
        {result.review.hostingError && <div className="notice" role="status">
          <p>{t(result.review.hostingError)}</p>
          <button type="button" disabled={busy} onClick={() => {setCreateRequest(false);edit();}}>{t('Continue with branch only')}</button>
        </div>}
      </section>}
      {result.kind === 'published' && <section>
        <p className="promotion-route"><strong>{result.published.branch}</strong> <code>{result.published.oid.slice(0,8)}</code> → {result.published.target}</p>
        <details className="promotion-options"><summary>{t('Recovery repository')}</summary><code>{result.published.recovery.display}</code></details>
        {result.request && <dl className="promotion-facts">
          <dt>{t('Title')}</dt><dd>{result.request.request.title}</dd>
          {result.request.request.body && <><dt>{t('Description')}</dt><dd>{result.request.request.body}</dd></>}
          <dt>{t('Destination')}</dt><dd>{result.request.context.project} · {result.request.request.targetBranch}</dd>
        </dl>}
        {result.request?.blockedReason && <p className="notice">{t(result.request.blockedReason)}</p>}
        {(result.created || result.request?.existing) && <p role="status">{t(result.created ? 'Request created' : 'Existing request')} · #{(result.created ?? result.request?.existing)?.number}</p>}
      </section>}
      {busy && <p className="loading-status" role="status">{t('Working with the selected remote…')}</p>}
      {error && <p className="inline-error" role="alert">{t(error)}</p>}
    </div>
    <footer className="promotion-footer">
      <button disabled={busy} onClick={close}>{t(result.kind === 'form' ? 'Cancel' : 'Close')}</button>
      {(result.kind === 'review' || result.kind === 'unchanged') && <button disabled={busy} onClick={edit}>{t('Edit selection')}</button>}
      {result.kind === 'form' && <button type="submit" form="promotion-form" className="primary" disabled={busy || !canPreview}>{t('Preview promotion')}</button>}
      {result.kind === 'review' && <button className="primary" disabled={busy || Boolean(result.review.hostingError)} onClick={()=>publish(result.review,result.options)}>{t('Publish review branch')}</button>}
      {result.kind === 'published' && (() => {
        const opened = result.created ?? result.request?.existing;
        if (opened) return <button className="primary" disabled={busy} onClick={()=>void run(()=>api.openCiUrl(opened.url))}>{t('Open request')}</button>;
        if (result.request && !result.request.blockedReason) {
          const request = result.request;
          return <button className="primary" disabled={busy} onClick={()=>create(result.published,request)}>{requestLabel(request.context.provider)}</button>;
        }
        if (result.published.hosting) return <button className="primary" disabled={busy} onClick={()=>reviewRequest(result.published)}>{t('Review request')}</button>;
        return null;
      })()}
    </footer>
  </dialog>;
}
