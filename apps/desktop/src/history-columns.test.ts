import {describe,it,expect} from 'vitest';
import {parseHistoryWidths,historyColumnLayout,columnWidth,columnLimits} from './history-columns';
describe('history column layout',()=>{
 it('recovers from malformed storage and bounds known dimensions',()=>{
   for(const raw of [null,'bad','[]','null','42'])expect(parseHistoryWidths(raw)).toEqual({});
   expect(parseHistoryWidths('{"refs":9000,"message":-1,"author":"100","unexpected":12}')).toEqual({refs:600,message:180});
 });
 it('keeps all lanes inside a compact graph without shrinking below legibility',()=>{
   expect(historyColumnLayout({},88).values.graph).toBe(88);
   const layout=historyColumnLayout({},984);
   expect(layout.values.graph).toBeLessThan(984);
   expect(layout.values.graph).toBeGreaterThanOrEqual(columnLimits('graph',984)[0]);
   expect(columnWidth('graph',1,984)).toBe(columnLimits('graph',984)[0]);
 });
 it('retains independently sized tracks and reserves a minimum table width for horizontal scrolling',()=>{
   const layout=historyColumnLayout({refs:200,graph:120,message:420,ci:180,author:160,date:100},88);
   expect(layout.template).toBe('200px 120px 420px 180px 160px 100px minmax(0,1fr)');
   expect(layout.minWidth).toBe(1194);
   expect(historyColumnLayout({},88).template).toContain('minmax(180px,1fr)');
 });
});
