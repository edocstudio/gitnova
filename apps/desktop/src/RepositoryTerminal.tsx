import {useEffect,useRef,useState} from 'react';
import {Terminal} from '@xterm/xterm';
import {FitAddon} from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import {api,errorMessage} from './api';
import {t,useLocale} from './i18n';
import {Icon} from './Icons';
export type TerminalEntry={membershipId:string;name:string};

function Shell({entry,visible,onReady}:{entry:TerminalEntry;visible:boolean;onReady:()=>void}){
 useLocale();
 const root=useRef<HTMLDivElement>(null);
 const live=useRef({visible,onReady});live.current={visible,onReady};
 const [status,setStatus]=useState('Opening terminal…');const [error,setError]=useState('');
 const [context,setContext]=useState<{path:string;shell:string}|null>(null);
 const control=useRef<{fit:()=>void}|null>(null);
 useEffect(()=>{
  let disposed=false;let sessionId='';let timer:ReturnType<typeof setTimeout>|undefined;
  let outputPending=false;let queue='';let writing=false;let stopped=false;
  const terminal=new Terminal({scrollback:2000,fontSize:12,fontFamily:'Menlo, Monaco, monospace',cursorBlink:true,allowProposedApi:false,theme:{background:'#0c141c',foreground:'#d5e2ee',cursor:'#72afff'},disableStdin:true});
  const fit=new FitAddon();terminal.loadAddon(fit);terminal.open(root.current!);
  const fail=(e:unknown)=>{if(!disposed){setError(errorMessage(e));terminal.options.disableStdin=true;stopped=true;}};
  const resize=()=>{if(!live.current.visible||disposed)return;fit.fit();if(sessionId)void api.terminalResize(entry.membershipId,sessionId,Math.max(2,Math.min(500,terminal.cols)),Math.max(2,Math.min(200,terminal.rows))).catch(fail);};
  control.current={fit:()=>{resize();terminal.focus();}};
  const observer=new ResizeObserver(resize);observer.observe(root.current!);
  const drain=async()=>{
   if(writing||!sessionId||disposed||stopped)return;writing=true;
   try{while(queue&&!disposed&&!stopped){let end=Math.min(queue.length,2048);if(end<queue.length&&/[\uD800-\uDBFF]/.test(queue[end-1]))end--;const chunk=queue.slice(0,end);queue=queue.slice(end);await api.terminalWrite(entry.membershipId,sessionId,chunk);}}
   catch(e){fail(e);}finally{writing=false;}
  };
  const input=terminal.onData(data=>{if(stopped||disposed)return;if(queue.length+data.length>65536){setError('Terminal paste is too large; paste smaller sections.');return;}queue+=data;void drain();});
  const poll=async()=>{
   if(disposed||stopped)return;
   if(!live.current.visible||document.hidden){timer=setTimeout(()=>void poll(),300);return;}
   try{
    const batch=await api.terminalRead(entry.membershipId,sessionId);if(disposed)return;
    outputPending=true;
    if(batch.dropped)terminal.writeln(`\r\n${t('Older terminal output was trimmed to keep memory bounded.')}\r\n`);
    terminal.write(new Uint8Array(batch.data),()=>{outputPending=false;if(disposed)return;if(batch.exited){stopped=true;terminal.options.disableStdin=true;setStatus('Shell exited');live.current.onReady();}else timer=setTimeout(()=>void poll(),batch.data.length?16:100);});
   }catch(e){fail(e);}
  };
  // StrictMode can dispose the first effect before it becomes interactive.
  // Defer admission one microtask so that cancelled mounts never spawn a shell.
  void Promise.resolve().then(async()=>{
   if(disposed)return;
   const opened=await api.terminalOpen(entry.membershipId,80,24);
   sessionId=opened.id;
   if(disposed){void api.terminalClose(entry.membershipId,sessionId).catch(()=>{});return;}
   setContext(opened);setStatus('Interactive shell');terminal.options.disableStdin=false;resize();terminal.focus();void poll();
  }).catch(fail);
  return()=>{disposed=true;clearTimeout(timer);observer.disconnect();input.dispose();control.current=null;if(sessionId)void api.terminalClose(entry.membershipId,sessionId).catch(()=>{});if(outputPending)requestAnimationFrame(()=>terminal.dispose());else terminal.dispose();};
 },[entry.membershipId]);
 useEffect(()=>{if(visible)control.current?.fit();},[visible]);
 return <div className="repository-shell" hidden={!visible}>
  <div className="terminal-context"><span role="status">{t(status)}</span>{context&&<span title={context.path}>{t('Started in {0}',[context.path])} · {context.shell}</span>}</div>
  {error&&<p role="alert" className="terminal-error">{t(error)}</p>}
  <div ref={root} className="terminal-emulator" aria-label={t('Repository terminal input')}/>
 </div>;
}
export function RepositoryTerminal({entries,selectedId,visible,onHide,onEnd,onChanged}:{entries:TerminalEntry[];selectedId?:string;visible:boolean;onHide:()=>void;onEnd:(id:string)=>void;onChanged:()=>void}){
 useLocale();const active=entries.find(entry=>entry.membershipId===selectedId);
 const [height,setHeight]=useState(260);const [ending,setEnding]=useState(false);
 useEffect(()=>setEnding(false),[selectedId,visible]);
 return <section className="repository-terminal" hidden={!visible||!active} style={{height}} aria-label={t('Repository terminal')}>
  <div className="terminal-resize" role="separator" tabIndex={0} aria-label={t('Resize terminal')} aria-orientation="horizontal" aria-valuenow={height} aria-valuemin={160} aria-valuemax={600} onKeyDown={e=>{if(e.key==='ArrowUp'||e.key==='ArrowDown'){e.preventDefault();setHeight(h=>Math.max(160,Math.min(600,h+(e.key==='ArrowUp'?20:-20))));}}} onPointerDown={e=>{e.preventDefault();e.currentTarget.setPointerCapture(e.pointerId);const y=e.clientY,h=height;const move=(event:PointerEvent)=>setHeight(Math.max(160,Math.min(600,h+y-event.clientY)));const end=()=>{window.removeEventListener('pointermove',move);window.removeEventListener('pointerup',end);window.removeEventListener('pointercancel',end);};window.addEventListener('pointermove',move);window.addEventListener('pointerup',end);window.addEventListener('pointercancel',end);}}/>
  <header><Icon name="terminal" size={16}/><strong>{t('Terminal')} · {active?.name}</strong><small>{t('{0} active sessions',[entries.length])}</small><button onClick={()=>setEnding(true)}>{t('End session')}</button><button aria-label={t('Hide terminal')} title={t('Hide terminal; keep the shell running')} onClick={()=>{onHide();onChanged();}}><Icon name="close" size={16}/></button></header>
  {ending&&<div className="terminal-end-review">{t('End this shell and its running commands?')} <button onClick={()=>{if(selectedId)onEnd(selectedId);onChanged();setEnding(false);}}>{t('End session')}</button><button onClick={()=>setEnding(false)}>{t('Cancel')}</button></div>}
  <div className="terminal-hint">{t('Shell commands run directly, outside GitNova previews and connection commit identities.')}</div>
  {entries.map(entry=><Shell key={entry.membershipId} entry={entry} visible={visible&&entry.membershipId===selectedId} onReady={onChanged}/>)}
 </section>;
}
