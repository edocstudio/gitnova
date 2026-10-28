import {afterEach,describe,expect,it,vi} from 'vitest';
import {initialTheme,setTheme} from './theme';
afterEach(()=>vi.unstubAllGlobals());
describe('device-wide theme preference',()=>{
 it('defaults to dark for new, invalid or unavailable storage',()=>{
  expect(initialTheme(null)).toBe('dark');expect(initialTheme({getItem:()=> 'light'})).toBe('light');expect(initialTheme({getItem:()=> 'system'})).toBe('dark');expect(initialTheme({getItem:()=>{throw new Error('blocked');}})).toBe('dark');
 });
 it('applies immediately and remains usable when persistence is blocked',()=>{
  const document={documentElement:{dataset:{} as Record<string,string>,style:{colorScheme:''}}};vi.stubGlobal('document',document);vi.stubGlobal('localStorage',{setItem:()=>{throw new Error('blocked');}});
  expect(()=>setTheme('light')).not.toThrow();expect(document.documentElement.dataset.theme).toBe('light');expect(document.documentElement.style.colorScheme).toBe('light');setTheme('dark');
 });
});
