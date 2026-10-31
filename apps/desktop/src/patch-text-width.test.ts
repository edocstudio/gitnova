import {describe,it,expect} from 'vitest';
import {TextWidthCache} from './patch-text-width';
describe('bounded diff text measurements',()=>{
 it('reuses exact text across file revisits and evicts least recent entries',()=>{
  const cache=new TextWidthCache(136);let reads=0;const measure=(text:string)=>{reads++;return text==='ไทย'?29:17;};
  expect(cache.measure('a',measure)).toBe(17);expect(cache.measure('ไทย',measure)).toBe(29);
  expect(cache.measure('a',measure)).toBe(17);expect(reads).toBe(2);
  cache.measure('b',measure);expect(cache.retainedBytes).toBeLessThanOrEqual(136);
  cache.measure('ไทย',measure);expect(reads).toBe(4);
 });
 it('measures oversized lines correctly without evicting or retaining them',()=>{
  const cache=new TextWidthCache(80);let reads=0;const measure=(text:string)=>{reads++;return text.length*9;};
  cache.measure('a',measure);const before=cache.retainedBytes;
  expect(cache.measure('x'.repeat(1000),measure)).toBe(9000);expect(cache.retainedBytes).toBe(before);
  cache.measure('a',measure);expect(reads).toBe(2);
 });
});
