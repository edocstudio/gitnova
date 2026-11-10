import {it,expect,vi} from 'vitest';
import {parseTerminalIdle,setTerminalIdle,TERMINAL_IDLE_KEY} from './terminal-idle';
it('defaults to no automatic closure and accepts only supported choices',()=>{
 for(const value of [null,'bad','-1','1','Infinity'])expect(parseTerminalIdle(value)).toBe(0);
 for(const value of ['0','30','60'])expect(parseTerminalIdle(value)).toBe(Number(value));
});
it('persists the device preference without accepting unsupported timeouts',()=>{
 const data=new Map<string,string>();vi.stubGlobal('localStorage',{getItem:(key:string)=>data.get(key)??null,setItem:(key:string,value:string)=>data.set(key,value)});
 setTerminalIdle(30);expect(localStorage.getItem(TERMINAL_IDLE_KEY)).toBe('30');
 setTerminalIdle(15);expect(localStorage.getItem(TERMINAL_IDLE_KEY)).toBe('30');
 setTerminalIdle(0);expect(localStorage.getItem(TERMINAL_IDLE_KEY)).toBe('0');vi.unstubAllGlobals();
});
