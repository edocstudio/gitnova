// Offsets preserve untouched bytes/line endings. Only complete Git marker
// groups are actionable; diff3/zdiff3 base text never becomes result content.
export type ConflictBlock = {start:number; end:number; current:string; incoming:string};
export function conflictBlocks(text:string):ConflictBlock[] {
 const lines=Array.from(text.matchAll(/[^\n]*\n|[^\n]+$/g),m=>({text:m[0].replace(/\r?\n$/,''),start:m.index!,end:m.index!+m[0].length}));
 const blocks:ConflictBlock[]=[];
 for(let i=0;i<lines.length;i++) {
  const opening=/^(<{7,})(?: .*)?$/.exec(lines[i].text);if(!opening)continue;
  const width=opening[1].length,start=i;let base=-1,separator=-1,end=-1,invalid=false;
  for(let j=i+1;j<lines.length;j++) {
   const line=lines[j].text;
   if(/^<{7,}(?: .*)?$/.test(line)){invalid=true;break;}
   if(new RegExp(`^\\|{${width}}(?: .*)?$`).test(line)){if(base>=0||separator>=0){invalid=true;break;}base=j;}
   if(line==='='.repeat(width)){if(separator>=0){invalid=true;break;}separator=j;}
   if(new RegExp(`^>{${width}}(?: .*)?$`).test(line)){end=j;break;}
  }
  if(invalid)return [];
  if(separator<0||end<0||base>=separator)continue;
  blocks.push({start:lines[start].start,end:lines[end].end,current:text.slice(lines[start].end,lines[base<0?separator:base].start),incoming:text.slice(lines[separator].end,lines[end].start)});i=end;
 }
 return blocks;
}
export function resolveBlock(text:string,block:ConflictBlock,choice:'current'|'incoming'|'both'):string {
 // Refuse a stale block descriptor instead of replacing unrelated text.
 if(!conflictBlocks(text).some(b=>b.start===block.start&&b.end===block.end&&b.current===block.current&&b.incoming===block.incoming))return text;
 return text.slice(0,block.start)+(choice==='current'?block.current:choice==='incoming'?block.incoming:block.current+block.incoming)+text.slice(block.end);
}

export function hasConflictMarkers(text:string):boolean {
 return text.split('\n').some(line=>/^(?:<{7,}|>{7,}|\|{7,})(?:\s|$)/.test(line));
}
