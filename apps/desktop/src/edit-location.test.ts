import {expect,it} from 'vitest';
import {parseDiff} from './diff';
import {diffEditAnchor,editorOffset,editorSectionPosition} from './edit-location';
import {splitEditorText} from './working-file-text';

const patch=(body:string)=>parseDiff(`diff --git a/file b/file\n--- a/file\n+++ b/file\n${body}`)[0];

it('targets the visible changed hunk and follows its text when working lines shift',()=>{
  const file=patch('@@ -1 +1 @@\n-old\n+first\n@@ -5099,5 +5099,6 @@\n before\n-old entry\n+fixed entry\n+new entry\n after\n tail\n');
  const anchor=diffEditAnchor(file,file.lines.slice(3));
  expect(anchor?.line).toBe(5100);
  const text='prefix\r\n'.repeat(5300)+'before\r\nfixed entry\r\nnew entry\r\nafter\r\ntail\r\n';
  const offset=editorOffset(text,anchor);
  expect(text.slice(offset)).toMatch(/^fixed entry\r\n/);
  const sections=splitEditorText(text),position=editorSectionPosition(sections,offset);
  expect(position.section).toBeGreaterThan(0);
  expect(sections[position.section].slice(position.offset)).toMatch(/^fixed entry\r\n/);
});

it('uses surrounding lines to disambiguate repeated text and edited target lines',()=>{
  const anchor={line:2,text:'duplicate',before:['correct before'],after:['correct after']};
  const text='wrong before\nduplicate\nwrong after\ncorrect before\nduplicate\ncorrect after\n';
  expect(editorOffset(text,anchor)).toBe(text.lastIndexOf('duplicate'));
  const edited=text.replace('before\nduplicate\ncorrect','before\nlocal edit\ncorrect');
  expect(editorOffset(edited,anchor)).toBe(edited.indexOf('local edit'));
});

it('maps deletions to surviving new-side lines, including EOF and an empty file',()=>{
  const middle=patch('@@ -40,3 +40,2 @@\n before\n-deleted\n after\n');
  expect(diffEditAnchor(middle,[middle.lines[2]])?.line).toBe(41);
  const end=patch('@@ -40,2 +40 @@\n before\n-deleted\n');
  expect(diffEditAnchor(end,[end.lines[2]])?.line).toBe(40);
  const empty=patch('@@ -1 +0,0 @@\n-last line\n');
  expect(editorOffset('',diffEditAnchor(empty,empty.lines))).toBe(0);
});

it('falls back to the nearest existing line without an anchor match',()=>{
  expect(editorOffset('one\ntwo', {line:5102,text:'missing',before:[],after:[]})).toBe(4);
  expect(editorOffset('one\ntwo', {line:2,text:'missing',before:['one'],after:[]})).toBe(4);
  expect(editorOffset('one\ntwo',null)).toBe(0);
  expect(editorSectionPosition(['abc\n','def'],4)).toEqual({section:1,offset:0});
  expect(editorSectionPosition(['abc\n','def'],99)).toEqual({section:1,offset:3});
});

it('uses visible context when the changed rows have scrolled out of view',()=>{
  const file=patch('@@ -8,3 +8,3 @@\n-old\n+new\n context\n end\n');
  expect(diffEditAnchor(file,[file.lines[3]])?.line).toBe(9);
});
