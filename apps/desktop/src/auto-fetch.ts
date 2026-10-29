import {useSyncExternalStore} from 'react';
export const AUTO_FETCH_KEY='gitnova.auto-fetch-seconds';
export const AUTO_FETCH_INTERVALS=[0,30,60,120,300] as const;
export function parseFetchInterval(value:string|null):number{const n=value===null?60:Number(value);return AUTO_FETCH_INTERVALS.some(v=>v===n)?n:60;}
let interval=(()=>{try{return parseFetchInterval(localStorage.getItem(AUTO_FETCH_KEY));}catch{return 60;}})();
const listeners=new Set<()=>void>();
export function setAutoFetchInterval(value:number){if(!AUTO_FETCH_INTERVALS.some(v=>v===value))return;interval=value;try{localStorage.setItem(AUTO_FETCH_KEY,String(value));}catch{/* session preference */}listeners.forEach(fn=>fn());}
export function useAutoFetchInterval(){return useSyncExternalStore(fn=>{listeners.add(fn);return()=>listeners.delete(fn);},()=>interval,()=>60);}
if(typeof window!=='undefined')window.addEventListener('storage',e=>{if(e.key!==AUTO_FETCH_KEY)return;interval=parseFetchInterval(e.newValue);listeners.forEach(fn=>fn());});
// Per-checkout cooldown survives quick repository switches; storage is bounded.
export class FetchSchedule {
 private entries=new Map<string,{due:number;failures:number}>();
 private entry(id:string,now:number,seconds:number){let item=this.entries.get(id);if(!item){item={due:now+seconds*1000,failures:0};while(this.entries.size>=32)this.entries.delete(this.entries.keys().next().value!);this.entries.set(id,item);}return item;}
 due(id:string,now:number,seconds:number){return this.entry(id,now,seconds).due;}
 finish(id:string,now:number,seconds:number,failed=false){const entry=this.entry(id,now,seconds);entry.failures=failed?Math.min(entry.failures+1,5):0;entry.due=now+Math.min(900,seconds*2**entry.failures)*1000;}
 reset(){this.entries.clear();}
}
