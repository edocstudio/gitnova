import {afterEach,expect,it,vi} from 'vitest';
import {storedMapping} from './kustomize-images';

afterEach(()=>vi.unstubAllGlobals());
it('keeps existing branch, container and custom overlay paths when adding SIT to a 1.8 mapping',()=>{
  const legacy={deployId:'infra',reference:'refs/remotes/origin/main',container:'worker',production:'overlays/prd',uat:'region/uat'};
  vi.stubGlobal('localStorage',{getItem:()=>JSON.stringify(legacy)});
  expect(storedMapping('app')).toEqual({deployId:'infra',reference:legacy.reference,container:'worker',
    overlays:{sit:'overlays/sit',production:'overlays/prd',uat:'region/uat'}});
});
