import {useEffect, useRef, useState} from 'react';
import {api, errorMessage} from './api';
import type {Catalog, InitPreview} from './contracts';
import {t} from './i18n';
export function InitControls({workspaceId, defaultPath, disabled, onBusyChange, onCreated}: {workspaceId: string; defaultPath: string; disabled: boolean; onBusyChange: (busy: boolean) => void; onCreated: (catalog: Catalog) => void}) {
  const [destination, setDestination] = useState('');
  const [branch, setBranch] = useState('main');
  const [preview, setPreview] = useState<InitPreview|null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const flight = useRef(false);
  useEffect(() => {setDestination(defaultPath); setPreview(null); setError('');}, [defaultPath]);
  const run = async (create: boolean) => {
    if (disabled || flight.current) return;
    flight.current = true; setBusy(true); setError(''); onBusyChange(true);
    try {
      if (create && preview) onCreated(await api.initRepository(preview.request, preview.token));
      else setPreview(await api.previewInit({workspaceId, destination, branch}));
    } catch (e) { setError(errorMessage(e)); setPreview(null); }
    finally {flight.current = false; setBusy(false); onBusyChange(false);}
  };
  return <details className="init-controls">
    <summary onClick={e => {if (busy) e.preventDefault();}}>{t('Initialize repository…')}</summary>
    <form onSubmit={e => {e.preventDefault(); void run(false);}}>
      <label htmlFor="init-destination">{t('Repository folder')}</label>
      <input id="init-destination" value={destination} maxLength={8192} placeholder={t('/absolute/path/to/repository')} disabled={disabled || busy} onChange={e => {setDestination(e.target.value); setPreview(null); setError('');}} required/>
      <label htmlFor="init-branch">{t('Initial branch')}</label>
      <input id="init-branch" value={branch} maxLength={255} disabled={disabled || busy} onChange={e => {setBranch(e.target.value); setPreview(null); setError('');}} required/>
      <p className="hint">{t('Choose a new folder or an existing project folder. Its parent must exist. Existing files stay unchanged; nothing is staged or committed.')}</p>
      {!preview && <button className="primary" disabled={disabled || busy || !destination || !branch}>{busy ? t('Inspecting repository folder…') : t('Review initialization')}</button>}
    </form>
    {preview && <div className="clone-preview"><strong>{t(preview.existingFolder ? 'Initialize existing folder' : 'Create repository folder')}</strong><dl><dt>{t('Destination')}</dt><dd>{preview.destination.display}</dd><dt>{t('Initial branch')}</dt><dd>{preview.request.branch}</dd></dl><p className="hint">{t('Creates local Git metadata only. No remote, network request or initial commit. Add a remote later through Push.')}</p><div className="graph-action-footer"><button disabled={busy} onClick={() => setPreview(null)}>{t('Back')}</button><button className="primary" disabled={disabled || busy} onClick={() => void run(true)}>{busy ? t('Initializing repository…') : t('Initialize & open graph')}</button></div></div>}
    {busy && <progress aria-label={t('Initializing repository…')}/>}
    {error && <p className="inline-error" role="alert">{t(error)}</p>}
  </details>;
}
