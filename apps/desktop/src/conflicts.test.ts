import {describe,it,expect} from 'vitest';
import {conflictBlocks,resolveBlock,hasConflictMarkers,buildConflictResult} from './conflicts';
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
