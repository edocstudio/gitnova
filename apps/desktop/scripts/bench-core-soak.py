#!/usr/bin/env python3
"""Bounded persistent Rust-core soak. Requires an already-built soak_history.
No build, native app launch, credentials or existing project repositories.
"""
import argparse, collections, datetime, hashlib, json, math, os, pathlib, platform, queue, statistics, subprocess, threading, time
ROOT = pathlib.Path(__file__).resolve().parents[3]
def percentile(values, p):
    values=sorted(values)
    return values[min(len(values)-1,math.ceil(len(values)*p)-1)] if values else None

def main():
    p=argparse.ArgumentParser();p.add_argument('--seconds',type=int,default=1800);p.add_argument('--commits',type=int,default=50000);p.add_argument('--files',type=int,default=10000);p.add_argument('--output',required=True);a=p.parse_args()
    if not 60<=a.seconds<=7200 or not 100<=a.commits<=100000 or not 100<=a.files<=20000:p.error('Fixture/duration out of bounds')
    binary=ROOT/'target/debug/examples/soak_history';assert binary.is_file(),'Build soak_history first; this runner does not build.'
    out=pathlib.Path(a.output).resolve();out.parent.mkdir(parents=True,exist_ok=True)
    events=queue.Queue(maxsize=4096);memory=[];timings=collections.defaultdict(lambda:collections.deque(maxlen=2048));counts=collections.Counter();cold={};partial=collections.Counter();ready=None;complete=None;failure=None
    env={k:v for k,v in os.environ.items() if not k.startswith('GIT_')}
    env.update({'GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':os.devnull,'LC_ALL':'C'})
    stderr=out.with_suffix('.stderr.log').open('w');child=subprocess.Popen([str(binary),str(a.seconds),str(a.commits),str(a.files)],env=env,stdout=subprocess.PIPE,stderr=stderr,text=True)
    def read():
        try:
            for line in child.stdout:events.put(json.loads(line))
        except Exception as error:events.put({'event':'readerError','error':str(error)})
    reader=threading.Thread(target=read,daemon=True);reader.start();start=time.monotonic();next_sample=0;next_print=0
    try:
        while child.poll() is None or not events.empty():
            now=time.monotonic()-start
            if now>a.seconds+600:raise RuntimeError('Fixture/soak deadline exceeded')
            try:
                event=events.get(timeout=.25)
                if event['event']=='ready':ready=event;print(json.dumps(event),flush=True)
                elif event['event']=='measurement':
                    value=event['milliseconds']
                    if not isinstance(value,(int,float)) or not math.isfinite(value) or value<0:raise ValueError('Invalid latency')
                    key=event['scenario']
                    if counts[key]==0:cold[key]=value
                    else:timings[key].append(value)
                    counts[key]+=1;partial[key]+=int(event['partial'])
                elif event['event']=='complete':complete=event
                else:raise RuntimeError(event.get('error','Unknown benchmark event'))
            except queue.Empty:pass
            if now>=next_sample:
                raw=subprocess.check_output(['/bin/ps','-A','-o','pid=,ppid=,rss='],text=True,timeout=5)
                ps={int(v[0]):(int(v[1]),int(v[2])) for line in raw.splitlines() if len(v:=line.split())==3}
                owned={child.pid}
                for _ in range(16):
                    expanded=owned|{pid for pid,(parent,_) in ps.items() if parent in owned}
                    if expanded==owned:break
                    owned=expanded
                core=ps.get(child.pid,(0,0))[1];desc=sum(ps.get(pid,(0,0))[1] for pid in owned if pid!=child.pid)
                memory.append({'elapsedSeconds':round(now,3),'phase':'workload' if ready else 'fixture','coreRssMiB':core/1024,'sampledDescendantRssMiB':desc/1024,'sampledTotalRssMiB':(core+desc)/1024})
                if (core+desc)>1024*1024:raise RuntimeError('Resource guard: owned sampled RSS exceeds 1 GiB')
                next_sample=now+5
            if now>=next_print:
                if platform.system()=='Darwin':
                    pressure=subprocess.check_output(['/usr/bin/memory_pressure','-Q'],text=True,timeout=10)
                    import re
                    m=re.search(r'free percentage: (\d+)%',pressure)
                    if m and int(m.group(1))<8:raise RuntimeError('Resource guard: system free memory below 8%')
                print(f'Elapsed {now:.0f}s; core RSS {memory[-1]["coreRssMiB"]:.1f} MiB; measured operations {sum(counts.values())}',flush=True);next_print=now+30
        if child.wait()!=0 or not complete:raise RuntimeError('Core soak did not complete; inspect stderr artifact')
    except Exception as error:
        failure=str(error)
    finally:
        if child.poll() is None:child.terminate()
        try:child.wait(timeout=25)
        except subprocess.TimeoutExpired:child.kill();child.wait()
        reader.join(timeout=5);stderr.close()
    valid=[m for m in memory if m['coreRssMiB']>0 and m['phase']=='workload']
    windows=[]
    for begin in range(0,int(time.monotonic()-start)+1,300):
        values=[v['coreRssMiB'] for v in valid if begin<=v['elapsedSeconds']<begin+300]
        if values:windows.append({'startSeconds':begin,'sampleCount':len(values),'medianCoreRssMiB':statistics.median(values)})
    report={'schemaVersion':1,'kind':'gitnova-core-soak','profile':'local debug, first-party CPU crates optimized at level 2','measuredAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sourceBaseCommit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True,env=env).strip(),'sourceHasUncommittedChanges':bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True,env=env).strip()),'platform':platform.system(),'architecture':platform.machine(),'binarySha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'fixture':ready,'completion':complete,'passed':failure is None,'failure':failure,'wallSeconds':time.monotonic()-start,'memory':{'sampleIntervalSeconds':5,'samples':memory,'windows':windows,'peakCoreRssMiB':max((v['coreRssMiB'] for v in valid),default=0),'peakSampledTotalRssMiB':max((v['sampledTotalRssMiB'] for v in valid),default=0)},'latency':{k:{'observations':counts[k],'coldSampleMs':cold[k],'retainedTailSamples':len(v),'partialObservations':partial[k],'medianMs':statistics.median(v) if v else None,'p95Ms':percentile(v,.95),'samplesMs':list(v)} for k,v in timings.items()},'limitations':['Rust core and sampled descendant processes only; excludes WebKit and native UI.','Synthetic local repositories; excludes network, provider credentials and write/recovery workloads.','RSS samples can miss short-lived peaks and do not prove absence of leaks.','Tail latency samples are capped at 2,048 per scenario; observation counts include the full run.','No before/after or cross-product comparison; elapsed run is not a full-workday acceptance test.','Other running applications can influence elapsed latency.']}
    out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k not in ('memory','latency')},indent=2),flush=True)
    if failure:raise SystemExit(1)
if __name__=='__main__':main()
