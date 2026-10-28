import tempfile,subprocess,os,json,time,sys
from pathlib import Path
bridge=str(Path(__file__).resolve().parents[3]/'target/debug/examples/ui_bridge')
if sys.platform!='darwin':raise SystemExit('This instrumented benchmark currently targets macOS.')
count=int(sys.argv[1]);label=sys.argv[2]
if not 1<=count<=2000 or not label.replace('_','').replace('-','').isalnum():raise SystemExit('Use 1..2000 files and an alphanumeric label.')
with tempfile.TemporaryDirectory(prefix='gitnova-discard-bench-') as tmp:
 root=Path(tmp);repo=root/'repo';repo.mkdir();bindir=root/'bin';bindir.mkdir();trace=root/'trace';trace.write_text('')
 wrapper=bindir/'git';wrapper.write_text('#!/bin/sh\nprintf ".\\n" >> "$GITNOVA_BENCH_TRACE"\nexec /usr/bin/git "$@"\n');wrapper.chmod(0o700)
 env={**os.environ,'PATH':str(bindir)+':/usr/bin:/bin:/usr/sbin:/sbin','GITNOVA_BENCH_TRACE':str(trace),'GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':'/dev/null'}
 def git(*args):return subprocess.check_output(['/usr/bin/git','-C',str(repo),'-c','user.name=Bench','-c','user.email=bench@example.invalid','-c','commit.gpgsign=false','-c','core.hooksPath=/dev/null',*args],env=env,stderr=subprocess.DEVNULL,text=True).strip()
 def invoke(command,**args):
  p=subprocess.run([bridge,str(root/'state.json')],input=json.dumps({'command':command,'args':args}),capture_output=True,text=True,env=env,timeout=300,check=True)
  r=json.loads(p.stdout)
  if 'error' in r:raise RuntimeError(r['error'])
  return r['ok']
 git('init','-b','main');git('config','core.hooksPath','/dev/null');(repo/'base.txt').write_text('base\n');git('add','.');git('commit','-m','Base');git('switch','-c','incoming');(repo/'artifacts/customer-guide').mkdir(parents=True)
 for i in range(count):(repo/f'artifacts/customer-guide/delivery-guide-{i:04}.txt').write_text('sample content\n'*16)
 git('add','.');git('commit','-m','Incoming');source=git('rev-parse','HEAD');git('switch','main');(repo/'destination.txt').write_text('destination\n');git('add','.');git('commit','-m','Destination')
 cat=invoke('add_repository',path=str(repo));member=cat['workspaces'][0]['selectedId'];args={'membershipId':member}
 p=invoke('preview_merge',**args,request={'operation':'merge','mainline':None,'sourceOid':source,'sourceRef':'refs/heads/incoming','sessionId':''});invoke('begin_merge',**args,request=p['request'],token=p['token'])
 action={'kind':'unstage','path':None,'choice':None,'content':None,'message':''};p=invoke('preview_merge_action',**args,action=action);invoke('apply_merge_action',**args,action=action,token=p['token'])
 changes=invoke('working_changes',**args);state=invoke('merge_state',**args);request={'paths':[f['path'] for f in changes['files']],'scope':'all','id':''}
 results={'label':label,'files':count,'phases':{}}
 for phase in ['preview','apply']:
  trace.write_text('');start=time.monotonic()
  if phase=='preview':p=invoke('preview_operation_discard',**args,request=request,operationToken=state['token'])
  else:
   r=invoke('discard_operation_changes',**args,request=p['request'],token=p['token'],operationToken=state['token']);assert r['status']=='pending',r
  results['phases'][phase]={'seconds':round(time.monotonic()-start,3),'gitProcesses':len(trace.read_text().splitlines())}
 assert all(not (repo/f'artifacts/customer-guide/delivery-guide-{i:04}.txt').exists() for i in range(count))
 print(json.dumps(results));Path('/tmp/gitnova-bench-'+label+'.json').write_text(json.dumps(results,indent=2)+'\n')
