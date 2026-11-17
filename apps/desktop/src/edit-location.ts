import type {DiffLine, PatchFile} from './diff';

export type EditAnchor = {line:number; text:string; before:string[]; after:string[]};

// Prefer a changed row in the actual viewport, excluding virtual overscan rows.
// Removed rows map to the next surviving line (or the preceding line at EOF).
export function diffEditAnchor(file:PatchFile, visible:DiffLine[]):EditAnchor|null {
  const selected=visible.find(line=>line.kind==='add'||line.kind==='remove')
    ??visible.find(line=>line.next!==null)
    ??file.lines.find(line=>line.kind==='add'||line.kind==='remove'||line.next!==null);
  if(!selected)return null;
  let index=file.lines.indexOf(selected);
  if(selected.next===null){
    let following=index+1;
    while(following<file.lines.length&&file.lines[following].kind!=='hunk'&&file.lines[following].next===null)following++;
    if(following<file.lines.length&&file.lines[following].next!==null)index=following;
    else {while(index>=0&&file.lines[index].next===null&&file.lines[index].kind!=='hunk')index--;}
  }
  const target=file.lines[index];
  if(!target||target.next===null){
    const header=file.lines.slice(0,file.lines.indexOf(selected)+1).reverse().find(line=>line.kind==='hunk');
    const line=Number(/\+(\d+)/.exec(header?.text??'')?.[1]??1);
    return {line:Math.max(1,line),text:'',before:[],after:[]};
  }
  const before:string[]=[],after:string[]=[];
  for(let i=index-1;i>=0&&before.length<3&&file.lines[i].kind!=='hunk';i--){
    if(file.lines[i].next!==null)before.unshift(file.lines[i].text.replace(/\r$/,''));
  }
  for(let i=index+1;i<file.lines.length&&after.length<3&&file.lines[i].kind!=='hunk';i++){
    if(file.lines[i].next!==null)after.push(file.lines[i].text.replace(/\r$/,''));
  }
  return {line:target.next,text:target.text.replace(/\r$/,''),before,after};
}

// Historical line numbers can drift in the working file. Match bounded context,
// break duplicate-text ties by distance, and fall back to a clamped line number.
export function editorOffset(text:string,anchor:EditAnchor|null):number {
  if(!anchor)return 0;
  const clean=(line:string)=>line.endsWith('\r')?line.slice(0,-1):line;
  const contexts=[...anchor.before.map((value,i)=>({value,delta:i-anchor.before.length})),
    ...anchor.after.map((value,i)=>({value,delta:i+1}))].filter(seed=>seed.value.trim());
  const target=Boolean(anchor.text.trim());
  const maximum=(contexts.length+Number(target))*2;
  // Common path: walk only to the expected line, then inspect bounded context.
  // A perfect match here is optimal and closest; no whole-file split is needed.
  let expectedOffset=0;
  for(let line=1;line<anchor.line;line++){
    const end=text.indexOf('\n',expectedOffset);
    if(end<0)break;
    expectedOffset=end+1;
  }
  if(!maximum)return expectedOffset;
  const at=(offset:number)=>{const end=text.indexOf('\n',offset);return clean(text.slice(offset,end<0?text.length:end));};
  let perfect=!target||at(expectedOffset)===anchor.text;
  for(const seed of contexts){
    let offset=expectedOffset;
    for(let i=0;i<Math.abs(seed.delta)&&offset>=0;i++){
      if(seed.delta<0)offset=offset===0?-1:text.lastIndexOf('\n',offset-2)+1;
      else {const end=text.indexOf('\n',offset);offset=end<0?-1:end+1;}
    }
    if(offset<0||at(offset)!==seed.value){perfect=false;break;}
  }
  if(perfect)return expectedOffset;
  // Normalize once and score each candidate once, nearest first. This replaces
  // repeated seed/candidate rescoring and regex allocation on duplicate lines.
  if(!(target&&text.includes(anchor.text))&&!contexts.some(seed=>text.includes(seed.value)))return expectedOffset;
  const lines=text.split('\n');
  const normalized=lines.map(clean);
  const expected=Math.max(0,Math.min(lines.length-1,anchor.line-1));
  let best=expected,bestScore=0;
  const score=(index:number)=>{
    let value=target&&normalized[index]===anchor.text?2:0;
    for(const seed of contexts)if(normalized[index+seed.delta]===seed.value)value+=2;
    return value;
  };
  search:for(let distance=0;distance<Math.max(expected+1,lines.length-expected);distance++){
    const candidates=distance?[expected-distance,expected+distance]:[expected];
    for(const candidate of candidates){
      if(candidate<0||candidate>=lines.length)continue;
      const value=score(candidate);
      if(value>=2&&value>bestScore){best=candidate;bestScore=value;}
      if(value===maximum)break search;
    }
  }
  let offset=0;
  for(let i=0;i<best;i++)offset+=lines[i].length+1;
  return offset;
}

export function editorSectionPosition(sections:string[],offset:number):{section:number;offset:number} {
  let remaining=Math.max(0,offset);
  for(let section=0;section<sections.length;section++){
    if(remaining<sections[section].length||section===sections.length-1)
      return {section,offset:Math.min(remaining,sections[section].length)};
    remaining-=sections[section].length;
  }
  return {section:0,offset:0};
}
