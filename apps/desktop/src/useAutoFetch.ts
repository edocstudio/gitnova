import {useEffect,useRef,useState} from 'react';
import {api,errorMessage} from './api';
import {FetchSchedule,useAutoFetchInterval} from './auto-fetch';
const schedule=new FetchSchedule();
let inFlight=false;
export function useAutoFetch(id:string|undefined,blocked:boolean,onBusy:(busy:boolean)=>void,onFetched:(id:string)=>Promise<void>){
 const seconds=useAutoFetchInterval();
 const latest=useRef({id,blocked,onBusy,onFetched});latest.current={id,blocked,onBusy,onFetched};
 const [status,setStatus]=useState<{id:string;error:string;remote:string|null;at:number;running:boolean}|null>(null);
 useEffect(()=>{schedule.reset();},[seconds]);
 useEffect(()=>{
  if(!id||!seconds)return;let live=true;let timer:ReturnType<typeof setTimeout>|undefined;
  const arm=(delay:number)=>{clearTimeout(timer);if(live&&!document.hidden)timer=setTimeout(()=>void tick(),delay);};
  const tick=async()=>{
   if(!live||document.hidden)return;
   const current=latest.current;const now=Date.now(),due=schedule.due(id,now,seconds);
   if(due>now){arm(due-now);return;}
   if(current.id!==id||current.blocked||inFlight||navigator.onLine===false||document.querySelector('.branch-menu,dialog[open]')){arm(5000);return;}
   inFlight=true;current.onBusy(true);setStatus(old=>({id,error:old?.id===id?old.error:'',remote:null,at:old?.id===id?old.at:0,running:true}));
   try{
    const remote=await api.autoFetch(id);schedule.finish(id,Date.now(),seconds);
    if(live&&latest.current.id===id){if(remote)await latest.current.onFetched(id);if(live&&latest.current.id===id)setStatus({id,error:'',remote,at:remote?Date.now():0,running:false});}
   }catch(e){schedule.finish(id,Date.now(),seconds,true);if(live&&latest.current.id===id)setStatus({id,error:errorMessage(e),remote:null,at:0,running:false});}
   finally{inFlight=false;latest.current.onBusy(false);arm(Math.max(1000,schedule.due(id,Date.now(),seconds)-Date.now()));}
  };
  const wake=()=>{if(document.hidden)clearTimeout(timer);else arm(1000);};
  const written=()=>{schedule.finish(id,Date.now(),seconds);wake();};
  arm(Math.max(1000,schedule.due(id,Date.now(),seconds)-Date.now()));
  document.addEventListener('visibilitychange',wake);window.addEventListener('online',wake);window.addEventListener('gitnova-write-completed',written);
  return()=>{live=false;clearTimeout(timer);document.removeEventListener('visibilitychange',wake);window.removeEventListener('online',wake);window.removeEventListener('gitnova-write-completed',written);};
 },[id,seconds]);
 return seconds>0&&status?.id===id?status:null;
}
