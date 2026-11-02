import {useEffect,useState} from 'react';
import {api,errorMessage} from './api';
import type {NativePath,WorkingFileText} from './contracts';
import {t} from './i18n';

export type FileDraft={preview:WorkingFileText;value:string};
export type FileDrafts=Map<string,FileDraft>;
export function WorkingFileEditor({membershipId,path,drafts,blocked,onBusyChange,onSaved,onClose}:{membershipId:string;path:NativePath;drafts:FileDrafts;blocked:boolean;onBusyChange:(busy:boolean)=>void;onSaved:()=>Promise<void>;onClose:()=>void}) {
  const key=JSON.stringify([membershipId,path.encoding,path.hex]);
  const [draft,setDraft]=useState<FileDraft|null>(()=>drafts.get(key)??null);
  const [error,setError]=useState('');const [loading,setLoading]=useState(false);const [saving,setSaving]=useState(false);const [stale,setStale]=useState(false);
  useEffect(()=>()=>{const current=drafts.get(key);if(current?.value===current?.preview.text)drafts.delete(key);},[key,drafts]);
  useEffect(()=>{
    if(drafts.has(key))return;
    let cancelled=false;setLoading(true);
    if(drafts.size>=4){setError('Four file drafts are already open. Save or discard one before editing another file.');setLoading(false);return;}
    api.workingFileText(membershipId,path).then(preview=>{if(!cancelled){const value={preview,value:preview.text};drafts.set(key,value);setDraft(value);}}).catch(e=>{if(!cancelled)setError(errorMessage(e));}).finally(()=>{if(!cancelled)setLoading(false);});
    return()=>{cancelled=true;};
  },[key,membershipId,drafts]);
  const update=(value:FileDraft)=>{drafts.set(key,value);setDraft(value);};
  const close=()=>{if(draft?.value===draft?.preview.text)drafts.delete(key);onClose();};
  const save=async()=>{
    if(!draft||saving||blocked)return;setSaving(true);onBusyChange(true);setError('');
    try{await api.saveWorkingFileText(membershipId,path,draft.value,draft.preview.token);drafts.delete(key);setDraft(null);await onSaved();onClose();}
    catch(e){setError(errorMessage(e));if(e&&typeof e==='object'&&'code' in e&&e.code==='stale_preview')setStale(true);}
    finally{setSaving(false);onBusyChange(false);}
  };
  const reviewCurrent=async()=>{
    setLoading(true);setError('');try{const preview=await api.workingFileText(membershipId,path);if(draft)update({...draft,preview});setStale(false);}catch(e){setError(errorMessage(e));}finally{setLoading(false);}
  };
  return <div className="working-file-editor" role="region" aria-label={t('Edit current working file')}>
    <div className="dock-file-heading"><code>{path.display}</code><button disabled={saving} onClick={close}>{t('Back to diff')}</button></div>
    <p>{t('Editing the current working file. Save changes the file only; stage and commit it afterward. History and staging stay unchanged.')}</p>
    {loading&&<div className="loading-status" role="status">{t('Reading current file…')}</div>}
    {error&&<div className="inline-error" role="alert">{t(error)}</div>}
    {draft&&<><div className="working-editor-columns"><div><strong>{t('Current file · read only')}</strong><pre tabIndex={0}>{draft.preview.text}</pre></div><div><strong>{t('Your edits')}</strong><textarea aria-label={t('Edit current file content')} spellCheck={false} wrap="off" maxLength={524288} disabled={saving||loading} value={draft.value} onChange={e=>{if(new TextEncoder().encode(e.target.value).length>524288){setError('The editor supports text up to 512 KiB.');return;}update({...draft,value:e.target.value});}}/></div></div>
      <div className="working-editor-actions"><button disabled={saving||loading} onClick={()=>{drafts.delete(key);onClose();}}>{t('Discard editor draft')}</button>{stale&&<button disabled={saving||loading||blocked} onClick={()=>void reviewCurrent()}>{t('Review updated file and keep my draft')}</button>}<button className="primary" disabled={saving||loading||blocked||stale||draft.value===draft.preview.text} onClick={()=>void save()}>{t(saving?'Saving file…':'Save working file')}</button></div>
    </>}
  </div>;
}
