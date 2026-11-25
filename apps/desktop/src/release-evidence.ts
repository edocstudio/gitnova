import {api,errorMessage} from './api';
import {connections} from './connection-store';
import {savedJenkinsConfig} from './JenkinsStatus';
import type {Membership, PipelineRun} from './contracts';

export interface Deployment {sha:string; tag:string|null; finished:string|null; id:string; url:string}
export interface ReleaseEvidence {
  production?:Deployment|null; run?:PipelineRun|null; jenkins?:string|null; scope?:string; build?:PipelineRun; buildError?:string;
  productionError?:string; pipelineError?:string; checked:number;
}
export async function readRelease(member:Membership, active:()=>boolean):Promise<ReleaseEvidence> {
  const info:ReleaseEvidence={checked:Date.now()};
  const listing=await api.remoteBranches(member.id);
  const preferred=listing.remotes.includes('origin')?'origin':listing.remotes[0];
  let oid=listing.branches.find(b=>b.remote===preferred && b.name==='release')?.oid ?? listing.branches[0]?.oid;
  const profiles=connections().filter(c=>c.enabled).map(({provider,origin})=>({provider,origin}));
  if(!oid && preferred && profiles.length && active()) oid=(await api.branches(member.id))[0]?.oid;
  const selection=oid && profiles.length && active()?await api.ciConnection(member.id,oid,profiles):null;
  const remote=selection?.remote??preferred;
  const urls=remote && listing.urls?.[remote];
  if(urls?.length) info.scope=JSON.stringify([member.repositoryId,remote,urls]);
  const jenkins=savedJenkinsConfig(member.id);
  if(jenkins && jenkins.remote===remote)info.jenkins=jenkins.jobUrl;
  if(selection?.provider==='gitlab' && active()){
    const [prod,uat]=await Promise.allSettled([api.productionDeployment(member.id,selection),api.releasePipeline(member.id,selection)]);
    if(prod.status==='fulfilled')info.production=prod.value;else info.productionError=errorMessage(prod.reason);
    if(uat.status==='fulfilled')info.run=uat.value;else info.pipelineError=errorMessage(uat.reason);
  }
  const release=listing.branches.find(b=>b.remote===remote && b.name==='release');
  if(jenkins && info.jenkins && release && active()){
    try{const builds=await api.jenkinsBuilds(member.id,{...jenkins,oid:release.oid});
      info.build=builds.pipeline.runs.find(run=>run.sha===release.oid);
    }catch(e){info.buildError=errorMessage(e);}
  }
  info.checked=Date.now();
  return info;
}
export function releaseAction(run?:PipelineRun|null){
  if(!run || run.reference!=='release' || ['failed','canceled','skipped','success'].includes(run.status))return null;
  const job=run.jobs.find(j=>j.status==='manual' && j.allowFailure===false);
  if(!job || !/^\d+$/.test(job.id))return run.status==='manual' || run.status==='blocked'?{label:'Open pipeline',url:run.url}:null;
  const url=new URL(run.url);
  if(url.protocol!=='https:' || !/\/-\/pipelines\/\d+\/?$/.test(url.pathname))return {label:'Open pipeline',url:run.url};
  url.pathname=url.pathname.replace(/\/-\/pipelines\/\d+\/?$/,'/-/jobs/'+job.id);url.search='';url.hash='';
  return {label:'Open manual job',url:url.href};
}
export interface Confirmation {sha:string; version:string; by:string; url:string; time:number}
export function readConfirmation(scope?:string):Confirmation|null {
  if(!scope)return null;
  try{const data=JSON.parse(localStorage.getItem('gitnova.release.confirmed.'+scope)??'null');
    return data && [40,64].includes(data.sha.length) && /^[a-f0-9]+$/.test(data.sha) && typeof data.by==='string' && typeof data.version==='string' && typeof data.url==='string' && Number.isFinite(data.time)?data:null;
  }catch{return null;}
}
