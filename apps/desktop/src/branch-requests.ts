import type {BranchRequests,BranchTracking,OpenBranchRequest} from './contracts';
export function createRequestMatcher(snapshot:BranchRequests|null,tracking:BranchTracking[]){
  const locals=new Map(tracking.map(value=>[value.branch,value]));
  const outgoing=new Map<string,OpenBranchRequest[]>(),incoming=new Map<string,OpenBranchRequest[]>();
  for(const request of snapshot?.requests??[]){
    if(request.sourceProjectId===snapshot?.projectId)outgoing.set(request.sourceBranch,[...outgoing.get(request.sourceBranch)??[],request]);
    incoming.set(request.targetBranch,[...incoming.get(request.targetBranch)??[],request]);
  }
  return (names:string[]):{outgoing:OpenBranchRequest[];incoming:OpenBranchRequest[]}=>{
    const result={outgoing:new Map<number,OpenBranchRequest>(),incoming:new Map<number,OpenBranchRequest>()};
    if(snapshot){
      const branches=new Set<string>(),prefix=`refs/remotes/${snapshot.remote}/`;
      for(const name of names){
        if(name.startsWith(prefix))branches.add(name.slice(prefix.length));
        else if(name.startsWith('refs/heads/')){const value=locals.get(name.slice(11));if(value?.remote===snapshot.remote&&value.remoteBranch)branches.add(value.remoteBranch);}
      }
      for(const branch of branches){for(const value of outgoing.get(branch)??[])result.outgoing.set(value.number,value);for(const value of incoming.get(branch)??[])result.incoming.set(value.number,value);}
    }
    return {outgoing:[...result.outgoing.values()],incoming:[...result.incoming.values()]};
  };
}
export function requestsForRefs(snapshot:BranchRequests|null,names:string[],tracking:BranchTracking[]){return createRequestMatcher(snapshot,tracking)(names);}
export function validateBranchRequests(value:BranchRequests):BranchRequests {
  if(!['github','gitlab'].includes(value.provider)||!value.project||!value.projectId||!value.remote||!Array.isArray(value.requests)||value.requests.length>300)throw new Error('Invalid request scope');
  const origin=new URL(value.origin);if(origin.protocol!=='https:'||origin.username||origin.password||origin.search||origin.hash)throw new Error('Invalid request host');
  for(const request of value.requests){
    if(!Number.isSafeInteger(request.number)||request.number<1||!request.sourceBranch||!request.targetBranch||request.title.length>4096)throw new Error('Invalid request identity');
    const route=value.provider==='github'?'pull':'-/merge_requests';
    const project=value.project.split('/').map(part=>encodeURIComponent(part).replace(/[!'()~]/g,char=>`%${char.charCodeAt(0).toString(16).toUpperCase()}`)).join('/');
    if(request.url!==`${value.origin}/${project}/${route}/${request.number}`)throw new Error('Invalid request URL');
  }
  return value;
}
