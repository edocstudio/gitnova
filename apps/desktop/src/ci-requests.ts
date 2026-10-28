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
