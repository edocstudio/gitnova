import type {PipelineSnapshot} from './contracts';
// One native provider read at a time. Rapid graph selection changes wait for the
// unavoidable HTTP read, then only the still-visible scope may start another.
let pending:{key:string;promise:Promise<unknown>}|null=null;
export async function readCi<T>(key:string,valid:()=>boolean,load:()=>Promise<T>):Promise<T>{
 while(pending&&pending.key!==key){await pending.promise.catch(()=>{});if(!valid())throw new Error('CI selection changed.');}
 if(!valid())throw new Error('CI selection changed.');
 if(pending?.key===key)return pending.promise as Promise<T>;
 const promise=load();pending={key,promise};
 try{return await promise;}finally{if(pending?.promise===promise)pending=null;}
}
export function ciBadgesForRepository(badges:Record<string,string>,id:string):Record<string,string>{
 const values=new Map<string,Set<string>>();
 for(const [key,status] of Object.entries(badges)){
  if(!key.startsWith(`${id}:`))continue;
  const oid=key.slice(id.length+1).split(':')[0];
  if(!values.has(oid))values.set(oid,new Set());values.get(oid)!.add(status);
 }
 return Object.fromEntries([...values].map(([oid,states])=>[oid,states.size===1?[...states][0]:'Mixed results']));
}

export type GraphCiLink={id:string;url:string;reference:string;kind?:'workflow';name?:string;status?:string};
export type GraphCiValue={status:string;checkedAt?:number;stale?:boolean;partial?:boolean;pipeline?:GraphCiLink;workflows?:GraphCiLink[]};
export function graphCiValue(value:PipelineSnapshot,stale=false):GraphCiValue{
 if(value.runs.some(run=>run.sha!==value.oid))throw new Error('CI revision did not match the selected commit.');
 const run=value.runs[0];
 const workflows=value.runs.filter(r=>r.source==='github_actions').map(r=>({id:String(r.displayNumber??r.id),url:r.url,reference:r.reference,name:r.name,status:r.status,kind:'workflow' as const}));
 const pipeline=run?.source==='gitlab_pipeline'&&/^[0-9]+$/.test(run.id)?{id:run.id,url:run.url,reference:run.reference}:workflows.length===1&&workflows[0].status===run?.status?workflows[0]:undefined;
 return {status:run?.status??'No pipeline',checkedAt:value.checkedAt,stale,pipeline,...(workflows.length?{workflows}:{}),...(value.partial?{partial:true}:{})};
}
export function ciStatusLabel(status:string):string{
 return ({success:'Passed',failed:'Failed',running:'Running',pending:'Pending',created:'Pending',preparing:'Preparing',waiting_for_resource:'Waiting',canceled:'Canceled',skipped:'Skipped',manual:'Manual',scheduled:'Scheduled',blocked:'Blocked',unknown:'Unknown'} as Record<string,string>)[status]??status;
}
