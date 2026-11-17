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
  const lines=text.split('\n');
  const expected=Math.max(0,Math.min(lines.length-1,anchor.line-1));
  let best=expected,bestScore=0;
  const seeds=[...anchor.before.map((value,i)=>({value,delta:i-anchor.before.length})),
    {value:anchor.text,delta:0},...anchor.after.map((value,i)=>({value,delta:i+1}))].filter(seed=>seed.value.trim());
  for(let i=0;i<lines.length;i++){
    const value=lines[i].replace(/\r$/,'');
    for(const seed of seeds){
      if(value!==seed.value)continue;
      const candidate=i-seed.delta;
      if(candidate<0||candidate>=lines.length)continue;
      let score=lines[candidate].replace(/\r$/,'')===anchor.text&&anchor.text.trim()?2:0;
      for(const other of seeds){
        if(other.delta!==0&&lines[candidate+other.delta]?.replace(/\r$/,'')===other.value)score+=2;
      }
      if(score<2)continue;
      if(score>bestScore||(score===bestScore&&Math.abs(candidate-expected)<Math.abs(best-expected))){best=candidate;bestScore=score;}
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
