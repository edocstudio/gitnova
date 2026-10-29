import {expect,it} from 'vitest';
import {ciBadgesForRepository,readCi,graphCiValue,ciStatusLabel} from './ci-requests';
it('keeps provider and repository results distinct rather than overwriting mixed results',()=>{
 const result=ciBadgesForRepository({'repo:abc:gitlab':'success','repo:abc:jenkins':'failed','repo:def:gitlab':'success','repo:def:jenkins':'success','other:abc:jenkins':'running'},'repo');
 expect(result).toEqual({abc:'Mixed results',def:'success'});
});
it('serializes provider requests and skips obsolete selection reads',async()=>{
 let finish!:(v:number)=>void;let obsolete=true;let called=false;
 const first=readCi('first',()=>true,()=>new Promise<number>(r=>{finish=r;}));
 const next=readCi('next',()=>obsolete,async()=>{called=true;return 2;});
 const rejected=expect(next).rejects.toThrow('CI selection changed.');
 obsolete=false;finish(1);expect(await first).toBe(1);await rejected;expect(called).toBe(false);
 expect(await readCi('latest',()=>true,async()=>3)).toBe(3);
});

it('deduplicates concurrent reads for the same provider scope',async()=>{
 let finish!:(n:number)=>void;let count=0;
 const first=readCi('same',()=>true,()=>{count++;return new Promise<number>(r=>{finish=r;});});
 const second=readCi('same',()=>true,async()=>{count++;return 99;});
 finish(7);expect(await first).toBe(7);expect(await second).toBe(7);expect(count).toBe(1);
});

it('retains exact-SHA pipeline identity and cached state, without inventing pipeline IDs for aggregate status',()=>{
 const value={oid:'a'.repeat(40),project:'team/project',origin:'https://git.example',checkedAt:123,partial:false,runs:[{id:'40778',sha:'a'.repeat(40),reference:'main',source:'gitlab_pipeline',status:'success',url:'https://git.example/team/project/-/pipelines/40778',jobs:[],jobsPartial:false}]};
 expect(graphCiValue(value,true)).toEqual({status:'success',checkedAt:123,stale:true,pipeline:{id:'40778',url:value.runs[0].url,reference:'main'}});
 expect(ciStatusLabel(graphCiValue(value).status)).toBe('Passed');
 expect(graphCiValue({...value,runs:[]})).toMatchObject({status:'No pipeline',pipeline:undefined});
 expect(graphCiValue({...value,runs:[{...value.runs[0],source:'gitlab_commit_status'}]}).pipeline).toBeUndefined();
 expect(()=>graphCiValue({...value,runs:[{...value.runs[0],sha:'b'.repeat(40)}]})).toThrow('CI revision did not match');
 expect(ciStatusLabel('unknown')).toBe('Unknown');expect(ciStatusLabel('failed')).toBe('Failed');
});

it('shows GitHub run number rather than API ID and preserves aggregate/multiple-workflow evidence',()=>{
 const aggregate={id:'a'.repeat(40),sha:'a'.repeat(40),reference:'',source:'github_status_and_checks',status:'failed',url:'https://github.com/team/project/commit/'+ 'a'.repeat(40),jobs:[],jobsPartial:false};
 const action={...aggregate,id:'99',displayNumber:1,name:'CI',reference:'main',source:'github_actions',url:'https://github.com/team/project/actions/runs/99'};
 const snapshot={oid:aggregate.sha,project:'team/project',origin:'https://github.com',checkedAt:123,partial:false,runs:[aggregate,action]};
 const badge=graphCiValue(snapshot);expect(badge.status).toBe('failed');expect(badge.pipeline).toMatchObject({id:'1',name:'CI',kind:'workflow',url:action.url,reference:'main'});
 expect(graphCiValue({...snapshot,runs:[aggregate,{...action,status:'success'}]}).pipeline).toBeUndefined();
 const multi=graphCiValue({...snapshot,runs:[aggregate,action,{...action,id:'100',name:'Release'}]});expect(multi.pipeline).toBeUndefined();expect(multi.workflows).toHaveLength(2);
 expect(graphCiValue({...snapshot,partial:true}).partial).toBe(true);
});
