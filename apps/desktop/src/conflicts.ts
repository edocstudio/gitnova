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

export type ConflictChoice = 'current'|'incoming'|'both'|null;
// Build from the stable draft snapshot, so a checkbox can undo/revise its own
// choice without losing untouched content or choices made in other blocks.
export function buildConflictResult(text:string,choices:readonly ConflictChoice[]):string {
 const blocks=conflictBlocks(text);let offset=0;const pieces:string[]=[];
 blocks.forEach((block,index)=>{
  pieces.push(text.slice(offset,block.start));const choice=choices[index];
  pieces.push(choice==='current'?block.current:choice==='incoming'?block.incoming:choice==='both'?block.current+block.incoming:text.slice(block.start,block.end));offset=block.end;
 });
 pieces.push(text.slice(offset));return pieces.join('');
}

export function hasConflictMarkers(text:string):boolean {
 return text.split('\n').some(line=>/^(?:<{7,}|>{7,}|\|{7,})(?:\s|$)/.test(line));
}

// Draft-only ranges. They never change the text sent to Git.
export type ResolutionRange = {start:number; end:number; choice:Exclude<ConflictChoice,null>|'edited'};
export function resolutionRanges(text:string,choices:readonly ConflictChoice[]):ResolutionRange[] {
 let delta=0;
 const ranges:ResolutionRange[]=[];
 for(const [index,block] of conflictBlocks(text).entries()) {
  const choice=choices[index];if(!choice)continue;
  const length=(choice==='current'?block.current:choice==='incoming'?block.incoming:block.current+block.incoming).length;
  ranges.push({start:block.start+delta,end:block.start+delta+length,choice});
  delta+=length-(block.end-block.start);
 }
 return ranges;
}
// Track a contiguous editor change, including typing/paste/undo before a range.
export function remapResolutionRanges(ranges:readonly ResolutionRange[],before:string,after:string):ResolutionRange[] {
 if(ranges.length===0)return [];
 if(before===after)return [...ranges];
 let start=0,oldEnd=before.length,newEnd=after.length;
 while(start<oldEnd&&start<newEnd&&before[start]===after[start])start++;
 while(oldEnd>start&&newEnd>start&&before[oldEnd-1]===after[newEnd-1]){oldEnd--;newEnd--;}
 const delta=newEnd-oldEnd;
 return ranges.map(range=>range.end<=start?range:range.start>=oldEnd?{...range,start:range.start+delta,end:range.end+delta}:{start:Math.min(range.start,start),end:Math.max(newEnd,range.end+delta),choice:'edited'});
}
export function resolutionLines(text:string,ranges:readonly ResolutionRange[]) {
 if(ranges.length===0)return [];
 const starts=[0];for(let i=0;i<text.length;i++)if(text[i]==='\n')starts.push(i+1);
 const lineAt=(offset:number)=>{let lo=0,hi=starts.length;while(lo+1<hi){const mid=(lo+hi)>>>1;if(starts[mid]<=offset)lo=mid;else hi=mid;}return lo;};
 return ranges.map(range=>({...range,line:lineAt(range.start),lines:lineAt(Math.max(range.start,range.end-1))-lineAt(range.start)+1}));
}
