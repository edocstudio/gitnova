import {expect,it} from 'vitest';
import {loadSyntaxParser,syntaxLanguage,syntaxTokens} from './editor-syntax';
import type {SyntaxLanguage} from './editor-syntax';
import {EDIT_SECTION_LIMIT} from './working-file-text';

it('detects supported file extensions and leaves unknown formats as text',()=>{
  expect(syntaxLanguage('folder/Pulumi.SIT.YAML')).toBe('yaml');
  expect(syntaxLanguage('src/example.tsx')).toBe('tsx');
  expect(syntaxLanguage('file.txt')).toBeNull();expect(syntaxLanguage('Makefile')).toBeNull();
});
it('colors real parser tokens without changing or interpreting user text',async()=>{
  const samples:Partial<Record<SyntaxLanguage,string>>={yaml:'enabled: true\nname: "ภาษาไทย 😀" # comment\n',json:'{"name":"<script>alert(1)</script>","count":42}',typescript:'export const count: number = 42;',tsx:'const view = <div title="test">hi</div>;',python:'def hello():\n  return "hi" # comment\n',rust:'fn main() { let value = "hi"; }',css:'body { color: red; }',html:'<div title="hello">text</div>'};
  for(const [name,text] of Object.entries(samples)){
    const parser=await loadSyntaxParser(name as SyntaxLanguage);const tokens=syntaxTokens(text,parser);
    expect(tokens.length,name).toBeGreaterThan(0);
    let end=0;const parts=tokens.flatMap(token=>{expect(token.from).toBeGreaterThanOrEqual(end);expect(token.to).toBeLessThanOrEqual(text.length);const previous=end;end=token.to;return [text.slice(previous,token.from),text.slice(token.from,token.to)];});
    expect(parts.join('')+text.slice(end)).toBe(text);
  }
});
it('falls back to exact plain text on oversized input or excessive token density',async()=>{
  const parser=await loadSyntaxParser('json');
  expect(syntaxTokens('x'.repeat(EDIT_SECTION_LIMIT+1),parser)).toEqual([]);
  expect(syntaxTokens('['+'1,'.repeat(6000)+'0]',parser)).toEqual([]);
  expect(syntaxTokens('anything',null)).toEqual([]);
});
