import {useEffect,useMemo,useState} from 'react';
import {api,errorMessage} from './api';
import type {NativePath,WorkingFileText} from './contracts';
import {t} from './i18n';
import {HighlightedFileText} from './HighlightedFileText';
import {EDIT_SECTION_LIMIT,fitsUtf8Budget,splitEditorText} from './working-file-text';

export type FileDraft={preview:WorkingFileText;value:string;sections:string[]};
export type FileDrafts=Map<string,FileDraft>;
export function WorkingFileEditor({membershipId,path,drafts,blocked,onBusyChange,onSaved,onClose}:{membershipId:string;path:NativePath;drafts:FileDrafts;blocked:boolean;onBusyChange:(busy:boolean)=>void;onSaved:()=>Promise<void>;onClose:()=>void}) {
  const key=JSON.stringify([membershipId,path.encoding,path.hex]);
  const [draft,setDraft]=useState<FileDraft|null>(()=>drafts.get(key)??null);
  const [section,setSection]=useState(0);
  const baseline=useMemo(()=>splitEditorText(draft?.preview.text??''),[draft?.preview.text]);
  const [error,setError]=useState('');const [loading,setLoading]=useState(false);const [saving,setSaving]=useState(false);const [stale,setStale]=useState(false);
  useEffect(()=>()=>{const current=drafts.get(key);if(current?.value===current?.preview.text)drafts.delete(key);},[key,drafts]);
  useEffect(()=>{
    if(drafts.has(key))return;
    let cancelled=false;setLoading(true);
    if(drafts.size>=4){setError('Four file drafts are already open. Save or discard one before editing another file.');setLoading(false);return;}
    api.workingFileText(membershipId,path).then(preview=>{if(!cancelled){const value={preview,value:preview.text,sections:splitEditorText(preview.text)};drafts.set(key,value);setDraft(value);}}).catch(e=>{if(!cancelled)setError(errorMessage(e));}).finally(()=>{if(!cancelled)setLoading(false);});
    return()=>{cancelled=true;};
  },[key,membershipId,drafts]);
  const update=(value:FileDraft)=>{drafts.set(key,value);setDraft(value);};
  const updateSection=(text:string)=>{
    if(!draft)return;
    if(text.length>EDIT_SECTION_LIMIT){setError('This section is too large. Paste smaller portions.');return;}
    const sections=draft.sections.map((value,index)=>index===section?text:value);
    const value=sections.join('');
    if(!fitsUtf8Budget(value,draft.preview.maxBytes)){setError('The editor supports text up to 4 MiB.');return;}
    setError(current=>current==='The editor supports text up to 4 MiB.'||current==='This section is too large. Paste smaller portions.'?'':current);
    update({...draft,value,sections});
  };
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
    {draft&&<>{draft.sections.length>1&&<div className="working-editor-pagination"><button disabled={saving||loading||section===0} onClick={()=>setSection(value=>value-1)}>{t('Previous section')}</button><span>{t('Section {0} of {1}',[section+1,draft.sections.length])}<small>{t('Save writes the complete file, including other sections.')}</small></span><button disabled={saving||loading||section>=draft.sections.length-1} onClick={()=>setSection(value=>value+1)}>{t('Next section')}</button></div>}<div className="working-editor-columns"><div><strong>{t('Current file · read only')}</strong><HighlightedFileText path={path.display} text={baseline[section]??''}/></div><div><strong>{t('Your edits')}</strong><HighlightedFileText path={path.display} editable disabled={saving||loading} text={draft.sections[section]??''} onPaste={e=>{const area=e.currentTarget;const length=area.value.length-(area.selectionEnd-area.selectionStart)+e.clipboardData.getData('text').length;if(length>EDIT_SECTION_LIMIT){e.preventDefault();setError('This section is too large. Paste smaller portions.');}}} onChange={updateSection}/></div></div>
      <div className="working-editor-actions"><button disabled={saving||loading} onClick={()=>{drafts.delete(key);onClose();}}>{t('Discard editor draft')}</button>{stale&&<button disabled={saving||loading||blocked} onClick={()=>void reviewCurrent()}>{t('Review updated file and keep my draft')}</button>}<button className="primary" disabled={saving||loading||blocked||stale||draft.value===draft.preview.text} onClick={()=>void save()}>{t(saving?'Saving file…':'Save working file')}</button></div>
    </>}
  </div>;
}
