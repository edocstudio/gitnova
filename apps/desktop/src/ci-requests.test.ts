import {expect,it} from 'vitest';
import {ciBadgesForRepository,readCi} from './ci-requests';
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
