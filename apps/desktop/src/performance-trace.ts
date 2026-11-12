// Explicit opt-in diagnostics for controlled measurements. Never enabled by a
// user preference, never persisted/uploaded, and never records request arguments.
export type PerformanceSample={kind:'ipc'|'selection'|'usable';name:string;milliseconds:number;queueMs?:number;started?:boolean;superseded?:boolean};
let enabled=false;
const samples:PerformanceSample[]=[];
export function enablePerformanceTrace(value:boolean){enabled=value;samples.length=0;}
export function performanceSamples(){return samples.slice();}
export function recordPerformance(sample:PerformanceSample){
  if(!enabled)return;
  if(samples.length===512)samples.shift();
  samples.push(sample);
}
export function traceUsable(name:string,start:number,current:()=>boolean){
  if(!enabled)return;
  requestAnimationFrame(()=>{requestAnimationFrame(()=>{if(current())recordPerformance({kind:'usable',name,milliseconds:performance.now()-start});});});
}
