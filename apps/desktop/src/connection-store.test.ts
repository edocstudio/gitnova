import {expect,it,vi,afterEach} from 'vitest';
import {connections,parseConnections,normalizeHost,saveConnections,CONNECTIONS_KEY} from './connection-store';
afterEach(()=>vi.unstubAllGlobals());
it('normalizes host identities and rejects credentials, query and non-HTTPS addresses',()=>{
 expect(normalizeHost('https://GIT.company.example/gitlab/')).toBe('https://git.company.example/gitlab');
 for(const url of ['http://git.example','https://user:secret@git.example','https://git.example?token=secret','file:///tmp','https://git.example/#secret'])expect(()=>normalizeHost(url)).toThrow();
});
it('bounds stored metadata and never imports credential fields into profiles',()=>{
 const result=parseConnections(JSON.stringify([{id:'a',label:'Company A',provider:'gitlab',origin:'https://git.a.example',enabled:true,token:'secret',jenkins:{controller:'https://ci.a.example',username:'alice',password:'secret'}}]));
 expect(JSON.stringify(result)).not.toContain('secret');expect(result[0].jenkins?.username).toBe('alice');expect(parseConnections(JSON.stringify(Array(21).fill(result[0])))).toEqual([]);expect(parseConnections('{')).toEqual([]);
});
it('promotes explicit legacy hosts without deleting repo settings and removal remains authoritative',()=>{
 const values:Record<string,string>={'gitnova.ci.gitlab.repo-a':JSON.stringify({remote:'origin',origin:'https://git.a.example'}),'gitnova.ci.gitlab.repo-b':JSON.stringify({remote:'origin',origin:'https://git.a.example'}),'gitnova.ci.gitlab.unknown':JSON.stringify({remote:'origin',origin:''}),'gitnova.ci.jenkins.repo-a':JSON.stringify({controller:'https://ci.a.example',username:'alice',jobUrl:'https://ci.a.example/job/project/'})};
 const storage={...values,getItem:(k:string)=>values[k]??null,setItem:(k:string,v:string)=>{values[k]=v;}};vi.stubGlobal('localStorage',storage);vi.stubGlobal('window',{dispatchEvent:vi.fn()});
 const migrated=connections();expect(migrated).toHaveLength(1);expect(migrated[0].jenkins?.controller).toBe('https://ci.a.example');expect(values['gitnova.ci.gitlab.repo-a']).toBeDefined();
 saveConnections([]);expect(values[CONNECTIONS_KEY]).toBe('[]');expect(connections()).toEqual([]);
});
