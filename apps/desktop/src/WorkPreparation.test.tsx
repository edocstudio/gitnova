import {expect, it} from 'vitest';
import {needsWorkPreparation} from './WorkPreparation';
it('routes only parent uncommitted work to stash, never child repositories or active operations',()=>{
  expect(needsWorkPreparation('Commit or preserve your uncommitted changes with Git before switching. Nothing will be discarded or stashed automatically.')).toBe(true);
  expect(needsWorkPreparation('Commit or preserve uncommitted work before starting this operation. GitNova will not stash or discard it automatically.')).toBe(true);
  for(const reason of [null, 'Submodule child contains uncommitted work. Preserve it before switching.', 'Finish or recover the current GitNova operation before starting another.', 'Another Git operation holds an index or HEAD lock.', 'Select the mainline parent used as the change baseline.'])expect(needsWorkPreparation(reason)).toBe(false);
});
