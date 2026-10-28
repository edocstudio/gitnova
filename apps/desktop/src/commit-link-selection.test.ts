import {describe,it,expect} from 'vitest';
import {commitLinkSelection as select} from './commit-link-selection';
const oid='1234567890abcdef1234567890abcdef12345678';
const data={remotes:['origin','upstream'],branches:[],urls:{origin:['git@gitlab.com:team/nested/project.git'],upstream:['https://github.com/other/project.git']}};
describe('direct commit link destination',()=>{
 it('uses the fetch upstream before origin and preserves the exact selected SHA',()=>{
  expect(select(data,'upstream',oid,[])).toMatchObject({remote:'upstream',oid,provider:'github',origin:'https://github.com'});
  expect(select(data,null,oid,[])).toMatchObject({remote:'origin',oid,provider:'gitlab'});
 });
 it('uses global profiles for private hosts without inheriting a different project URL',()=>{
  const privateRemote={remotes:['origin'],branches:[],urls:{origin:['ssh://git@git.example.com:2222/team/nested/project.git']}};
  expect(select(privateRemote,null,oid,[{provider:'gitlab',origin:'https://git.example.com'}])).toMatchObject({provider:'gitlab',origin:'https://git.example.com',oid});
  expect(()=>select(privateRemote,null,oid,[])).toThrow('Settings');
  expect(()=>select(privateRemote,null,oid,[{provider:'gitlab',origin:'https://git.example.com'},{provider:'github',origin:'https://git.example.com'}])).toThrow('More than one');
 });
 it('matches HTTPS ports and base paths, rejects lookalike hosts and ambiguous remotes',()=>{
  const profiles=[{provider:'gitlab' as const,origin:'https://git.example.com/gitlab'}];
  const remote=(url:string)=>({remotes:['team'],branches:[],urls:{team:[url]}});
  expect(select(remote('https://git.example.com/gitlab/team/repo.git'),null,oid,profiles).provider).toBe('gitlab');
  for(const url of ['https://git.example.com/gitlab-other/repo.git','https://git.example.com:8443/gitlab/team/repo.git','https://git.example.com.evil.invalid/gitlab/team/repo.git'])expect(()=>select(remote(url),null,oid,profiles)).toThrow('Settings');
  expect(()=>select({remotes:['a','b'],branches:[]},null,oid,[])).toThrow('ambiguous');
  expect(()=>select({remotes:[],branches:[]},null,oid,[])).toThrow('No configured');
  expect(()=>select(remote('/tmp/repository'),null,oid,[])).toThrow();
  expect(()=>select(remote('https://token:password@git.example.com/repo.git'),null,oid,profiles)).toThrow();
 });
});
