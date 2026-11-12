import {describe,it,expect,vi} from 'vitest';
import {SelectionReads} from './selection-reads';
import type {ReadTiming} from './selection-reads';
const deferred=<T>()=>{let resolve!:(value:T)=>void;let reject!:(error:unknown)=>void;const promise=new Promise<T>((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
const tick=async()=>{for(let i=0;i<8;i++)await Promise.resolve();};
describe('replaceable selection reads',()=>{
  it('runs the first and latest only; aborted work never publishes across repo switches',async()=>{
    const samples:ReadTiming[]=[];const queue=new SelectionReads(sample=>samples.push(sample));
    const first=deferred<string>();const calls:string[]=[];
    const a=new AbortController();const initial=queue.run('detail',()=>{calls.push('repo-a');return first.promise;},a.signal).catch(error=>error.name);
    a.abort();
    const pending=Array.from({length:50},(_,i)=>queue.run('detail',async()=>{calls.push(`repo-b-${i}`);return i;},new AbortController().signal).catch(error=>error.name));
    expect(calls).toEqual(['repo-a']);first.resolve('stale');await tick();
    expect(await initial).toBe('AbortError');expect((await Promise.all(pending)).slice(0,49)).toEqual(Array(49).fill('AbortError'));
    expect(await pending[49]).toBe(49);expect(calls).toEqual(['repo-a','repo-b-49']);
    expect(samples.filter(s=>s.started)).toHaveLength(2);expect(samples.filter(s=>!s.started)).toHaveLength(49);
    expect(samples.find(s=>s.started&&s.superseded)).toBeDefined();
  });
  it('aborts a queued selection without invoking native work and cleans listeners',async()=>{
    const queue=new SelectionReads();const first=deferred<void>();const a=new AbortController();
    const remove=vi.spyOn(a.signal,'removeEventListener');const native=vi.fn(async()=>1);
    const active=queue.run('working',()=>first.promise);const pending=queue.run('working',native,a.signal).catch(error=>error.name);
    a.abort();expect(await pending).toBe('AbortError');first.resolve();await active;await tick();
    expect(native).not.toHaveBeenCalled();expect(remove).toHaveBeenCalled();
    expect(await queue.run('working',async()=>2)).toBe(2);
  });
  it('releases a lane after native failure and keeps independent lanes responsive',async()=>{
    const queue=new SelectionReads();const first=deferred<number>();
    const active=queue.run('detail',()=>first.promise).catch(error=>error.message);
    const next=queue.run('detail',async()=>2);expect(await queue.run('patch',async()=>3)).toBe(3);
    first.reject(new Error('native failure'));expect(await active).toBe('native failure');expect(await next).toBe(2);
    expect(await queue.run('detail',async()=>4)).toBe(4);
  });
  it('does not retain results, so external Git changes are read again',async()=>{
    const queue=new SelectionReads();let value=1;const read=vi.fn(async()=>value);
    expect(await queue.run('detail',read)).toBe(1);await tick();value=2;
    expect(await queue.run('detail',read)).toBe(2);expect(read).toHaveBeenCalledTimes(2);
  });
  it('does not launch an already-aborted request',async()=>{
    const queue=new SelectionReads();const a=new AbortController();a.abort();const read=vi.fn(async()=>1);
    await expect(queue.run('detail',read,a.signal)).rejects.toHaveProperty('name','AbortError');expect(read).not.toHaveBeenCalled();
    expect(await queue.run('detail',async()=>2)).toBe(2);
  });
});
