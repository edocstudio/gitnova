import {useSyncExternalStore} from 'react';
export const TERMINAL_IDLE_KEY='gitnova.terminal-idle-minutes';
export const TERMINAL_IDLE_CHOICES=[0,30,60] as const;
export function parseTerminalIdle(value:string|null):number{const n=value===null?0:Number(value);return TERMINAL_IDLE_CHOICES.some(v=>v===n)?n:0;}
let minutes=(()=>{try{return parseTerminalIdle(localStorage.getItem(TERMINAL_IDLE_KEY));}catch{return 0;}})();
const listeners=new Set<()=>void>();
export function setTerminalIdle(value:number){if(!TERMINAL_IDLE_CHOICES.some(v=>v===value))return;minutes=value;try{localStorage.setItem(TERMINAL_IDLE_KEY,String(value));}catch{/* Session-only preference when storage is unavailable. */}listeners.forEach(fn=>fn());}
export function useTerminalIdle(){return useSyncExternalStore(fn=>{listeners.add(fn);return()=>listeners.delete(fn);},()=>minutes,()=>0);}
if(typeof window!=='undefined')window.addEventListener('storage',event=>{if(event.key!==TERMINAL_IDLE_KEY)return;minutes=parseTerminalIdle(event.newValue);listeners.forEach(fn=>fn());});
