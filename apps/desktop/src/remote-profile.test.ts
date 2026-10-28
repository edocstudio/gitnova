import {it,expect} from 'vitest';
import {profileRepositoryUrl} from './remote-profile';
const profile={id:'a',label:'Company',provider:'gitlab' as const,origin:'https://git.example/gitlab',enabled:false};
it('requires an explicit project and preserves self-hosted base paths without credentials',()=>{
 expect(profileRepositoryUrl(profile,'group/sub/project')).toBe('https://git.example/gitlab/group/sub/project.git');
 expect(profileRepositoryUrl(profile,'group/project.git')).toBe('https://git.example/gitlab/group/project.git');
 for(const path of ['project','../project','group/../project','group//project','https://other.example/project','group/project?token=x','group/project#x'])expect(()=>profileRepositoryUrl(profile,path)).toThrow();
});
