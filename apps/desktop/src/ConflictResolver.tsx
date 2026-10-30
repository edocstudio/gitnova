import {useEffect,useMemo,useRef,useState} from 'react';
import {t} from './i18n';
import {buildConflictResult,conflictBlocks,hasConflictMarkers,type ConflictChoice} from './conflicts';
import type {DockPatch} from './CommitDiffDock';
export function ConflictResolver({editor}:{editor:NonNullable<DockPatch['editor']>}) {
 const area=useRef<HTMLTextAreaElement>(null);
 const expected=useRef(editor.value);
 const [seed,setSeed]=useState(editor.value);
 const [choices,setChoices]=useState<ConflictChoice[]>([]);
 const [index,setIndex]=useState(0);
 // A manual/external replacement becomes the new draft snapshot. Never apply
 // old block choices over text that was not produced by those choices.
 useEffect(()=>{if(editor.value!==expected.current){expected.current=editor.value;setSeed(editor.value);setChoices([]);setIndex(0);}},[editor.value]);
 const blocks=useMemo(()=>conflictBlocks(seed),[seed]);
 const selected=Math.min(index,Math.max(0,blocks.length-1)),block=blocks[selected];
 const choice=choices[selected]??null;
 const remaining=blocks.filter((_,i)=>!choices[i]).length;
 const focusBlock=(next:number)=>{setIndex(next);const target=blocks[next];if(!target||!area.current)return;const before=buildConflictResult(seed.slice(0,target.start),choices.slice(0,next));const selection=choices[next];const content=selection==='current'?target.current:selection==='incoming'?target.incoming:selection==='both'?target.current+target.incoming:seed.slice(target.start,target.end);area.current.setSelectionRange(before.length,before.length+content.length);area.current.scrollTop=Math.max(0,(before.split('\n').length-2)*24);};
 const include=(side:'current'|'incoming',checked:boolean)=>{
  if(!block||editor.disabled)return;
  const current=side==='current'?checked:choice==='current'||choice==='both';
  const incoming=side==='incoming'?checked:choice==='incoming'||choice==='both';
  const next=[...choices];next[selected]=current&&incoming?'both':current?'current':incoming?'incoming':null;
  const result=buildConflictResult(seed,next);expected.current=result;setChoices(next);editor.onChange(result);
 };
 const manual=(value:string)=>{expected.current=value;setSeed(value);setChoices([]);setIndex(0);editor.onChange(value);};
 const markers=hasConflictMarkers(editor.value);
 return <div className="conflict-resolver" role="region" aria-label={t('Conflict resolution editor')}>
  <div className="conflict-toolbar">
   <span role="status">{blocks.length?t('Conflict {0} of {1}',[selected+1,blocks.length]):markers?t('Incomplete conflict markers remain. Edit the result before reviewing.'):t('No conflict blocks remain. Review the result before staging.')}{blocks.length>0&&<small>{t('{0} unresolved blocks',[remaining])}</small>}</span>
   {blocks.length>0&&<><button disabled={editor.disabled||selected===0} onClick={()=>focusBlock(selected-1)}>{t('Previous conflict')}</button><button disabled={editor.disabled||selected===blocks.length-1} onClick={()=>focusBlock(selected+1)}>{t('Next conflict')}</button></>}
   {editor.readOnly?<button disabled={editor.disabled||!editor.onEdit} onClick={editor.onEdit}>{t('Edit result')}</button>:<button className="primary" disabled={editor.disabled||markers||!editor.onReview} onClick={editor.onReview}>{t('Review edited result')}</button>}
  </div>
  <div className="conflict-columns">
   {(['current','incoming'] as const).map(side=>{
    const isCurrent=side==='current';
    const text=block?(isCurrent?block.current:block.incoming):isCurrent?editor.currentDeleted?t('Deleted in this version'):editor.beforeUnavailable?t('Text preview unavailable. Use version selection or inspect with Git.'):editor.before:editor.incomingDeleted?t('Deleted in this version'):editor.incoming===null?t('Text preview unavailable. Use version selection or inspect with Git.'):editor.incoming;
    const included=choice===side||choice==='both';
    return <section className={`conflict-version ${isCurrent?'current':'incoming'} ${included?'included':''}`} key={side}>
     <div className="merge-editor-label"><label><input type="checkbox" aria-label={t(isCurrent?'Include current block':'Include incoming block')} disabled={editor.disabled||!block} checked={included} onChange={e=>include(side,e.target.checked)}/><strong>{t(isCurrent?'Current':'Incoming')}</strong></label><code title={isCurrent?editor.currentLabel:editor.incomingLabel}>{isCurrent?editor.currentLabel:editor.incomingLabel}</code></div>
     <pre tabIndex={0} aria-label={t(isCurrent?'Current conflict version':'Incoming conflict version')}>{text||t('No lines in this block')}</pre>
    </section>;
   })}
  </div>
  <section className="conflict-result"><div className="conflict-output-heading"><label className="merge-editor-label" htmlFor="conflict-result-editor">{t('Result')}</label><small>{t('Choices change only this draft. Review, then confirm to save and stage.')}{choice==='both'&&' '+t('When both are selected, Current is placed before Incoming.')}</small></div><textarea id="conflict-result-editor" ref={area} aria-label={t('Edit merge result')} readOnly={editor.readOnly} disabled={editor.disabled} value={editor.value} maxLength={524288} spellCheck={false} wrap="off" onChange={e=>manual(e.target.value)}/></section>
 </div>;
}
