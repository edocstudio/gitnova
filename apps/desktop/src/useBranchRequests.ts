import {useEffect,useState} from 'react';
import {api} from './api';
import {connections} from './connection-store';
import type {BranchRequests} from './contracts';
import {validateBranchRequests} from './branch-requests';
const cache=new Map<string,BranchRequests>();
export function useBranchRequests(id:string,oid:string){
  const [revision,setRevision]=useState(0);
  const [state,setState]=useState<{id:string;snapshot:BranchRequests|null;stale:boolean;error:boolean}>({id,snapshot:null,stale:false,error:false});
  useEffect(()=>{const change=()=>{cache.clear();setRevision(v=>v+1);};const configuration=()=>{setState({id:'',snapshot:null,stale:false,error:false});change();};window.addEventListener('gitnova-ci-config',configuration);window.addEventListener('gitnova-requests-changed',change);return()=>{window.removeEventListener('gitnova-ci-config',configuration);window.removeEventListener('gitnova-requests-changed',change);};},[]);
  useEffect(()=>{
    const profiles=connections().filter(v=>v.enabled).map(({provider,origin})=>({provider,origin}));
    if(!oid||!profiles.length){setState({id,snapshot:null,stale:false,error:false});return;}
    setState(old=>old.id===id?old:{id,snapshot:null,stale:false,error:false});
    let live=true,reading=false;let timer:ReturnType<typeof setTimeout>|undefined;
    const read=async()=>{
      if(!live||reading||document.hidden)return;reading=true;let previous:BranchRequests|undefined;
      try{
        const selection=await api.ciConnection(id,oid,profiles);if(!live||document.hidden)return;
        if(!selection){setState({id,snapshot:null,stale:false,error:false});return;}
        const key=JSON.stringify([id,selection.remote,selection.provider,selection.origin]);previous=cache.get(key);
        const value=previous&&Date.now()/1000-previous.checkedAt<60?previous:validateBranchRequests(await api.branchRequests(id,selection));
        if(!live||document.hidden)return;
        if(value.remote!==selection.remote||value.origin!==selection.origin||value.provider!==selection.provider)throw new Error('Request scope changed');
        cache.delete(key);while(cache.size>=16)cache.delete(cache.keys().next().value!);cache.set(key,value);
        setState({id,snapshot:value,stale:false,error:false});
      }catch{if(live&&!document.hidden)setState({id,snapshot:previous??null,stale:Boolean(previous),error:true});}
      finally{reading=false;if(live&&!document.hidden)timer=setTimeout(()=>void read(),60000);}
    };
    const visibility=()=>{clearTimeout(timer);if(!document.hidden)timer=setTimeout(()=>void read(),500);};
    timer=setTimeout(()=>void read(),500);document.addEventListener('visibilitychange',visibility);
    return()=>{live=false;clearTimeout(timer);document.removeEventListener('visibilitychange',visibility);};
  },[id,oid,revision]);
  return state.id===id?state:{id,snapshot:null,stale:false,error:false};
}
