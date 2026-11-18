import type {PipelineSnapshot} from './contracts';

export type CiActionKind='required'|'optional'|'unknown'|'blocked';
export type CiAction={id:string;oid:string;origin:string;project:string;reference:string;pipelineId:string;jobId:string|null;name:string;stage:string;url:string;kind:CiActionKind;stale:boolean};
export type CiActions={items:CiAction[];partial:boolean;checkedAt:number;pipelineId:string|null};
const invalid=()=>new Error('CI action evidence did not match the current context.');
const numeric=(id:string)=>/^[0-9]{1,32}$/.test(id);

// Pure replacement/merge of one HEAD's latest matching pipeline. No unread log,
// notification history or provider mutation is involved.
export function ciActions(snapshot:PipelineSnapshot,oid:string,branch:string|null,previous?:CiActions):CiActions{
 if(snapshot.oid!==oid||snapshot.runs.some(run=>run.sha!==oid))throw invalid();
 const origin=new URL(snapshot.origin);
 const segments=snapshot.project.split('/');
 if(origin.protocol!=='https:'||origin.username||origin.password||origin.search||origin.hash||!segments.length||segments.some(s=>!s||s==='.'||s==='..'||/[\u0000-\u001f]/.test(s)))throw invalid();
 const prior=(previous?.items??[]).filter(item=>item.oid===oid&&item.origin===snapshot.origin&&item.project===snapshot.project&&(!branch||item.reference===branch));
 const candidates=snapshot.runs.filter(run=>!branch||run.reference===branch);
 if(candidates.some(run=>!numeric(run.id)))throw invalid();
 const run=candidates.reduce<(typeof candidates)[number]|undefined>((latest,row)=>!latest||BigInt(row.id)>BigInt(latest.id)?row:latest,undefined);
 if(!run)return {items:snapshot.partial?prior.map(item=>({...item,stale:true})):[],partial:snapshot.partial,checkedAt:snapshot.checkedAt,pipelineId:null};
 const expected=`${snapshot.origin.replace(/\/+$/,'')}/${segments.map(encodeURIComponent).join('/')}/-/pipelines/${run.id}`;
 if(new URL(run.url).href!==new URL(expected).href)throw invalid();
 const seen=new Set<string>(),items:CiAction[]=[];
 const add=(jobId:string|null,name:string,stage:string,kind:CiActionKind)=>{
  const id=JSON.stringify([snapshot.origin,snapshot.project,oid,run.id,jobId]);
  if(items.some(item=>item.id===id))return;
  items.push({id,oid,origin:snapshot.origin,project:snapshot.project,reference:run.reference,pipelineId:run.id,jobId,name,stage,url:run.url,kind,stale:false});
 };
 for(const job of run.jobs.slice(0,50)){
  if(!numeric(job.id))throw invalid();
  if(seen.has(job.id))throw invalid();
  seen.add(job.id);
  if(job.status==='manual')add(job.id,job.name,job.stage,job.allowFailure===false?'required':job.allowFailure===true?'optional':'unknown');
 }
 const jobsPartial=run.jobsPartial||run.jobs.length>50;
 if(jobsPartial)for(const old of prior){
  if(old.pipelineId===run.id&&old.jobId&&!seen.has(old.jobId)&&!items.some(item=>item.id===old.id)&&items.length<51)items.push({...old,stale:true});
 }
 if(run.status==='blocked'&&!items.some(item=>item.kind==='required'))add(null,'','','blocked');
 else if(run.status==='manual'&&!items.length)add(null,'','','unknown');
 // Missing rows on an incomplete page are not evidence of resolution. Observed
 // jobs that changed status are removed; unseen same-pipeline entries stay cached.
 const rank={blocked:0,required:1,unknown:2,optional:3};
 items.sort((a,b)=>rank[a.kind]-rank[b.kind]||a.id.localeCompare(b.id));
 return {items,partial:snapshot.partial||jobsPartial,checkedAt:snapshot.checkedAt,pipelineId:run.id};
}
