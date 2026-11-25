import {api,errorMessage} from './api';
import type {Membership,RemoteBranches} from './contracts';
import {storedMapping} from './kustomize-images';

export interface FetchReport {
  total:number; done:number; skipped:number;
  failures:{member:Membership;reason:string}[];
}

export async function fetchWorkspace(
  members:Membership[], requested:Membership[], active:()=>boolean,
  progress:(report:FetchReport)=>void,
  updated:(repo:string,reference:string,oid:string|null)=>Promise<void>,
){
  const repositories=new Map(members.map(m=>[m.repositoryId,m]));
  const listings=new Map<string,Promise<RemoteBranches>>();
  const fetches=new Map<string,Promise<RemoteBranches|null>>();
  const report:FetchReport={total:requested.length,done:0,skipped:0,failures:[]};
  const publish=()=>progress({...report,failures:[...report.failures]});
  publish();
  for(const member of requested){
    if(!active())break;
    try{
      const mapping=storedMapping(member.repositoryId);
      const target=mapping?repositories.get(mapping.deployId):member;
      if(!target)throw new Error('Add the configured deploy repository to this workspace.');
      let listing=listings.get(target.id);
      if(!listing){listing=api.remoteBranches(target.id);listings.set(target.id,listing);}
      const {remotes}=await listing;
      if(!active())break;
      const reference=mapping?.reference??'';
      const tracked=reference.startsWith('refs/remotes/');
      const remote=tracked?remotes.filter(r=>reference.startsWith('refs/remotes/'+r+'/')).sort((a,b)=>b.length-a.length)[0]:
        remotes.includes('origin')?'origin':remotes.length===1?remotes[0]:undefined;
      if(!remote){
        if(tracked||remotes.length)throw new Error('No matching fetch remote.');
        report.skipped++;
      }else{
        const key=JSON.stringify([target.repositoryId,remote]);
        let fetched=fetches.get(key);
        if(!fetched){
          fetched=api.autoFetch(target.id,remote).then(name=>name?api.remoteBranches(target.id):null);
          fetches.set(key,fetched);
        }
        const after=await fetched;
        if(!active())break;
        if(!after)throw new Error('Repository could not be fetched now.');
        if(tracked){
          const oid=after.branches.find(b=>b.remote===remote&&'refs/remotes/'+remote+'/'+b.name===reference)?.oid??null;
          await updated(member.repositoryId,reference,oid);
        }
      }
    }catch(e){report.failures.push({member,reason:errorMessage(e)});}
    report.done++;
    if(active())publish();
  }
}
