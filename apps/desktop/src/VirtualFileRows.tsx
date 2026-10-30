import {useLayoutEffect,useRef,useState} from 'react';
import type {ReactNode} from 'react';

// Small lists retain ordinary DOM/tab behavior. Large manifests retain all
// data/counts but mount only the viewport and a small keyboard overscan.
export function VirtualFileRows<T>({items,itemKey,selection,children,empty,className='',rowHeight=40}:{items:T[];itemKey:(item:T)=>string;selection:string|null;children:(item:T,index:number)=>ReactNode;empty:ReactNode;className?:string;rowHeight?:number}) {
 const root=useRef<HTMLDivElement>(null);
 const [scroll,setScroll]=useState(0);const [height,setHeight]=useState(300);
 const virtual=items.length>200;
 const selectionIndex=items.findIndex(item=>itemKey(item)===selection);
 const start=virtual?Math.max(0,Math.min(items.length-1,Math.floor(scroll/rowHeight)-4)):0;
 const end=virtual?Math.min(items.length,Math.ceil((scroll+height)/rowHeight)+4):items.length;
 useLayoutEffect(()=>{const node=root.current;if(!node)return;const measure=()=>setHeight(node.clientHeight||300);const observer=new ResizeObserver(measure);observer.observe(node);measure();return()=>observer.disconnect();},[]);
 useLayoutEffect(()=>{
  const node=root.current;if(!node||!virtual)return;
  // A Stage/Unstage or refreshed manifest may select an offscreen entry.
  const index=selectionIndex;
  if(index<0){if(node.scrollTop>Math.max(0,items.length*rowHeight-node.clientHeight))node.scrollTop=Math.max(0,items.length*rowHeight-node.clientHeight);setScroll(node.scrollTop);return;}
  if(index*rowHeight<node.scrollTop)node.scrollTop=index*rowHeight;
  else if((index+1)*rowHeight>node.scrollTop+node.clientHeight)node.scrollTop=(index+1)*rowHeight-node.clientHeight;
  setScroll(node.scrollTop);
 },[selectionIndex,virtual,items.length,rowHeight]);
 return <div ref={root} role={virtual?'list':undefined} className={`change-group-files ${className}${virtual?' virtual-change-list':''}`} onScroll={event=>setScroll(event.currentTarget.scrollTop)} onKeyDown={event=>{
  if(!virtual||!['ArrowDown','ArrowUp','Home','End'].includes(event.key)||event.altKey||event.ctrlKey||event.metaKey||event.shiftKey)return;
  const row=(event.target as HTMLElement).closest<HTMLElement>('[data-file-index]');if(!row)return;
  const index=Number(row.dataset.fileIndex);const next=event.key==='Home'?0:event.key==='End'?items.length-1:Math.max(0,Math.min(items.length-1,index+(event.key==='ArrowDown'?1:-1)));
  event.preventDefault();const node=event.currentTarget;
  if(next*rowHeight<node.scrollTop)node.scrollTop=next*rowHeight;
  else if((next+1)*rowHeight>node.scrollTop+node.clientHeight)node.scrollTop=(next+1)*rowHeight-node.clientHeight;
  setScroll(node.scrollTop);requestAnimationFrame(()=>node.querySelector<HTMLButtonElement>(`[data-file-index="${next}"] button`)?.focus({preventScroll:true}));
 }}>
  {virtual&&<div aria-hidden="true" style={{height:start*rowHeight}}/>}
  {virtual?items.slice(start,end).map((item,index)=><div key={itemKey(item)} role="listitem" aria-setsize={items.length} aria-posinset={start+index+1} data-file-index={start+index} style={{height:rowHeight,overflow:'hidden'}}>{children(item,start+index)}</div>):items.map(children)}
  {virtual&&<div aria-hidden="true" style={{height:(items.length-end)*rowHeight}}/>}
  {!items.length&&empty}
 </div>;
}
