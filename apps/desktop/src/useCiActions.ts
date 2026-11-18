import {useEffect,useRef,useState} from 'react';
import {api} from './api';
import {connections} from './connection-store';
import {readCi} from './ci-requests';
import {ciActions} from './ci-actions';
import type {CiActions} from './ci-actions';

type State={scope:string;value?:CiActions;mode:'loading'|'ready'|'unconfigured'|'unmatched'|'error';stale:boolean;providerScope?:string;reading?:boolean};
export function useCiActions(membershipId:string,oid:string,branch:string|null,blocked:boolean){
 const scope=JSON.stringify([membershipId,oid,branch]);
 const [state,setState]=useState<State>({scope,mode:'loading',stale:false});
 const [revision,setRevision]=useState(0);const epoch=useRef(0);const current=useRef(state);current.current=state;
 const refresh=useRef<()=>void>(()=>{});const attempt=useRef({scope,at:0});
 if(attempt.current.scope!==scope)attempt.current={scope,at:0};
 useEffect(()=>{
  const reset=()=>{epoch.current++;attempt.current={scope,at:0};current.current={scope,mode:'loading',stale:false};setState(current.current);setRevision(n=>n+1);};
  window.addEventListener('gitnova-ci-config',reset);window.addEventListener('gitnova-remotes-changed',reset);
  return()=>{window.removeEventListener('gitnova-ci-config',reset);window.removeEventListener('gitnova-remotes-changed',reset);};
 },[scope]);
 useEffect(()=>{
  const profiles=connections().filter(v=>v.enabled&&v.provider==='gitlab').map(({provider,origin})=>({provider,origin}));
  if(!profiles.length){setState({scope,mode:'unconfigured',stale:false});return;}
  let live=true,reading=false;const generation=epoch.current;
  const valid=()=>live&&generation===epoch.current&&!document.hidden;
  const publish=(next:State)=>{current.current=next;setState(next);};let timer:ReturnType<typeof setTimeout>|undefined;
  const old=current.current.scope===scope?current.current.value:undefined;
  publish({scope,value:old,providerScope:current.current.scope===scope?current.current.providerScope:undefined,mode:old?'ready':'loading',stale:!!old});
  const read=async(manual=false)=>{
   clearTimeout(timer);
   if(!valid()||reading||blocked)return;
   const interval=manual?2000:60000;const elapsed=Date.now()-attempt.current.at;
   if(elapsed<interval){timer=setTimeout(()=>void read(manual),interval-elapsed);return;}
   reading=true;attempt.current={scope,at:Date.now()};
   publish({...current.current,scope,reading:true});
   try{
    const selection=await api.ciConnection(membershipId,oid,profiles);
    if(!valid())return;
    if(!selection||selection.provider!=='gitlab'){publish({scope,mode:'unmatched',stale:false});return;}
    if(selection.oid!==oid)throw Error('CI scope changed');
    const providerScope=JSON.stringify([selection.remote,selection.provider,selection.origin]);
    // Do not carry rows across a changed remote/host, including a late read.
    const previous=current.current.providerScope===providerScope&&current.current.scope===scope?current.current.value:undefined;
    if(current.current.providerScope!==providerScope)publish({scope,providerScope,mode:'loading',stale:false,reading:true});
    const key=JSON.stringify([membershipId,selection.remote,selection.origin,oid]);
    const snapshot=await readCi(`gitlab:${key}`,valid,()=>api.pipelines(membershipId,selection,manual));
    if(!valid())return;
    if(snapshot.origin!==selection.origin)throw Error('CI host changed');
    const value=ciActions(snapshot,oid,branch,previous);
    publish({scope,providerScope,value,mode:'ready',stale:false});
   }catch{
    if(valid())publish({...current.current,scope,mode:'error',stale:true,reading:false});
   }finally{
    reading=false;
    if(valid()&&current.current.reading)publish({...current.current,reading:false});
    if(valid()&&!blocked)timer=setTimeout(()=>void read(),60000);
   }
  };
  const wake=()=>{clearTimeout(timer);if(!document.hidden&&!blocked)timer=setTimeout(()=>void read(),500);};
  refresh.current=()=>{void read(true);};
  if(!blocked&&!document.hidden)timer=setTimeout(()=>void read(),500);
  document.addEventListener('visibilitychange',wake);
  return()=>{live=false;clearTimeout(timer);refresh.current=()=>{};document.removeEventListener('visibilitychange',wake);};
 },[scope,membershipId,oid,branch,blocked,revision]);
 return {state:state.scope===scope?state:{scope,mode:'loading' as const,stale:false},refresh:()=>refresh.current()};
}
