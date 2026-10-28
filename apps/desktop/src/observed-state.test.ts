import {describe,it,expect} from 'vitest';
import {reuseObserved} from './observed-state';
describe('fresh polling DTO reuse',()=>{
 it('preserves an unchanged graph/working observation but accepts changed metadata even with the same token',()=>{
  const before={token:'same',status:{branch:'main',dirty:false},files:[] as string[]};
  expect(reuseObserved(before,structuredClone(before))).toBe(before);
  const switched={...before,status:{branch:'feature',dirty:false}};
  expect(reuseObserved(before,switched)).toBe(switched);
  const changed={...before,files:['new.txt']};expect(reuseObserved(before,changed)).toBe(changed);
 });
 it('does not hide pending operations or tracking configuration changes',()=>{
  const clear={phase:'none',conflicts:[] as string[]};const conflict={phase:'pending',conflicts:['file']};expect(reuseObserved(clear,conflict)).toBe(conflict);
  const old={ahead:1,behind:0,remote:'origin'},next={...old,remote:'other'};expect(reuseObserved(old,next)).toBe(next);
 });
});
