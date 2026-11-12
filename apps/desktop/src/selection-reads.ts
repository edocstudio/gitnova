// Only replaceable, read-only selection work uses these lanes. No cached DTOs,
// write previews, mutations, network requests or credentials are retained here.
export type ReadTiming = {lane:string; queueMs:number; executionMs:number; started:boolean; superseded:boolean};
type Job = {start:()=>Promise<void>; cancel:()=>void};
type Lane = {active:boolean; pending?:Job};
export class SelectionReads {
  private lanes = new Map<string,Lane>();
  constructor(private readonly observe?: (sample:ReadTiming)=>void) {}
  private async launch(lane:string,state:Lane,job:Job):Promise<void> {
    state.active=true;
    try {await job.start();}
    finally {
      const next=state.pending;state.pending=undefined;
      if(next)void this.launch(lane,state,next);
      else {state.active=false;this.lanes.delete(lane);}
    }
  }
  run<T>(lane:string,read:()=>Promise<T>,signal?:AbortSignal):Promise<T> {
    const state=this.lanes.get(lane)??{active:false};this.lanes.set(lane,state);
    const queued=performance.now();
    return new Promise<T>((resolve,reject)=>{
      let cancelled=false,started=false,finished=false;
      const cleanup=()=>{finished=true;signal?.removeEventListener('abort',abort);};
      const cancel=()=>{
        if(finished)return;
        cancelled=true;reject(new DOMException('Selection changed','AbortError'));
        if(!started){cleanup();this.observe?.({lane,queueMs:performance.now()-queued,executionMs:0,started:false,superseded:true});}
      };
      const abort=()=>{cancel();if(state.pending===job)state.pending=undefined;};
      const job:Job={cancel,start:async()=>{
        if(cancelled || signal?.aborted){cancel();return;}
        started=true;const start=performance.now();
        try {const value=await read();if(!cancelled)resolve(value);}
        catch(error){if(!cancelled)reject(error);}
        finally {cleanup();this.observe?.({lane,queueMs:start-queued,executionMs:performance.now()-start,started:true,superseded:cancelled});}
      }};
      if(signal?.aborted){cancel();if(!state.active)this.lanes.delete(lane);return;}
      signal?.addEventListener('abort',abort,{once:true});
      if(state.active){state.pending?.cancel();state.pending=job;}
      else void this.launch(lane,state,job);
    });
  }
}
