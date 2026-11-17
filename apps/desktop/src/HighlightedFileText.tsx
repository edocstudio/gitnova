import {memo,useEffect,useLayoutEffect,useMemo,useRef,useState} from 'react';
import type {ClipboardEvent,UIEvent} from 'react';
import type {Parser} from '@lezer/common';
import {loadSyntaxParser,syntaxLanguage,syntaxTokens} from './editor-syntax';
import {t} from './i18n';

function ColoredText({text,parser}:{text:string;parser:Parser|null}){
  const tokens=useMemo(()=>syntaxTokens(text,parser),[text,parser]);
  let end=0;
  const nodes=tokens.flatMap(token=>{
    const plain=text.slice(end,token.from);end=token.to;
    return [plain,<span key={token.from} className={token.className}>{text.slice(token.from,token.to)}</span>];
  });
  return <>{nodes}{text.slice(end)}</>;
}
const Highlight=memo(ColoredText);

// Keep the native textarea for IME, selection, undo, accessibility and exact text.
// Its colored mirror has identical metrics and never receives pointer/input events.
export function HighlightedFileText({path,text,editable=false,disabled=false,onChange,onPaste,revealOffset}:{path:string;text:string;editable?:boolean;disabled?:boolean;revealOffset?:number;onChange?:(value:string)=>void;onPaste?:(event:ClipboardEvent<HTMLTextAreaElement>)=>void}){
  const language=syntaxLanguage(path);
  const [loaded,setLoaded]=useState<{language:typeof language;parser:Parser}|null>(null);
  const parser=loaded?.language===language?loaded.parser:null;
  const mirror=useRef<HTMLPreElement>(null);
  const textarea=useRef<HTMLTextAreaElement>(null);
  const baseline=useRef<HTMLPreElement>(null);
  const baselineScroll=useRef(0);
  const revealed=useRef<number|undefined>(undefined);
  useLayoutEffect(()=>{
    if(revealOffset===undefined){revealed.current=undefined;return;}
    if(disabled||revealed.current===revealOffset)return;
    const node=editable?textarea.current:baseline.current;
    if(!node)return;
    const content=node instanceof HTMLTextAreaElement?node.value:node.textContent??'';
    const offset=Math.max(0,Math.min(revealOffset,content.length));
    if(node instanceof HTMLTextAreaElement){node.focus({preventScroll:true});node.setSelectionRange(offset,offset);}
    const line=content.slice(0,offset).split('\n').length-1;
    const lineHeight=Number.parseFloat(getComputedStyle(node).lineHeight)||24;
    node.scrollTop=Math.max(0,line*lineHeight-(node.clientHeight-lineHeight)/2);
    node.scrollLeft=0;
    if(!editable)baselineScroll.current=node.scrollTop;
    if(mirror.current){mirror.current.scrollTop=node.scrollTop;mirror.current.scrollLeft=0;}
    revealed.current=revealOffset;
  },[revealOffset,editable,disabled]);
  // A lazily loaded grammar replaces the mirror's text nodes. Reapply its
  // viewport after that layout so the initial jump remains visually aligned.
  useLayoutEffect(()=>{
    if(editable&&mirror.current&&textarea.current){mirror.current.scrollTop=textarea.current.scrollTop;mirror.current.scrollLeft=textarea.current.scrollLeft;}
    else if(baseline.current)baseline.current.scrollTop=baselineScroll.current;
  },[parser,text,editable]);
  useEffect(()=>{
    if(!language)return;
    let cancelled=false;
    void loadSyntaxParser(language).then(parser=>{if(!cancelled)setLoaded({language,parser});}).catch(()=>{/* Plain text remains editable if a grammar cannot load. */});
    return()=>{cancelled=true;};
  },[language]);
  const scroll=(event:UIEvent<HTMLTextAreaElement>)=>{if(mirror.current){mirror.current.scrollTop=event.currentTarget.scrollTop;mirror.current.scrollLeft=event.currentTarget.scrollLeft;}};
  if(!editable)return <pre ref={baseline} tabIndex={0} onScroll={event=>{baselineScroll.current=event.currentTarget.scrollTop;}} data-language={language??'text'}><Highlight text={text} parser={parser}/></pre>;
  return <div className="syntax-editor" data-language={language??'text'}>
    <pre ref={mirror} className="syntax-mirror" aria-hidden="true"><Highlight text={text} parser={parser}/>{'\n'}</pre>
    <textarea ref={textarea} aria-label={t('Edit current file content')} spellCheck={false} wrap="off" disabled={disabled} value={text} onScroll={scroll} onPaste={onPaste} onChange={event=>onChange?.(event.target.value)}/>
  </div>;
}
