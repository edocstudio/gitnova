#!/usr/bin/env python3
"""Serial isolated native/WebKit soak. Changes only newly created fixture repos.
Requires an already-built debug bundle and ui_bridge; never builds during sampling.
"""
import argparse, datetime, hashlib, json, os, pathlib, statistics, subprocess, tempfile, time, shutil, re, plistlib
ROOT=pathlib.Path(__file__).resolve().parents[3]
ENV={**{k:v for k,v in os.environ.items() if not k.startswith('GIT_')},'GIT_CONFIG_GLOBAL':'/dev/null','GIT_CONFIG_NOSYSTEM':'1','GIT_OPTIONAL_LOCKS':'0','LC_ALL':'C'}
def run(args,cwd=None,**kw):
 return subprocess.run(args,cwd=cwd,env=ENV,check=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=120,**kw).stdout
def git(path,*args):return run(['git','--no-optional-locks','-C',str(path),'-c','user.name=Soak Fixture','-c','user.email=soak@example.invalid','-c','commit.gpgsign=false','-c','core.hooksPath=/dev/null','-c','gc.auto=0','-c','pack.threads=1',*args])
def seed(path,commits,files):
 path.mkdir();git(path,'init','-b','main')
 proc=subprocess.Popen(['git','-C',str(path),'fast-import','--quiet'],env=ENV,stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
 try:
  for i in range(commits):
   message=f'Performance needle {i}: review repository history and bounded cache behavior\n'.encode();content=f'revision {i}\n'.encode()
   proc.stdin.write(f'commit refs/heads/main\nmark :{i+1}\ncommitter Soak Fixture <soak@example.invalid> {1700000000+i} +0000\ndata {len(message)}\n'.encode()+message)
   if i==0:
    for f in range(files):
     value=(f'fixture file {f}\n'+'context line\n'*12).encode();proc.stdin.write(f'M 100644 inline files/{f//100:03}/file-{f}.txt\ndata {len(value)}\n'.encode()+value+b'\n')
   proc.stdin.write(f'M 100644 inline history.txt\ndata {len(content)}\n'.encode()+content+b'\n\n')
  proc.stdin.write(b'done\n');proc.stdin.close();error=proc.stderr.read();assert proc.wait(timeout=120)==0,error.decode()
 finally:
  if proc.poll() is None:proc.kill();proc.wait()
 git(path,'reset','--hard','HEAD');git(path,'config','user.name','Soak Fixture');git(path,'config','user.email','soak@example.invalid');git(path,'config','commit.gpgsign','false');git(path,'config','core.hooksPath','/dev/null');git(path,'config','gc.auto','0');git(path,'config','pack.threads','1')
def fixture(folder,commits,files,small):
 large=folder/'large-a';seed(large,commits,files)
 child=folder/'child-source';seed(child,100,10)
 git(large,'-c','protocol.file.allow=always','submodule','add',str(child),'child-one');git(large,'-c','protocol.file.allow=always','submodule','add',str(child),'child-two');git(large,'commit','-am','Record two submodules')
 for i in range(10):
  git(large,'checkout','-b',f'merged-feature-{i}');(large/f'feature-{i}.txt').write_text(f'feature content {i}\n');git(large,'add','.');git(large,'commit','-m',f'Performance needle feature {i}');git(large,'checkout','main');git(large,'merge','--no-ff',f'merged-feature-{i}','-m',f'Performance needle merge {i}')
 for i in range(min(1000,files)):
  path=large/f'files/{i//100:03}/file-{i}.txt';path.write_text(path.read_text()+'bulk review context '*25+'\n')
 (large/'bulk.bin').write_bytes(b'\x00fixture binary\xff'*131072)
 git(large,'add','.');git(large,'commit','-m','Performance needle: review a thousand-file change')
 large_b=folder/'large-b';git(folder,'clone','--shared',str(large),str(large_b));git(large_b,'branch','soak-switch')
 git(large,'remote','add','origin',str(large_b))
 heads=git(large,'rev-list','--max-count=200','HEAD').decode().splitlines();commands=[]
 for i,oid in enumerate(heads):
  commands+= [f'update refs/heads/fixture-{i:03} {heads[0]}',f'update refs/remotes/origin/fixture-{i:03} {oid}']
 run(['git','-C',str(large),'update-ref','--stdin'],input=('\n'.join(commands)+'\n').encode())
 # Configure tracking in one native config write per branch outside measurement.
 config=large/'.git/config'
 with config.open('a') as out:
  for i in range(len(heads)):out.write(f'\n[branch "fixture-{i:03}"]\n remote = origin\n merge = refs/heads/fixture-{i:03}\n')
 medium=folder/'medium-0';seed(medium,small,max(100,files//10));repos=[large,large_b,medium]
 for i in range(1,6):
  path=folder/f'medium-{i}';git(folder,'clone','--shared',str(medium),str(path));repos.append(path)
 for i in range(2):
  path=folder/f'linked-{i}';git(large,'worktree','add','-b',f'linked-{i}',str(path));repos.append(path)
 # Exercise the working-file diff path without any soak-time Git write.
 for i in range(min(20,files)):
  path=large/f'files/{i//100:03}/file-{i}.txt';path.write_text(path.read_text()+'local fixture edit\n')
 for repo in repos:
  git(repo,'config','core.hooksPath','/dev/null');git(repo,'config','core.attributesFile','/dev/null');git(repo,'config','gc.auto','0');git(repo,'config','pack.threads','1')
 return repos

def processes():
 raw=run(['/bin/ps','-A','-o','pid=,ppid=,rss=,pcpu=,comm=']).decode();result={}
 for line in raw.splitlines():
  fields=line.split(None,4)
  if len(fields)!=5:continue
  pid,parent,rss,cpu,command=fields
  result[int(pid)]={'parent':int(parent),'rss_kib':int(rss),'cpu_percent':float(cpu),'webkit':'com.apple.WebKit.' in command,'git':pathlib.Path(command).name in ('git','git-remote-http','git-remote-https','ssh')}
 return result

def snapshot(repos):
 result={}
 for repo in repos:
  index=git(repo,'rev-parse','--path-format=absolute','--git-path','index').decode().strip()
  result[repo.name]=(git(repo,'rev-parse','HEAD'),git(repo,'symbolic-ref','HEAD'),hashlib.sha256(git(repo,'ls-files','--stage','-z')).hexdigest(),git(repo,'status','--porcelain=v1','-z','--untracked-files=all'),hashlib.sha256(git(repo,'show-ref')).hexdigest(),hashlib.sha256(pathlib.Path(index).read_bytes()).hexdigest())
 return result

def slope(samples,key):
 rows=[(s['elapsed_seconds']/60,s[key]/1024) for s in samples]
 if len(rows)<2:return None
 x=statistics.mean(r[0] for r in rows);y=statistics.mean(r[1] for r in rows);den=sum((r[0]-x)**2 for r in rows)
 return sum((a-x)*(b-y) for a,b in rows)/den if den else None

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--seconds',type=int,default=1800);parser.add_argument('--commits',type=int,default=50000);parser.add_argument('--files',type=int,default=10000);parser.add_argument('--small-commits',type=int,default=5000);parser.add_argument('--output',required=True);args=parser.parse_args();assert 60<=args.seconds<=7200
 bundle=ROOT/'target/debug/bundle/macos/GitNova.app/Contents/MacOS/gitnova-desktop';bridge=ROOT/'target/debug/examples/ui_bridge';assert bundle.exists() and bridge.exists()
 if any('gitnova-desktop' in row for row in run(['/bin/ps','-A','-o','comm=']).decode().splitlines()):raise RuntimeError('Close the existing GitNova before the isolated soak; it will not be closed automatically.')
 folder=pathlib.Path(tempfile.mkdtemp(prefix='gitnova-native-smoke-'));folder.chmod(0o700);print(f'Preparing isolated fixtures: {args.commits} commits, {args.files} tracked files, 10 locations',flush=True)
 child=None;caffeine=None;passed=False;output=pathlib.Path(args.output).resolve();output.parent.mkdir(parents=True,exist_ok=True);samples=[];ui=[];failure=None
 try:
  if shutil.disk_usage(folder).free<4*1024**3:raise RuntimeError('Less than 4 GiB free for bounded fixtures/build margin')
  repos=fixture(folder,args.commits,args.files,args.small_commits);state=folder/'state.json'
  for repo in repos:
   response=json.loads(run([str(bridge),str(state)],input=json.dumps({'command':'add_repository','args':{'path':str(repo)}}).encode()));assert 'error' not in response,response
  baseline=snapshot(repos);before=processes();original_webkit={pid for pid,v in before.items() if v['webkit']};owned_webkit=set();log=(folder/'native.log').open('wb');start=time.monotonic();last_print=0;free_percent=None
  child=subprocess.Popen([str(bundle)],cwd='/tmp',env={**ENV,'GITNOVA_NATIVE_SMOKE':'1','GITNOVA_SMOKE_STATE':str(state),'GITNOVA_NATIVE_SOAK_SECONDS':str(args.seconds)},stdin=subprocess.DEVNULL,stdout=log,stderr=log)
  caffeine=subprocess.Popen(['/usr/bin/caffeinate','-i','-w',str(child.pid)],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  while time.monotonic()-start<args.seconds+180:
   elapsed=time.monotonic()-start
   if child.poll() is not None:raise RuntimeError(f'Native application exited: {child.returncode}')
   ps=processes();desc={child.pid};changed=True
   while changed:
    added={pid for pid,v in ps.items() if v['parent'] in desc};changed=not added.issubset(desc);desc|=added
   owned_webkit|={pid for pid,v in ps.items() if v['webkit'] and pid not in original_webkit}
   native=ps.get(child.pid,{}).get('rss_kib',0);webkit=sum(ps[p]['rss_kib'] for p in owned_webkit if p in ps and ps[p]['webkit']);git_rss=sum(ps[p]['rss_kib'] for p in desc if p in ps and ps[p]['git']);total=native+webkit+git_rss
   samples.append({'elapsed_seconds':round(elapsed,2),'native_rss_kib':native,'webkit_rss_kib':webkit,'sampled_git_rss_kib':git_rss,'sampled_total_rss_kib':total,'cpu_percent':round(sum(ps[p]['cpu_percent'] for p in desc|owned_webkit if p in ps and (p in desc or ps[p]['webkit'])),1)})
   events=folder/'ui-soak.jsonl';ui=[json.loads(v) for v in events.read_text().splitlines()] if events.exists() else []
   if total>1536*1024:raise RuntimeError('Resource guard: sampled native/WebKit/Git RSS exceeds 1.5 GiB')
   if ui and ui[-1]['kind']=='failed':raise RuntimeError(ui[-1].get('error','UI driver failure'))
   if ui and ui[-1]['kind']=='complete':passed=True;break
   if elapsed-last_print>=30:
    pressure=run(['/usr/bin/memory_pressure','-Q']).decode();match=re.search(r'free percentage: (\d+)%',pressure);free_percent=int(match.group(1)) if match else None
    if free_percent is not None and free_percent<8:raise RuntimeError('Resource guard: system memory free percentage below 8%')
    samples[-1]['system_memory_free_percent']=free_percent
    print(f'Soak {elapsed:.0f}s: native {native/1024:.1f} MiB, WebKit {webkit/1024:.1f} MiB, sampled total {total/1024:.1f} MiB; cycles {sum(v["kind"]=="cycle" for v in ui)}',flush=True);last_print=elapsed
   time.sleep(2)
  if not passed:raise RuntimeError('Soak deadline exceeded')
  after=snapshot(repos)
  for name,value in baseline.items():
   assert after[name][:5]==value[:5],f'Soak changed fixture HEAD/branch/staging/refs/working state: {name}'
   if name!='large-b':assert after[name][5]==value[5],f'Read-only fixture index changed: {name}'
 except Exception as error:failure=str(error);passed=False
 finally:
  owned_git=set()
  if child and child.poll() is None:
   ps=processes();desc={child.pid}
   for _ in range(8):desc|={pid for pid,v in ps.items() if v['parent'] in desc}
   owned_git={pid for pid in desc if pid in ps and ps[pid]['git']}
   child.terminate()
   try:child.wait(timeout=10)
   except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
  if caffeine:
   try:caffeine.wait(timeout=5)
   except subprocess.TimeoutExpired:caffeine.terminate();caffeine.wait(timeout=5)
  metrics={}
  for event in ui:
   for key,values in event.get('metrics',{}).items():metrics.setdefault(key,[]).extend(values)
  summary={key:{'samples':len(values),'median_ms':statistics.median(values),'p95_ms':sorted(values)[min(len(values)-1,int(len(values)*.95))],'max_ms':max(values)} for key,values in metrics.items() if values}
  warm=[s for s in samples if s['elapsed_seconds']>=300];last=[s for s in samples if s['elapsed_seconds']>=max(0,args.seconds-300)]
  report={'schemaVersion':1,'kind':'gitnova-native-soak','measuredAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sourceBaseCommit':run(['git','rev-parse','HEAD'],cwd=ROOT).decode().strip(),'sourceHasUncommittedChanges':bool(run(['git','status','--porcelain'],cwd=ROOT).strip()),'binarySha256':hashlib.sha256(bundle.read_bytes()).hexdigest(),'passed':passed,'failure':failure,'requested_seconds':args.seconds,'sampled_seconds':samples[-1]['elapsed_seconds'] if samples else 0,'fixture':{'large_commits':args.commits,'large_tracked_files':args.files,'medium_commits':args.small_commits,'locations':10,'linked_worktrees':2,'submodules':2,'tracking_branches':200,'merge_commits':10},'profile':'packaged debug app with optimized first-party crates, real background WKWebView, scripted manual refresh/interaction; not foreground rendering or GPU/JS-heap measurement; no provider/network or cross-product comparison','attribution':'Native RSS is exact PID. WebKit RSS includes new WebKit helpers absent before launch; XPC parentage cannot exclude unrelated helpers launched during the interval. Git RSS is sampled descendants only and can miss short-lived peaks. Summed RSS may double-count shared pages; it is not JS heap or total GPU allocation.','observed_webkit_visibility':sorted({v['visibility'] for v in ui if 'visibility' in v}),'idle_sleep_guard':'Temporary task-owned caffeinate -i tied to native PID; no display/lock settings changed','latencies':summary,'memory':{'peak_native_mib':max((s['native_rss_kib']/1024 for s in samples),default=0),'peak_sampled_total_mib':max((s['sampled_total_rss_kib']/1024 for s in samples),default=0),'native_warm_slope_mib_per_minute':slope(warm,'native_rss_kib'),'webkit_warm_slope_mib_per_minute':slope(warm,'webkit_rss_kib'),'last_5min_native_median_mib':statistics.median(s['native_rss_kib']/1024 for s in last) if last else None},'cycles':sum(v['kind']=='cycle' for v in ui),'max_dom_nodes':max((v.get('dom_nodes',0) for v in ui),default=0),'max_visible_rows':max((v.get('visible_rows',0) for v in ui),default=0),'samples':samples,'ui_events':ui}
  output.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k not in ('samples','ui_events')},indent=2),flush=True)
  for _ in range(10):
   live=processes();owned_git={pid for pid in owned_git if pid in live and live[pid]['git']}
   if not owned_git:break
   time.sleep(1)
  if passed and not owned_git:shutil.rmtree(folder)
  elif passed:print(f'Fixture retained until task-owned Git readers exit: {folder}',flush=True)
  else:print(f'Failed fixture retained: {folder}',flush=True)
 if not passed:raise SystemExit(1)
if __name__=='__main__':main()
