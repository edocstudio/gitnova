import {useMemo,useRef,useState} from 'react';
import {t} from './i18n';
import {conflictBlocks,resolveBlock,hasConflictMarkers} from './conflicts';
import type {DockPatch} from './CommitDiffDock';
export function ConflictResolver({editor}:{editor:NonNullable<DockPatch['editor']>}) {
 const area=useRef<HTMLTextAreaElement>(null);const [index,setIndex]=useState(0);
 const blocks=useMemo(()=>conflictBlocks(editor.value),[editor.value]);const selected=Math.min(index,Math.max(0,blocks.length-1));const block=blocks[selected];
 const focusBlock=(next:number)=>{setIndex(next);const item=blocks[next];if(!item||!area.current)return;area.current.focus();area.current.setSelectionRange(item.start,item.end);area.current.scrollTop=Math.max(0,(editor.value.slice(0,item.start).split('\n').length-2)*24);};
 const accept=(choice:'current'|'incoming'|'both')=>{if(!block||editor.disabled||editor.readOnly)return;editor.onChange(resolveBlock(editor.value,block,choice));requestAnimationFrame(()=>area.current?.focus());};
 return <div className="conflict-resolver" role="region" aria-label={t('Conflict resolution editor')}>
  <div className="conflict-toolbar"><span role="status">{blocks.length?t('Conflict {0} of {1}',[selected+1,blocks.length]):hasConflictMarkers(editor.value)?t('Incomplete conflict markers remain. Edit the result before reviewing.'):t('No conflict blocks remain. Review the result before staging.')}</span>{blocks.length>0&&<><button disabled={selected===0} onClick={()=>focusBlock(selected-1)}>{t('Previous conflict')}</button><button disabled={selected===blocks.length-1} onClick={()=>focusBlock(selected+1)}>{t('Next conflict')}</button>{!editor.readOnly&&<><button disabled={editor.disabled} onClick={()=>accept('current')}>{t('Accept current block')}</button><button disabled={editor.disabled} onClick={()=>accept('incoming')}>{t('Accept incoming block')}</button><button disabled={editor.disabled} onClick={()=>accept('both')}>{t('Accept both blocks')}</button></>}</>}</div>
  <div className="conflict-columns"><section><div className="merge-editor-label">{t('Current · read only')} <code>{editor.currentLabel}</code></div><pre tabIndex={0} aria-label={t('Current conflict version')}>{editor.currentDeleted?t('Deleted in this version'):editor.beforeUnavailable?t('Text preview unavailable. Use version selection or inspect with Git.'):editor.before}</pre></section>
   <section className="conflict-result"><label className="merge-editor-label" htmlFor="conflict-result-editor">{editor.readOnly?t('Working result · select Edit result to resolve'):t('Result · editable draft, not saved')}</label><textarea id="conflict-result-editor" ref={area} aria-label={t('Edit merge result')} readOnly={editor.readOnly} disabled={editor.disabled} value={editor.value} maxLength={524288} spellCheck={false} wrap="off" onChange={e=>editor.onChange(e.target.value)}/></section>
   <section><div className="merge-editor-label">{editor.incomingLabel??t('Incoming · read only')}</div><pre tabIndex={0} aria-label={t('Incoming conflict version')}>{editor.incomingDeleted?t('Deleted in this version'):editor.incoming===null?t('Text preview unavailable. Use version selection or inspect with Git.'):editor.incoming}</pre></section>
  </div>
 </div>;
}
