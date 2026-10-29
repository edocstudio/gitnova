import {it,expect} from 'vitest';
import {FetchSchedule,parseFetchInterval} from './auto-fetch';
it('defaults to one minute and validates persisted choices',()=>{expect(parseFetchInterval(null)).toBe(60);expect(parseFetchInterval('bad')).toBe(60);expect(parseFetchInterval('1')).toBe(60);expect(parseFetchInterval('0')).toBe(0);expect(parseFetchInterval('30')).toBe(30);});
it('keeps per-repository cooldown and backs failures off to fifteen minutes',()=>{
 const schedule=new FetchSchedule();expect(schedule.due('a',1000,60)).toBe(61000);expect(schedule.due('a',50000,60)).toBe(61000);expect(schedule.due('b',50000,60)).toBe(110000);
 schedule.finish('a',61000,60,true);expect(schedule.due('a',61000,60)).toBe(181000);
 for(let i=0;i<8;i++)schedule.finish('a',200000,60,true);expect(schedule.due('a',200000,60)).toBe(1100000);
 schedule.finish('a',1100000,60);expect(schedule.due('a',1100000,60)).toBe(1160000);schedule.reset();expect(schedule.due('a',0,30)).toBe(30000);
});
it('evicts older repository schedules instead of growing without bound',()=>{const schedule=new FetchSchedule();schedule.due('old',0,60);for(let n=0;n<32;n++)schedule.due(String(n),1000,60);expect(schedule.due('old',5000,60)).toBe(65000);});
