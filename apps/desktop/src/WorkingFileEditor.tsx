import {useEffect,useMemo,useRef,useState} from 'react';
import {api,errorMessage} from './api';
import type {NativePath,WorkingFileText} from './contracts';
import {t} from './i18n';
import {HighlightedFileText} from './HighlightedFileText';
import {editorOffset,editorSectionPosition} from './edit-location';
import type {EditAnchor} from './edit-location';
import {EDIT_SECTION_LIMIT,fitsUtf8Budget,splitEditorText} from './working-file-text';

export type FileDraft={preview:WorkingFileText;value:string;sections:string[]};
export type FileDrafts=Map<string,FileDraft>;
export function WorkingFileEditor({membershipId,path,anchor=null,drafts,blocked,onBusyChange,onSaved,onClose}:{membershipId:string;path:NativePath;anchor?:EditAnchor|null;drafts:FileDrafts;blocked:boolean;onBusyChange:(busy:boolean)=>void;onSaved:()=>Promise<void>;onClose:()=>void}) {
  const key=JSON.stringify([membershipId,path.encoding,path.hex]);
  const [draft,setDraft]=useState<FileDraft|null>(()=>drafts.get(key)??null);
  const [initialPosition,setInitialPosition]=useState(()=>{const saved=drafts.get(key);return saved?editorSectionPosition(saved.sections,editorOffset(saved.value,anchor)):null;});
  const [section,setSection]=useState(initialPosition?.section??0);
  const baseline=useMemo(()=>splitEditorText(draft?.preview.text??''),[draft?.preview.text]);
  const flight=useRef(false);
  const [saved,setSaved]=useState(false);const [refreshFailed,setRefreshFailed]=useState(false);
  const [error,setError]=useState('');const [loading,setLoading]=useState(false);const [saving,setSaving]=useState(false);const [stale,setStale]=useState(false);
  useEffect(()=>()=>{const current=drafts.get(key);if(current?.value===current?.preview.text)drafts.delete(key);},[key,drafts]);
  useEffect(()=>{
    if(drafts.has(key))return;
    let cancelled=false;setLoading(true);
    if(drafts.size>=4){setError('Four file drafts are already open. Save or discard one before editing another file.');setLoading(false);return;}
    api.workingFileText(membershipId,path).then(preview=>{if(!cancelled){const value={preview,value:preview.text,sections:splitEditorText(preview.text)};const position=editorSectionPosition(value.sections,editorOffset(value.value,anchor));setInitialPosition(position);setSection(position.section);drafts.set(key,value);setDraft(value);}}).catch(e=>{if(!cancelled)setError(errorMessage(e));}).finally(()=>{if(!cancelled)setLoading(false);});
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
  const refreshSaved=async()=>{
    setRefreshFailed(false);setError('');
    try{await onSaved();onClose();}
    catch{setRefreshFailed(true);}
  };
  const save=async()=>{
    if(!draft||flight.current||blocked)return;
    flight.current=true;setSaving(true);onBusyChange(true);setError('');
    try{
      try{await api.saveWorkingFileText(membershipId,path,draft.value,draft.preview.token);}
      catch(e){setError(errorMessage(e));if(e&&typeof e==='object'&&'code' in e&&e.code==='stale_preview')setStale(true);return;}
      drafts.delete(key);setDraft(null);setSaved(true);
      await refreshSaved();
    }finally{flight.current=false;setSaving(false);onBusyChange(false);}
  };
  const retryRefresh=async()=>{
    if(flight.current||blocked)return;
    flight.current=true;setSaving(true);onBusyChange(true);
    try{await refreshSaved();}finally{flight.current=false;setSaving(false);onBusyChange(false);}
  };
  const reviewCurrent=async()=>{
    setLoading(true);setError('');try{const preview=await api.workingFileText(membershipId,path);if(draft)update({...draft,preview});setStale(false);}catch(e){setError(errorMessage(e));}finally{setLoading(false);}
  };
  return <div className="working-file-editor" role="region" aria-label={t('Edit current working file')}>
    <div className="dock-file-heading"><code>{path.display}</code><button disabled={saving} onClick={close}>{t('Back to diff')}</button></div>
    <p>{t('Editing the current working file. Save changes the file only; stage and commit it afterward. History and staging stay unchanged.')}</p>
    {loading&&<div className="loading-status" role="status">{t('Reading current file…')}</div>}
    {error&&<div className="inline-error" role="alert">{t(error)}</div>}
    {saved&&<p role="status">{t('Working file saved.')}</p>}
    {refreshFailed&&<div className="notice" role="region" aria-label={t('Refresh saved file')}><p>{t('The file was saved, but its refreshed status is unavailable. Retry refresh without saving the file again.')}</p><button disabled={saving||blocked} onClick={()=>void retryRefresh()}>{t('Retry refresh')}</button></div>}
    {draft&&<>{draft.sections.length>1&&<div className="working-editor-pagination"><button disabled={saving||loading||section===0} onClick={()=>setSection(value=>value-1)}>{t('Previous section')}</button><span>{t('Section {0} of {1}',[section+1,draft.sections.length])}<small>{t('Save writes the complete file, including other sections.')}</small></span><button disabled={saving||loading||section>=draft.sections.length-1} onClick={()=>setSection(value=>value+1)}>{t('Next section')}</button></div>}<div className="working-editor-columns"><div><strong>{t('Current file · read only')}</strong><HighlightedFileText path={path.display} text={baseline[section]??''} revealOffset={section===initialPosition?.section?initialPosition.offset:undefined}/></div><div><strong>{t('Your edits')}</strong><HighlightedFileText path={path.display} editable disabled={saving||loading} text={draft.sections[section]??''} revealOffset={section===initialPosition?.section?initialPosition.offset:undefined} onPaste={e=>{const area=e.currentTarget;const length=area.value.length-(area.selectionEnd-area.selectionStart)+e.clipboardData.getData('text').length;if(length>EDIT_SECTION_LIMIT){e.preventDefault();setError('This section is too large. Paste smaller portions.');}}} onChange={updateSection}/></div></div>
      <div className="working-editor-actions"><button disabled={saving||loading} onClick={()=>{drafts.delete(key);onClose();}}>{t('Discard editor draft')}</button>{stale&&<button disabled={saving||loading||blocked} onClick={()=>void reviewCurrent()}>{t('Review updated file and keep my draft')}</button>}<button className="primary" disabled={saving||loading||blocked||stale||draft.value===draft.preview.text} onClick={()=>void save()}>{t(saving?'Saving file…':'Save working file')}</button></div>
    </>}
  </div>;
}
