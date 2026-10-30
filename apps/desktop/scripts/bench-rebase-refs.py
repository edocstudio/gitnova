#!/usr/bin/env python3
"""Isolated old/new publication-query comparison; no user repository writes."""
import argparse, datetime, json, os, pathlib, statistics, subprocess, tempfile, time

def main():
    p=argparse.ArgumentParser();p.add_argument('--output',required=True);a=p.parse_args()
    env={k:v for k,v in os.environ.items() if not k.startswith('GIT_')}
    env.update(GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL=os.devnull,LC_ALL='C')
    with tempfile.TemporaryDirectory(prefix='gitnova-ref-perf-') as temp:
        root=pathlib.Path(temp)
        def git(args,data=None):
            return subprocess.check_output(['git','-C',str(root),*args],input=data,env=env,stderr=subprocess.PIPE)
        git(['init','-b','main']);stream=[]
        for i in range(100):
            text=f'revision {i}\n';stream.append(f'commit refs/heads/main\ncommitter Fixture <fixture@example.invalid> {1700000000+i} +0000\ndata {len(text)}\n{text}\n')
        git(['fast-import','--quiet'],(''.join(stream)+'done\n').encode())
        commits=git(['rev-list','--reverse','main']).decode().splitlines()
        for i,oid in enumerate(commits[::10]):git(['update-ref',f'refs/remotes/fixture/branch-{i}',oid])
        before=[];after=[]
        for _ in range(5):
            start=time.monotonic();old=set()
            for oid in commits:old.update(git(['for-each-ref',f'--contains={oid}','--format=%(refname)','refs/remotes']).decode().splitlines())
            before.append((time.monotonic()-start)*1000)
            start=time.monotonic();new=set(git(['for-each-ref','--format=%(refname)',*[f'--contains={o}' for o in commits],'refs/remotes']).decode().splitlines());after.append((time.monotonic()-start)*1000)
            assert old==new and len(new)==10
    report={'schemaVersion':1,'kind':'gitnova-rebase-publication-query','measuredAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'fixture':{'commits':100,'remoteRefs':10},'equivalentResults':True,'rounds':5,'before':{'gitCallsPerRound':100,'samplesMs':before,'medianMs':statistics.median(before)},'after':{'gitCallsPerRound':1,'samplesMs':after,'medianMs':statistics.median(after)},'limitations':['Publication-reference query only, not complete rebase or a cross-product benchmark.','Synthetic temporary local repositories, warm filesystem caches, fixed before/after order.','No credentials or network; no Git configuration is persisted outside the fixture.'],'reference':'https://git-scm.com/docs/git-for-each-ref#_notes'}
    pathlib.Path(a.output).write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
if __name__=='__main__':main()
