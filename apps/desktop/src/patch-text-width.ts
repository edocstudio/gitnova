// One tiny measuring surface; retain a bounded LRU of immutable text widths.
// This is a cache bound, not a bound on WebKit or the active diff's memory.
export class TextWidthCache {
  private entries=new Map<string,number>();
  private bytes=0;
  constructor(private readonly budget=512*1024){}
  get retainedBytes(){return this.bytes;}
  measure(text:string,read:(text:string)=>number):number {
    const prior=this.entries.get(text);
    if(prior!==undefined){this.entries.delete(text);this.entries.set(text,prior);return prior;}
    const width=read(text),weight=64+text.length*2;
    if(weight>this.budget)return width;
    while(this.bytes+weight>this.budget&&this.entries.size){const oldest=this.entries.keys().next().value!;this.bytes-=64+oldest.length*2;this.entries.delete(oldest);}
    this.entries.set(text,width);this.bytes+=weight;return width;
  }
}
const cache=new TextWidthCache();
let context:CanvasRenderingContext2D|null|undefined;
export function patchTextWidth(value:string):number {
  if(!value)return 0;
  const text=value.replace(/\t/g,'    ');
  return cache.measure(text,line=>{
    if(context===undefined){const canvas=document.createElement('canvas');canvas.width=1;canvas.height=1;context=canvas.getContext('2d');if(context)context.font='12px "SFMono-Regular", Consolas, monospace';}
    return context?.measureText(line).width??line.length*8;
  });
}
