import {expect,it} from 'vitest';
import {ciActions} from './ci-actions';
import type {PipelineSnapshot,PipelineJob} from './contracts';
const oid='a'.repeat(40),origin='https://git.example/base';
const job=(id:string,allowFailure?:boolean|null,status='manual'):PipelineJob=>({id,name:`deploy-${id}`,stage:'deploy',status,allowFailure,runner:null,duration:null});
const snapshot=(jobs:PipelineJob[],id='41'):PipelineSnapshot=>({oid,origin,project:'team/project',checkedAt:100,partial:false,runs:[{id,sha:oid,reference:'release',source:'push',status:'manual',url:`${origin}/team/project/-/pipelines/${id}`,jobs,jobsPartial:false}]});
it('distinguishes required, optional and absent manual evidence without treating all manual jobs as approvals',()=>{
 const result=ciActions(snapshot([job('1',false),job('2',true),job('3')]),oid,'release');
 expect(result.items.map(item=>[item.jobId,item.kind])).toEqual([['1','required'],['3','unknown'],['2','optional']]);
 expect(result.partial).toBe(false);
 expect(ciActions(snapshot([job('1',null)]),oid,'release').items[0].kind).toBe('unknown');
});
it('uses only the newest matching branch pipeline and retains no history or duplicate refresh events',()=>{
 const latest=snapshot([job('3',false)],'100'),older=snapshot([job('4',false)],'99');
 latest.runs.push(older.runs[0],{...snapshot([job('5',false)],'101').runs[0],reference:'other'});
 const first=ciActions(latest,oid,'release'),again=ciActions(latest,oid,'release',first);
 expect(again.items).toEqual(first.items);expect(first.items).toHaveLength(1);expect(first.pipelineId).toBe('100');
 expect(ciActions(latest,oid,'main').items).toHaveLength(0);
 expect(ciActions(latest,oid,null).pipelineId).toBe('101');
});
it('removes observed completed jobs and replaces superseded pipelines while preserving unseen partial-page evidence',()=>{
 const previous=ciActions(snapshot([job('1',false),job('2',true)]),oid,'release');
 const partial=snapshot([job('1',false,'running')]);partial.runs[0].jobsPartial=true;
 const next=ciActions(partial,oid,'release',previous);
 expect(next.partial).toBe(true);expect(next.items).toEqual(expect.arrayContaining([expect.objectContaining({jobId:'2',stale:true})]));
 expect(next.items.some(item=>item.jobId==='1')).toBe(false);
 const completed=snapshot([job('1',false,'success'),job('2',true,'running')]);completed.runs[0].status='running';
 expect(ciActions(completed,oid,'release',previous).items).toHaveLength(0);
 expect(ciActions(snapshot([job('3',true)],'42'),oid,'release',previous).items.map(item=>item.jobId)).toEqual(['3']);
});
it('keeps an unresolved list when a bounded run list cannot establish absence',()=>{
 const previous=ciActions(snapshot([job('1',false)]),oid,'release');
 const missing={...snapshot([]),runs:[],partial:true};
 expect(ciActions(missing,oid,'release',previous).items[0].stale).toBe(true);
 expect(ciActions({...missing,partial:false},oid,'release',previous).items).toEqual([]);
 expect(ciActions(missing,oid,'other',previous).items).toEqual([]);
});
it('shows pipeline-level blocked/manual evidence without inventing jobs or duplicating known required work',()=>{
 const blocked=snapshot([job('1',true)]);blocked.runs[0].status='blocked';
 expect(ciActions(blocked,oid,'release').items.map(item=>item.kind)).toEqual(['blocked','optional']);
 blocked.runs[0].jobs=[job('1',false)];expect(ciActions(blocked,oid,'release').items).toHaveLength(1);
 expect(ciActions(snapshot([]),oid,'release').items[0]).toMatchObject({jobId:null,kind:'unknown'});
});
it('rejects wrong SHA, duplicate job identity, credentials and cross-project/host pipeline links',()=>{
 const bad=snapshot([job('1')]);
 expect(()=>ciActions({...bad,oid:'b'.repeat(40)},oid,'release')).toThrow();
 expect(()=>ciActions({...bad,runs:[{...bad.runs[0],sha:'b'.repeat(40)}]},oid,'release')).toThrow();
 expect(()=>ciActions(snapshot([job('1'),job('1')]),oid,'release')).toThrow();
 for(const url of ['https://other.example/base/team/project/-/pipelines/41',`${origin}/other/project/-/pipelines/41`,`${origin}/team/project/-/pipelines/42`,`${origin}/team/project/-/pipelines/41?token=private`,'https://user:secret@git.example/base/team/project/-/pipelines/41']){
  expect(()=>ciActions({...bad,runs:[{...bad.runs[0],url}]},oid,'release')).toThrow();
 }
});

it('does not treat optional manual jobs in an otherwise successful pipeline as failure or required approval',()=>{
 const value=snapshot([job('1',true)]);value.runs[0].status='success';
 expect(ciActions(value,oid,'release').items).toEqual([expect.objectContaining({kind:'optional',jobId:'1',stale:false})]);
});
