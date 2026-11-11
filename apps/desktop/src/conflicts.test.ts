import {describe,it,expect} from 'vitest';
import {conflictBlocks,resolveBlock,hasConflictMarkers,buildConflictResult,resolutionRanges,remapResolutionRanges,resolutionLines} from './conflicts';
describe('Git conflict block drafts',()=>{
 it('resolves separate blocks without changing surrounding content',()=>{
  const text='prefix\n<<<<<<< HEAD\ncurrent\n=======\nincoming\n>>>>>>> topic\nmiddle\n<<<<<<< HEAD\nleft\n=======\nright\n>>>>>>> topic\nsuffix\n';
  const blocks=conflictBlocks(text);expect(blocks).toHaveLength(2);
  const one=resolveBlock(text,blocks[0],'both');expect(one).toContain('prefix\ncurrent\nincoming\nmiddle\n<<<<<<< HEAD');expect(one.endsWith('suffix\n')).toBe(true);
  const two=resolveBlock(one,conflictBlocks(one)[0],'incoming');expect(two).toBe('prefix\ncurrent\nincoming\nmiddle\nright\nsuffix\n');expect(conflictBlocks(two)).toHaveLength(0);
 });
 it('excludes diff3 base contents, preserves CRLF and custom marker widths',()=>{
  const text='before\r\n<<<<<<<<< ours\r\nleft\r\n||||||||| base\r\nancestor\r\n=========\r\nright\r\n>>>>>>>>> theirs\r\nafter';
  const block=conflictBlocks(text)[0];expect(block.current).toBe('left\r\n');expect(block.incoming).toBe('right\r\n');expect(resolveBlock(text,block,'current')).toBe('before\r\nleft\r\nafter');expect(resolveBlock(text,block,'both')).not.toContain('ancestor');
 });
 it('does not turn malformed, nested or mismatched markers into choices',()=>{
  for(const text of ['<<<<<<< HEAD\na\n=======\nb\n','<<<<<<< HEAD\na\n========\nb\n>>>>>>> x\n','<<<<<<< a\n<<<<<<< b\nx\n=======\ny\n>>>>>>> b\n>>>>>>> a\n'])expect(conflictBlocks(text)).toHaveLength(0);
  expect(conflictBlocks('text <<<<<<< HEAD\nordinary ======= text\n')).toHaveLength(0);
 });
 it('refuses stale block descriptors and preserves no-final-newline results',()=>{
  const text='<<<<<<< HEAD\nleft\n=======\nright\n>>>>>>> topic';const block=conflictBlocks(text)[0];expect(resolveBlock(text+' moved',block,'both')).toBe(text+' moved');expect(resolveBlock(text,block,'incoming')).toBe('right\n');
 });
 it('identifies incomplete/custom-width markers without treating inline text as a marker',()=>{
  for(const text of ['<<<<<<<<< HEAD\n','>>>>>>> branch\n','||||||||| base\n','<<<<<<<'])expect(hasConflictMarkers(text)).toBe(true);
  for(const text of ['left < right','ordinary ||||||| text','<<<<<<<<expression','seven ======= signs'])expect(hasConflictMarkers(text)).toBe(false);
 });

 it('revises and clears independent choices without losing other blocks or surrounding text',()=>{
  const text='prefix\n<<<<<<< HEAD\na\n=======\nb\n>>>>>>> topic\nmiddle\n<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> topic\nsuffix';
  expect(buildConflictResult(text,['current','incoming'])).toBe('prefix\na\nmiddle\ny\nsuffix');
  expect(buildConflictResult(text,['both','incoming'])).toBe('prefix\na\nb\nmiddle\ny\nsuffix');
  expect(buildConflictResult(text,[null,'incoming'])).toBe('prefix\n<<<<<<< HEAD\na\n=======\nb\n>>>>>>> topic\nmiddle\ny\nsuffix');
  expect(buildConflictResult(text,[])).toBe(text);
 });
 it('keeps deletion choices, diff3 base exclusion and CRLF exact when reconstructing choices',()=>{
  const text='start\r\n<<<<<<<<< HEAD\r\n||||||||| base\r\nold\r\n=========\r\nnew\r\n>>>>>>>>> incoming\r\nend';
  expect(buildConflictResult(text,['current'])).toBe('start\r\nend');
  expect(buildConflictResult(text,['both'])).toBe('start\r\nnew\r\nend');
  expect(buildConflictResult('<<<<<<< broken\n',['incoming'])).toBe('<<<<<<< broken\n');
 });

});

describe('resolved draft highlights',()=>{
 const seed='prefix\r\n<<<<<<< HEAD\r\na\r\n=======\r\nb\r\n>>>>>>> other\r\nmiddle\r\n<<<<<<< HEAD\r\nx\r\n=======\r\ny\r\n>>>>>>> other\r\nsuffix';
 it('locates both resolved blocks in output coordinates, excluding surrounding lines',()=>{
  const choices=['current','both'] as const,result=buildConflictResult(seed,choices),ranges=resolutionRanges(seed,choices);
  expect(ranges.map(r=>result.slice(r.start,r.end))).toEqual(['a\r\n','x\r\ny\r\n']);
  expect(resolutionLines(result,ranges).map(r=>[r.line,r.lines])).toEqual([[1,1],[3,2]]);
  expect(resolutionRanges(seed,[null,'incoming'])).toHaveLength(1);
 });
 it('moves highlights with edits before them and marks edits inside them',()=>{
  const result=buildConflictResult(seed,['current','incoming']),ranges=resolutionRanges(seed,['current','incoming']);
  const moved=remapResolutionRanges(ranges,result,'new\n'+result);
  expect(moved[0].start).toBe(ranges[0].start+4);
  const edited=remapResolutionRanges(ranges,result,result.replace('a\r\n','changed\r\n'));
  expect(edited[0].choice).toBe('edited');expect(edited[1].start).toBe(ranges[1].start+6);
 });
 it('shows a deletion at its output position without including the next block',()=>{
  const text='before\n<<<<<<< HEAD\n=======\nincoming\n>>>>>>> other\nafter';
  const ranges=resolutionRanges(text,['current']);expect(ranges).toEqual([{start:7,end:7,choice:'current'}]);
  expect(resolutionLines(buildConflictResult(text,['current']),ranges)[0]).toMatchObject({line:1,lines:1});
 });
});
