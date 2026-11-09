import {memo,useEffect,useMemo,useRef,useState} from 'react';
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
export function HighlightedFileText({path,text,editable=false,disabled=false,onChange,onPaste}:{path:string;text:string;editable?:boolean;disabled?:boolean;onChange?:(value:string)=>void;onPaste?:(event:ClipboardEvent<HTMLTextAreaElement>)=>void}){
  const language=syntaxLanguage(path);
  const [loaded,setLoaded]=useState<{language:typeof language;parser:Parser}|null>(null);
  const parser=loaded?.language===language?loaded.parser:null;
  const mirror=useRef<HTMLPreElement>(null);
  useEffect(()=>{
    if(!language)return;
    let cancelled=false;
    void loadSyntaxParser(language).then(parser=>{if(!cancelled)setLoaded({language,parser});}).catch(()=>{/* Plain text remains editable if a grammar cannot load. */});
    return()=>{cancelled=true;};
  },[language]);
  const scroll=(event:UIEvent<HTMLTextAreaElement>)=>{if(mirror.current){mirror.current.scrollTop=event.currentTarget.scrollTop;mirror.current.scrollLeft=event.currentTarget.scrollLeft;}};
  if(!editable)return <pre tabIndex={0} data-language={language??'text'}><Highlight text={text} parser={parser}/></pre>;
  return <div className="syntax-editor" data-language={language??'text'}>
    <pre ref={mirror} className="syntax-mirror" aria-hidden="true"><Highlight text={text} parser={parser}/>{'\n'}</pre>
    <textarea aria-label={t('Edit current file content')} spellCheck={false} wrap="off" disabled={disabled} value={text} onScroll={scroll} onPaste={onPaste} onChange={event=>onChange?.(event.target.value)}/>
  </div>;
}
