import {useEffect,useId,useLayoutEffect,useRef,useState} from 'react';
import {createPortal} from 'react-dom';
import type {AuthoredLine} from './diff-authors';
import {relativeAuthorTime} from './diff-authors';
import {formatTimestamp} from './date-format';
import {t,useLocale} from './i18n';
export type SelectedLineAuthor={value:AuthoredLine;anchor:HTMLButtonElement};
export function LineAuthorDetails({selected,onClose}:{selected:SelectedLineAuthor;onClose:(keyboard?:boolean)=>void}){
 const locale=useLocale();const panel=useRef<HTMLElement>(null),id=useId();
 const [panelHeight,setPanelHeight]=useState(240);
 useLayoutEffect(()=>{if(panel.current)setPanelHeight(panel.current.getBoundingClientRect().height);},[selected,locale]);
 const rect=selected.anchor.getBoundingClientRect(),width=Math.min(380,window.innerWidth-24);
 const left=Math.max(12,Math.min(window.innerWidth-width-12,rect.left));
 const top=rect.bottom+panelHeight+8<=window.innerHeight?rect.bottom+8:Math.max(12,rect.top-panelHeight-8);
 useEffect(()=>{
  let timer:ReturnType<typeof setTimeout>|undefined;const close=()=>onClose();
  const inside=(target:EventTarget|null)=>target instanceof Node&&(selected.anchor.contains(target)||panel.current?.contains(target));
  const move=(event:PointerEvent)=>{if(inside(event.target)){clearTimeout(timer);timer=undefined;}else if(!timer)timer=setTimeout(close,180);};
  const outside=(event:PointerEvent)=>{if(!inside(event.target))onClose();};
  const focus=(event:FocusEvent)=>{if(!inside(event.target))onClose();};
  const scroll=(event:Event)=>{if(!(event.target instanceof Node&&panel.current?.contains(event.target)))onClose();};
  const escape=(event:KeyboardEvent)=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();onClose(true);if(selected.anchor.isConnected)selected.anchor.focus();}};
  selected.anchor.setAttribute('aria-describedby',id);
  window.addEventListener('pointermove',move);window.addEventListener('pointerdown',outside);window.addEventListener('focusin',focus);window.addEventListener('scroll',scroll,true);window.addEventListener('resize',close);window.addEventListener('keydown',escape,true);
  return()=>{clearTimeout(timer);selected.anchor.removeAttribute('aria-describedby');window.removeEventListener('pointermove',move);window.removeEventListener('pointerdown',outside);window.removeEventListener('focusin',focus);window.removeEventListener('scroll',scroll,true);window.removeEventListener('resize',close);window.removeEventListener('keydown',escape,true);};
 },[selected,onClose,id]);
 const {value}=selected;
 return createPortal(<aside ref={panel} id={id} className="line-author-details" role="tooltip" aria-label={t('Line authorship details')} style={{left,top,width}}>
  <strong>{value.commit.author}</strong><span> · {relativeAuthorTime(value.commit.timestamp)}</span>
  <p>{t('Authored {0}',[formatTimestamp(value.commit.timestamp,'commit')])}</p>
  <code title={value.commit.oid}>{value.commit.oid.slice(0,12)}</code><p className="line-author-subject">{value.commit.subject}</p>
  <p>{t('Original line {0} in {1}',[value.line.originalLine,value.line.path.display])}</p>
  <small>{t(value.side==='before'?'Before revision {0}':'After revision {0}',[value.revision.slice(0,12)])}</small>
  {value.partial&&<p className="notice">{t('Line history is incomplete; boundary attribution may change when more history is available.')}</p>}
 </aside>,document.body);
}
