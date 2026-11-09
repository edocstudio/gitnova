import type {Parser} from '@lezer/common';
import {highlightTree,tagHighlighter,tags} from '@lezer/highlight';
import {EDIT_SECTION_LIMIT} from './working-file-text';

export type SyntaxLanguage='javascript'|'typescript'|'jsx'|'tsx'|'python'|'rust'|'json'|'yaml'|'html'|'css';
const extensions:Record<string,SyntaxLanguage>={js:'javascript',mjs:'javascript',cjs:'javascript',ts:'typescript',jsx:'jsx',tsx:'tsx',py:'python',rs:'rust',json:'json',yaml:'yaml',yml:'yaml',html:'html',htm:'html',css:'css'};
export function syntaxLanguage(path:string):SyntaxLanguage|null{return extensions[path.split('.').at(-1)?.toLowerCase()??'']??null;}
export async function loadSyntaxParser(language:SyntaxLanguage):Promise<Parser>{
  switch(language){
    case 'javascript':case 'typescript':case 'jsx':case 'tsx':{
      const {parser}=await import('@lezer/javascript');
      return parser.configure({dialect:language==='tsx'?'ts jsx':language==='typescript'?'ts':language==='jsx'?'jsx':''});
    }
    case 'python':return (await import('@lezer/python')).parser;
    case 'rust':return (await import('@lezer/rust')).parser;
    case 'json':return (await import('@lezer/json')).parser;
    case 'yaml':return (await import('@lezer/yaml')).parser;
    case 'html':return (await import('@lezer/html')).parser;
    case 'css':return (await import('@lezer/css')).parser;
  }
}
const palette=tagHighlighter([
  {tag:tags.keyword,class:'syntax-keyword'},
  {tag:tags.comment,class:'syntax-comment'},
  {tag:[tags.string,tags.regexp],class:'syntax-string'},
  {tag:[tags.number,tags.bool,tags.null],class:'syntax-literal'},
  {tag:[tags.propertyName,tags.attributeName],class:'syntax-property'},
  {tag:[tags.typeName,tags.className,tags.tagName],class:'syntax-type'},
  {tag:tags.function(tags.variableName),class:'syntax-function'},
]);
export type SyntaxToken={from:number;to:number;className:string};
// Bound both parser input and styled DOM nodes. Coloring is never required to edit/save.
export function syntaxTokens(text:string,parser:Parser|null):SyntaxToken[]{
  if(!parser||text.length>EDIT_SECTION_LIMIT)return [];
  const result:SyntaxToken[]=[];
  try{
    highlightTree(parser.parse(text),palette,(from,to,className)=>{
      if(result.length>=4096)throw new Error('Highlight node budget');
      result.push({from,to,className});
    });
    return result;
  }catch{return [];}
}
