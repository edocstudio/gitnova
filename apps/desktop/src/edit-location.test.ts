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

it('keeps exact UTF-16 offsets for large repeated text and CRLF context at section boundaries',()=>{
  const text='value\r\n'.repeat(600000);
  const anchor={line:300001,text:'value',before:['value','value','value'],after:['value','value','value']};
  expect(editorOffset(text,anchor)).toBe(300000*7);
  expect(editorOffset('😀\r\nภาษาไทย\r\ntail',{line:2,text:'ภาษาไทย',before:['😀'],after:['tail']})).toBe(4);
});

it('matches a complete candidate search across repeated, shifted and missing context',()=>{
  let seed=17112026;
  const random=(n:number)=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed%n;};
  const choices=['','same','value','other','ภาษาไทย','😀','  '];
  for(let example=0;example<400;example++){
    const original=Array.from({length:5+random(40)},()=>choices[random(choices.length)]);
    const target=random(original.length),start=Math.max(0,target-3);
    const anchor={line:target+1,text:original[target],before:original.slice(start,target),after:original.slice(target+1,target+4)};
    const current=[...original];current.splice(random(current.length),random(4),...Array.from({length:random(5)},()=>choices[random(choices.length)]));
    const text=current.join(example%2?'\r\n':'\n'),lines=text.split('\n');
    const normalized=lines.map(line=>line.replace(/\r$/,''));
    const expected=Math.min(lines.length-1,anchor.line-1);
    let best=expected,bestScore=0;
    for(let candidate=0;candidate<lines.length;candidate++){
      let score=anchor.text.trim()&&normalized[candidate]===anchor.text?2:0;
      anchor.before.forEach((value,i)=>{if(value.trim()&&normalized[candidate+i-anchor.before.length]===value)score+=2;});
      anchor.after.forEach((value,i)=>{if(value.trim()&&normalized[candidate+i+1]===value)score+=2;});
      if(score>=2&&(score>bestScore||score===bestScore&&Math.abs(candidate-expected)<Math.abs(best-expected))){best=candidate;bestScore=score;}
    }
    const offset=lines.slice(0,best).reduce((sum,line)=>sum+line.length+1,0);
    expect(editorOffset(text,anchor),`case ${example}`).toBe(offset);
  }
});
