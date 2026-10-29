import {useEffect, useRef, useState} from 'react';
import {t} from './i18n';
import {columnLimits, columnWidth} from './history-columns';
import type {HistoryColumn} from './history-columns';
export function HistoryColumnHeader({column,label,width,naturalGraph,onResize,onReset}:{column:HistoryColumn;label:string;width:number;naturalGraph:number;onResize:(width:number,persist:boolean)=>void;onReset:()=>void}) {
  const header=useRef<HTMLSpanElement>(null);
  const [measured,setMeasured]=useState(width);
  useEffect(()=>{const node=header.current;if(!node)return;const observer=new ResizeObserver(()=>setMeasured(Math.round(node.getBoundingClientRect().width)));observer.observe(node);return()=>observer.disconnect();},[]);
  const drag=useRef<{pointer:number;x:number;width:number;value:number}|null>(null);
  const frame=useRef<number|null>(null);
  const [min,max]=columnLimits(column,naturalGraph);
  const cancelFrame=()=>{if(frame.current!==null)cancelAnimationFrame(frame.current);frame.current=null;};
  useEffect(()=>()=>{if(frame.current!==null)cancelAnimationFrame(frame.current);},[]);
  return <span ref={header} className="history-column-heading" data-column={column}><span>{t(label)}</span><span className="history-column-resize" role="separator" aria-orientation="vertical" aria-label={t('Resize {0} column',[t(label)])} aria-valuemin={min} aria-valuemax={Math.max(max,measured)} aria-valuenow={measured} tabIndex={0} title={t('Drag to resize. Double-click to reset this column.')}
    onDoubleClick={event=>{event.preventDefault();onReset();}}
    onKeyDown={event=>{if(event.key==='Home'){event.preventDefault();onReset();}else if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();const current=event.currentTarget.parentElement?.getBoundingClientRect().width??width;onResize(columnWidth(column,current+(event.key==='ArrowRight'?1:-1)*(event.shiftKey?40:10),naturalGraph),true);}}}
    onPointerDown={event=>{if(event.button!==0)return;event.preventDefault();event.currentTarget.focus();const current=event.currentTarget.parentElement?.getBoundingClientRect().width??width;drag.current={pointer:event.pointerId,x:event.clientX,width:current,value:current};event.currentTarget.setPointerCapture(event.pointerId);}}
    onPointerMove={event=>{const current=drag.current;if(!current||current.pointer!==event.pointerId)return;current.value=columnWidth(column,current.width+event.clientX-current.x,naturalGraph);if(frame.current===null)frame.current=requestAnimationFrame(()=>{frame.current=null;if(drag.current)onResize(drag.current.value,false);});}}
    onPointerUp={event=>{const current=drag.current;if(!current||current.pointer!==event.pointerId)return;cancelFrame();drag.current=null;onResize(current.value,true);event.currentTarget.releasePointerCapture(event.pointerId);}}
    onPointerCancel={()=>{cancelFrame();if(drag.current)onResize(drag.current.width,false);drag.current=null;}}
    onLostPointerCapture={()=>{cancelFrame();if(drag.current)onResize(drag.current.value,true);drag.current=null;}}/></span>;
}
