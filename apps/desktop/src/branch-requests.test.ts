import {expect,it} from 'vitest';
import {requestsForRefs,validateBranchRequests} from './branch-requests';
import type {BranchRequests,BranchTracking} from './contracts';
const request={number:12,title:'Release fix',draft:false,url:'https://git.example/team/project/-/merge_requests/12',sourceBranch:'feature/fix',targetBranch:'release',sourceProjectId:'1'};
const snapshot:BranchRequests={provider:'gitlab',origin:'https://git.example',project:'team/project',projectId:'1',remote:'origin',requests:[request],partial:false,checkedAt:1};
it('matches outgoing and incoming by exact remote and upstream, not by display name or commit',()=>{
 expect(requestsForRefs(snapshot,['refs/remotes/origin/feature/fix'],[]).outgoing).toHaveLength(1);
 expect(requestsForRefs(snapshot,['refs/remotes/origin/release'],[]).incoming).toHaveLength(1);
 expect(requestsForRefs(snapshot,['refs/remotes/other/feature/fix','refs/tags/feature/fix','refs/heads/feature/fix'],[]).outgoing).toHaveLength(0);
 const tracking={branch:'local-alias',remote:'origin',remoteBranch:'feature/fix'} as BranchTracking;
 expect(requestsForRefs(snapshot,['refs/heads/local-alias'],[tracking]).outgoing).toHaveLength(1);
 expect(requestsForRefs(snapshot,['refs/heads/local-alias'],[{...tracking,remote:'other'}]).outgoing).toHaveLength(0);
});
it('keeps fork requests incoming only and preserves several destinations',()=>{
 const fork={...snapshot,requests:[{...request,sourceProjectId:'2'}]};
 expect(requestsForRefs(fork,['refs/remotes/origin/feature/fix'],[]).outgoing).toHaveLength(0);
 expect(requestsForRefs(fork,['refs/remotes/origin/release'],[]).incoming).toHaveLength(1);
 expect(requestsForRefs({...snapshot,requests:[request,{...request,number:13,targetBranch:'main'}]},['refs/remotes/origin/feature/fix'],[]).outgoing).toHaveLength(2);
});
it('validates exact provider repository links and bounded responses',()=>{
 expect(validateBranchRequests(snapshot)).toBe(snapshot);
 for(const url of ['https://other.example/team/project/-/merge_requests/12','https://git.example/other/project/-/merge_requests/12','https://git.example/team/project/-/merge_requests/13','https://git.example/team/project/-/merge_requests/12?token=bad'])expect(()=>validateBranchRequests({...snapshot,requests:[{...request,url}]})).toThrow();
 expect(()=>validateBranchRequests({...snapshot,requests:Array(301).fill(request)})).toThrow();
});
