import {afterEach,expect,it,vi} from 'vitest';
import {storedMapping} from './kustomize-images';

afterEach(()=>vi.unstubAllGlobals());
it('keeps existing branch, container and custom overlay paths when adding SIT to a 1.8 mapping',()=>{
  const legacy={deployId:'infra',reference:'refs/remotes/origin/main',container:'worker',production:'overlays/prd',uat:'region/uat'};
  vi.stubGlobal('localStorage',{getItem:()=>JSON.stringify(legacy)});
  expect(storedMapping('app')).toEqual({deployId:'infra',reference:legacy.reference,container:'worker',
    overlays:{sit:'overlays/sit',production:'overlays/prd',uat:'region/uat'}});
});

it('retains legacy PRD identity and isolates inherited overlays and overrides per workspace',async()=>{
  const {releaseProfile,saveProfile}=await import('./release-profile');
  const {effectiveMapping,saveMapping}=await import('./kustomize-images');
  const data=new Map<string,string>([['gitnova.release.environments','["SIT","PRD"]']]);
  vi.stubGlobal('localStorage',{getItem:(k:string)=>data.get(k)??null,setItem:(k:string,v:string)=>data.set(k,v)});
  vi.stubGlobal('window',new EventTarget());
  const member={id:'one',repositoryId:'app',worktreeId:null,name:'worker',pinned:false};
  const a={id:'a',name:'A',selectedId:null,memberships:[member]},b={...a,id:'b'};
  const profile=releaseProfile(a.id);
  expect(profile.production).toBe('prd');
  profile.images=true;
  profile.environments[1].name='Live';
  saveProfile(a.id,profile);
  const mapping=effectiveMapping(a,'app')!;
  expect(mapping.container).toBe('worker');
  expect(mapping.overlays.prd).toBe('overlays/prd');
  saveMapping(a,'app',{...mapping,overlays:{...mapping.overlays,prd:'region/live'}});
  profile.environments[0].overlay='region/test';
  saveProfile(a.id,profile);
  expect(effectiveMapping(a,'app')?.overlays).toEqual({sit:'region/test',prd:'region/live'});
  expect(releaseProfile(b.id).environments[1].name).toBe('PRD');
  expect(effectiveMapping(b,'app')).toBeNull();
  saveMapping(a,'app','manual');
  expect(effectiveMapping(a,'app')).toBeNull();
  saveMapping(a,'app','inherit');
  expect(effectiveMapping(a,'app')?.overlays.prd).toBe('overlays/prd');
});
