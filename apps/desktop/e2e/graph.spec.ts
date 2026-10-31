// The browser transport is emulated. Every Git result comes from the real Rust core
// and temporary Git repositories; this does not certify the native webview transport.
import type { Catalog } from '../src/contracts';
import { test, expect, type Page } from '@playwright/test';
import { execFileSync, execFile, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, renameSync, rmSync, chmodSync, existsSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { promisify } from 'node:util';
const exec = promisify(execFile);
const root = resolve('../..');
const bridge = join(root, 'target/debug/examples', process.platform === 'win32' ? 'ui_bridge.exe' : 'ui_bridge');
let folder: string, repository: string, other: string;
const longFile = (version: string) => Array.from({ length: 100 }, (_, i) => `export const entry${i} = "${version}-${i === 0 ? 'very-long-content-'.repeat(90) : 'value'}";`).join('\n') + '\n';
function git(path: string, args: string[]) {
  const options = { encoding: 'utf8' as const, env: { ...process.env, GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: process.platform === 'win32' ? 'NUL' : '/dev/null' } };
  const output = execFileSync('git', ['-C', path, '-c', 'user.name=GitNova Test', '-c', 'user.email=test@example.invalid', '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', ...args], options).trim();
  // Command-local -c identity is not inherited by application subprocesses.
  // Give only newly created disposable checkouts a persistent synthetic identity.
  const created = args[0] === 'init' && !args.includes('--bare') ? path : args[0] === 'clone' ? resolve(path, args[args.length - 1]) : null;
  if (created) {
    if (!resolve(created).startsWith(resolve(folder) + (process.platform === 'win32' ? '\\' : '/'))) throw new Error('Fixture configuration escaped temporary root');
    for (const [key, value] of [['user.name','GitNova Test'],['user.email','test@example.invalid'],['commit.gpgsign','false'],['core.autocrlf','false']]) execFileSync('git', ['-C', created, 'config', '--local', key, value], options);
  }
  return output;
}
test.beforeAll(() => {
  folder = mkdtempSync(join(tmpdir(), 'gitnova-ui-')); repository = join(folder, 'project-api'); other = join(folder, 'project-web');
  mkdirSync(join(folder, 'empty-home'));
  for (const repo of [repository, other]) {
    mkdirSync(repo); git(repo, ['init', '-b', 'main']);
    mkdirSync(join(repo, 'src/auth'), { recursive: true });
    writeFileSync(join(repo, 'README.md'), '# Project\n');
    writeFileSync(join(repo, 'src/auth/long-lines.ts'), longFile('before'));
    writeFileSync(join(repo, 'src/auth/token.ts'), 'export const tokenLifetime = 900;\n\nexport function refreshSession(expired: boolean) {\n  if (expired) return null;\n  return { cached: false };\n}\n');
    git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Create project']);
  }
  git(repository, ['checkout', '-b', 'fix/session-cache']); writeFileSync(join(repository, 'src/auth/token.ts'), 'export const tokenLifetime = 3600;\n\nexport function refreshSession(expired: boolean) {\n  if (expired) {\n    throw new Error(\'Session expired\');\n  }\n  return { cached: true };\n}\n');
  writeFileSync(join(repository, 'src/auth/long-lines.ts'), longFile('after'));
  writeFileSync(join(repository, 'src/auth/cache.ts'), 'export const sessionCache = new Map<string, string>();\n');
  mkdirSync(join(repository, 'tests')); writeFileSync(join(repository, 'tests/auth.test.ts'), '// Regression: expired sessions must preserve the existing cache.\n'); git(repository, ['add', '.']); git(repository, ['commit', '-m', 'Preserve session cache across refresh']);
  git(repository, ['checkout', 'main']); writeFileSync(join(repository, 'config.ts'), 'export const retries = 3;\n'); git(repository, ['add', '.']); git(repository, ['commit', '-m', 'Configure retry policy']); git(repository, ['merge', '--no-ff', 'fix/session-cache', '-m', 'Merge session cache fix', '-m', 'Keep refresh behavior consistent across sessions.\n\n- Reject expired tokens with a clear error\n- Preserve the session cache\n- Add regression coverage']);
  writeFileSync(join(repository, 'pending.txt'), 'Uncommitted work\n');
  execFileSync('cargo', ['build', '-p', 'gitnova-core', '--example', 'ui_bridge'], { cwd: root, stdio: 'pipe' });
});
const pendingCore = new Set<Promise<unknown>>();
let coreQueue: Promise<void> = Promise.resolve();
function runCoreSerially<T>(task: () => Promise<T>): Promise<T> {
  const request = coreQueue.then(task);
  coreQueue = request.then(() => undefined, () => undefined);
  // Track queued requests too, so cleanup never races a later queued Git write.
  pendingCore.add(request);
  void request.then(() => pendingCore.delete(request), () => pendingCore.delete(request));
  return request;
}
test.afterAll(async () => {
  // A timed-out browser assertion must not delete a fixture underneath its Git operation.
  await Promise.allSettled([...pendingCore]);
  rmSync(folder, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
});
async function connectRealCore(page: Page, state: string, observe?: (command: string, args: unknown) => void, autoFetchSeconds:number|null=0) {
  await page.exposeBinding('gitnovaTestInvoke', async (_source, command: string, args: unknown) => runCoreSerially(async () => {
    observe?.(command, args);
    // Spawn with an argument array; the JSON request goes through stdin.
    const child = exec(bridge, [state], { maxBuffer: 16 * 1024 * 1024, env: { ...process.env, HOME:join(folder,'empty-home'), USERPROFILE:join(folder,'empty-home'), XDG_CONFIG_HOME:join(folder,'empty-home') } });
    const channel=(args as {onProgress?:string})?.onProgress;const id=channel?.startsWith('__CHANNEL__:') ? Number(channel.split(':')[1]) : null;
    let index=0,buffer='',delivery:Promise<unknown>=Promise.resolve(),lastPhase='',lastTime=0;
    child.child.stderr?.on('data',chunk=>{buffer+=String(chunk);const lines=buffer.split('\n');buffer=lines.pop()!;for(const line of lines){if(!line.startsWith('GITNOVA_PROGRESS:') || id===null)continue;const event=JSON.parse(line.slice('GITNOVA_PROGRESS:'.length));const now=Date.now();if(event.phase===lastPhase && event.completed!==0 && event.completed!==event.total && now-lastTime<100)continue;lastPhase=event.phase;lastTime=now;const message={index:index++,message:event};delivery=delivery.then(()=>page.evaluate(({id,message})=>(window as unknown as {__TAURI_INTERNALS__:{receive:(id:number,data:unknown)=>void}}).__TAURI_INTERNALS__.receive(id,message),{id,message})).catch(()=>{});}});
    child.child.stdin!.end(JSON.stringify({ command, args }));
    const { stdout } = await child;await delivery;
    if(id!==null)await page.evaluate(({id,index})=>(window as unknown as {__TAURI_INTERNALS__:{receive:(id:number,data:unknown)=>void}}).__TAURI_INTERNALS__.receive(id,{index,end:true}),{id,index}).catch(()=>{});
    const result = JSON.parse(stdout); if (result.error) throw new Error(result.error.message); return result.ok;
  }));
  await page.addInitScript((fetchSeconds:number|null) => {
    // Unrelated suites must never contact fixture hostnames on a timer.
    // Auto-fetch tests explicitly enable their disposable local remote.
    if(fetchSeconds===null)localStorage.removeItem('gitnova.auto-fetch-seconds');else localStorage.setItem('gitnova.auto-fetch-seconds',String(fetchSeconds));
    let sequence=0;const callbacks=new Map<number,(value:unknown)=>void>();
    Object.assign(window, { isTauri: true, __TAURI_INTERNALS__: {
      transformCallback:(callback:(value:unknown)=>void)=>{const id=++sequence;callbacks.set(id,callback);return id;},
      unregisterCallback:(id:number)=>callbacks.delete(id),
      receive:(id:number,value:unknown)=>{const event=value as {message?:unknown};if(event.message){const target=window as unknown as {gitnovaProgressEvents?:unknown[]};(target.gitnovaProgressEvents??=[]).push(event.message);}callbacks.get(id)?.(value);},
      invoke: (command: string, args: unknown) => (window as unknown as { gitnovaTestInvoke: (c: string, a: unknown) => Promise<unknown> }).gitnovaTestInvoke(command, JSON.parse(JSON.stringify(args??{})))
    } });
  },autoFetchSeconds);
}
async function changeLanguageThroughSettings(page:Page,language:'en'|'th') {
  const pickerWasOpen=await page.locator('.picker-menu').isVisible();
  await page.keyboard.press('Control+,');const dialog=page.locator('dialog.settings-dialog');await expect(dialog).toBeVisible();
  await dialog.locator('#settings-language').selectOption(language);await page.keyboard.press('Escape');await expect(dialog).not.toBeVisible();await expect(page.locator('.settings-trigger')).toBeFocused();
  if(pickerWasOpen){await page.keyboard.press('Control+o');await expect(page.locator('.picker-menu')).toBeVisible();}
}
test('real history, diff, workspace switching and keyboard context', async ({ page }, testInfo) => {
  const state = join(folder, 'state.json');
  await connectRealCore(page, state);
  await page.goto('/');
  await expect(page.getByRole('heading', { name: /Every change/ })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('welcome.png') });
  await page.getByLabel('Add an existing repository').fill(repository);
  await page.getByRole('button', { name: 'Open', exact: true }).click();
  await expect(page.getByRole('button', { name: /Merge session cache fix/ })).toBeVisible();
  await expect(page.getByText('1 working tree change')).toBeVisible();
  await page.getByRole('button', { name: /Merge session cache fix/ }).click();
  await expect(page.getByRole('heading', { name: 'Merge session cache fix' })).toBeVisible();
  await page.getByRole('button', { name: /src\/auth\/token.ts/ }).click();
  await expect(page.getByLabel('Commit diff')).toContainText('+export const tokenLifetime = 3600;');
  expect(await page.locator('body').evaluate(node => getComputedStyle(node).fontFamily)).toContain('sans-serif');
  await page.screenshot({ path: testInfo.outputPath('real-git-graph.png') });
  const historyBounds = await page.getByLabel('Commit history', { exact: true }).boundingBox();
  const diffBounds = await page.getByLabel('Commit diff', { exact: true }).boundingBox();
  const inspectorBounds = await page.getByLabel('Selected commit', { exact: true }).boundingBox();
  expect(diffBounds!.y).toBeGreaterThanOrEqual(historyBounds!.y + historyBounds!.height - 1);
  expect(inspectorBounds!.x).toBeGreaterThan(historyBounds!.x);
  await expect(page.getByRole('region', { name: 'Side-by-side patch' })).toContainText('export const tokenLifetime = 900;');
  await page.getByRole('button', { name: 'Unified', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Unified patch' })).toContainText('export const tokenLifetime = 3600;');
  await page.getByRole('button', { name: 'Side by side', exact: true }).click();
  await page.getByRole('button', { name: /src\/auth\/cache.ts/ }).click();
  await expect(page.getByLabel('Commit diff')).toContainText('new Map<string, string>()');
  await page.getByRole('button', { name: /src\/auth\/token.ts/ }).click();
  await page.getByRole('button', { name: 'Collapse diff', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Side-by-side patch' })).toBeHidden();
  await expect(page.locator('.commit-row.selected')).toContainText('Merge session cache fix');
  await page.getByRole('button', { name: 'Expand diff', exact: true }).click();
  await expect(page.locator('.dock-files button[aria-pressed="true"]')).toContainText('src/auth/token.ts');
  const divider = page.getByRole('separator', { name: 'Resize diff area' });
  const initialHeight = await page.locator('.commit-diff-dock').evaluate(node => node.clientHeight);
  await divider.focus(); await page.keyboard.press('ArrowUp');
  await expect.poll(() => page.locator('.commit-diff-dock').evaluate(node => node.clientHeight)).toBeGreaterThan(initialHeight);
  const resizedHeight = await page.locator('.commit-diff-dock').evaluate(node => node.clientHeight);
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.getByRole('button', { name: 'Copy SHA', exact: true }).click();
  await expect(page.getByRole('button', { name: 'SHA copied', exact: true })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(git(repository, ['rev-parse', 'HEAD']));
  await page.getByRole('button', { name: /Local workspace|My workspace/ }).click();
  await page.getByLabel('Add an existing repository').fill(other);
  await page.getByRole('button', { name: 'Open', exact: true }).click();
  await expect(page.locator('.repo-picker')).toContainText('project-web');
  await expect(page.getByRole('button', { name: /Create project/ })).toBeVisible();
  await page.keyboard.press(process.platform === 'darwin' ? 'Meta+o' : 'Control+o');
  await page.getByRole('button', { name: /project-api.*main/ }).click();
  await expect(page.getByRole('heading', { name: 'Merge session cache fix' })).toBeVisible();
  const merge = page.getByRole('button', { name: /Merge session cache fix/ });
  await merge.focus(); await page.keyboard.press('ArrowDown');
  await expect(page.locator('.commit-row:focus')).toHaveAttribute('aria-pressed', 'true');
  await expect(merge).toHaveAttribute('aria-pressed', 'false');
  await expect.poll(() => page.locator('.commit-diff-dock').evaluate(node => node.clientHeight)).toBe(resizedHeight);
  await expect(page.locator('.commit-row.selected')).toHaveCount(1);
  await expect(page.getByLabel('Selected commit')).toBeVisible();
  await page.setViewportSize({ width: 800, height: 600 });
  await expect(page.getByRole('button', { name: 'Close commit details' })).toBeVisible();
  await page.getByRole('button', { name: 'Close commit details' }).click();
  await expect(page.getByLabel('Selected commit')).toHaveCount(0);
  await page.getByRole('button', { name: 'Show commit details', exact: true }).click();
  await expect(page.getByLabel('Selected commit')).toBeVisible();
  await page.getByRole('button', { name: 'Close commit details' }).click();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await expect(page.getByRole('region', { name: 'Side-by-side patch' })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('compact-layout.png') });
  await page.keyboard.press(process.platform === 'darwin' ? 'Meta+o' : 'Control+o');
  await page.getByRole('textbox', { name: 'New workspace name' }).fill('Release tools');
  await page.getByRole('button', { name: 'Create', exact: true }).click();
  await expect(page.getByRole('heading', { name: /Every change/ })).toBeVisible();
  await page.getByLabel('Workspace', { exact: true }).selectOption({ label: 'My workspace' });
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: /Merge session cache fix/ })).toBeVisible();
  expect(git(repository, ['status', '--porcelain'])).toBe('?? pending.txt');
});
test('browser preview clearly requires the desktop host', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('alert')).toContainText('Open GitNova with npm run tauri dev');
  await expect(page.getByRole('button', { name: 'Open', exact: true })).toBeDisabled();
});

test('stage, unstage, review, commit and external cache invalidation use real Git', async ({ page }, testInfo) => {
  test.setTimeout(90000);
  const repo = join(folder, 'write-fixture');
  git(folder, ['clone', repository, repo]);
  git(repo, ['config', 'user.name', 'GitNova Test']); git(repo, ['config', 'user.email', 'test@example.invalid']); git(repo, ['config', 'commit.gpgsign', 'false']);
  git(repo, ['config', 'core.hooksPath', '.git/gitnova-empty-hooks']); git(repo, ['config', 'core.attributesFile', '.git/gitnova-empty-attributes']);
  writeFileSync(join(repo, 'pending.ts'), 'export const approved = true;\n');
  writeFileSync(join(repo, 'keep.txt'), 'This stays uncommitted.\n');
  await connectRealCore(page, join(folder, 'write-state.json'));
  await page.goto('/');
  await page.getByLabel('Add an existing repository').fill(repo);
  await page.getByRole('button', { name: 'Open', exact: true }).click();
  await expect(page.locator('.repo-picker')).toContainText('write-fixture');
  await page.locator('.working-row').click();
  await page.locator('.changed-file').filter({ hasText: 'pending.ts' }).click();
  await expect(page.getByLabel('Working file diff')).toContainText('export const approved = true;');
  await page.getByRole('button', { name: 'Stage file', exact: true }).click();
  await expect(page.getByText('File staged', { exact: true })).toBeVisible();
  await page.locator('.changed-file').filter({ hasText: 'pending.ts' }).click();
  await page.getByRole('button', { name: 'Unstage file', exact: true }).click();
  await expect(page.getByText('File unstaged; working file preserved', { exact: true })).toBeVisible();
  await page.locator('.changed-file').filter({ hasText: 'pending.ts' }).click();
  await page.getByRole('button', { name: 'Stage file', exact: true }).click();
  await expect(page.getByText('File staged', { exact: true })).toBeVisible();
  await page.locator('.changed-file').filter({ hasText: 'pending.ts' }).click();
  await expect(page.getByLabel('Working file diff')).toContainText('export const approved = true;');
  await page.screenshot({ path: testInfo.outputPath('working-changes.png') });
  await page.getByLabel('Summary', { exact: true }).fill('Add approved example');
  await page.getByLabel('Description optional').fill('Keep unrelated work outside this commit.');
  await page.getByRole('button', { name: 'Review commit', exact: true }).click();
  await expect(page.getByLabel('Staged commit preview')).toContainText('export const approved = true;');
  await expect(page.getByLabel('Staged commit preview')).not.toContainText('This stays uncommitted.');
  await page.getByRole('button', { name: 'Commit staged changes', exact: true }).click();
  await expect(page.getByText(/Commit created:/)).toBeVisible();
  expect(git(repo, ['show', 'HEAD:pending.ts'])).toBe('export const approved = true;');
  expect(git(repo, ['log', '-1', '--format=%B'])).toBe('Add approved example\n\nKeep unrelated work outside this commit.');
  expect(git(repo, ['status', '--porcelain'])).toBe('?? keep.txt');
  await expect(page.getByRole('button', { name: /Add approved example/ })).toBeVisible();
  await expect(page.getByLabel('Summary', { exact: true })).toBeEnabled();
  git(repo, ['commit', '--allow-empty', '-m', 'External history refresh']);
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await expect(page.getByRole('button', { name: /External history refresh/ })).toBeVisible({ timeout: 15000 });
  await page.getByRole('button', { name: /Add approved example/ }).click();
  await expect(page.getByRole('heading', { name: 'Add approved example' })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('refined-git-graph.png') });
});

test('local branch selector previews and switches Git without discarding dirty work', async ({ page }) => {
  test.setTimeout(90000);
  const repo = join(folder, 'branch-fixture'); git(folder, ['clone', repository, repo]);
  git(repo, ['config', 'core.hooksPath', '.git/gitnova-empty-hooks']); git(repo, ['config', 'core.attributesFile', '.git/gitnova-empty-attributes']);
  git(repo, ['branch', 'feature', 'origin/fix/session-cache']);
  await connectRealCore(page, join(folder, 'branch-state.json')); await page.goto('/');
  await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  const switcher = page.getByRole('button', { name: 'Switch branch', exact: true });
  await expect(switcher).toContainText('main'); await switcher.click();
  await page.getByRole('button', { name: 'Select branch feature', exact: true }).click();
  await expect(page.locator('.branch-preview-files')).toContainText('config.ts');
  await page.getByRole('button', { name: 'Switch to feature', exact: true }).click();
  await expect(switcher).toContainText('feature');
  expect(git(repo, ['branch', '--show-current'])).toBe('feature');
  writeFileSync(join(repo, 'do-not-lose.txt'), 'Keep my uncommitted work.\n');
  await switcher.click(); await page.getByRole('button', { name: 'Select branch main', exact: true }).click();
  await expect(page.locator('.branch-preview .notice')).toContainText('uncommitted changes');
  await expect(page.getByRole('button', { name: 'Switch to main', exact: true })).toBeDisabled();
  expect(git(repo, ['branch', '--show-current'])).toBe('feature');
  expect(git(repo, ['status', '--porcelain'])).toBe('?? do-not-lose.txt');
});

test('before and after stay equal-width with independent horizontal and synchronized vertical scrolling', async ({ page }, testInfo) => {
  await connectRealCore(page, join(folder, 'fixed-diff-state.json')); await page.goto('/');
  await page.getByLabel('Add an existing repository').fill(repository); await page.getByRole('button', { name: 'Open', exact: true }).click();
  await page.getByRole('button', { name: /src\/auth\/long-lines.ts/ }).click();
  const before = page.getByRole('region', { name: 'Before code', exact: true }); const after = page.getByRole('region', { name: 'After code', exact: true });
  await expect(before).toBeVisible(); await expect(after).toBeVisible();
  const leftBox = await before.boundingBox(); const rightBox = await after.boundingBox();
  expect(Math.abs(leftBox!.width - rightBox!.width)).toBeLessThanOrEqual(1);
  expect(Math.abs(leftBox!.x + leftBox!.width - rightBox!.x)).toBeLessThanOrEqual(1);
  await before.evaluate(node => { node.scrollLeft = 500; node.scrollTop = 700; });
  await expect.poll(() => after.evaluate(node => node.scrollTop)).toBe(700);
  expect(await before.evaluate(node => node.scrollLeft)).toBeGreaterThan(0);
  expect(await after.evaluate(node => node.scrollLeft)).toBe(0);
  const rightAfter = await after.boundingBox(); expect(rightAfter!.x).toBe(rightBox!.x);
  const headers = page.locator('.split-labels'); const headerBox = await headers.boundingBox();
  expect(headerBox!.y + headerBox!.height).toBeLessThanOrEqual(rightAfter!.y + 1);
  await after.evaluate(node => { node.scrollLeft = 320; node.scrollTop = 1100; });
  await expect.poll(() => before.evaluate(node => node.scrollTop)).toBe(1100);
  expect(await before.evaluate(node => node.scrollLeft)).toBeGreaterThan(0);
  await page.screenshot({ path: testInfo.outputPath('fixed-diff-panes.png') });
});

test('parallel branch graph keeps reference labels, compact rows and merge topology readable', async ({ page }, testInfo) => {
  const repo = join(folder, 'parallel-graph'); mkdirSync(repo); git(repo, ['init', '-b', 'main']);
  writeFileSync(join(repo, 'README.md'), '# Parallel history\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Start workspace']);
  const rootOid = git(repo, ['rev-parse', 'HEAD']); const tree = git(repo, ['rev-parse', 'HEAD^{tree}']);
  const tips = Array<string>(4).fill(rootOid);
  const create = (message: string, parents: string[], seconds: number) => execFileSync('git', ['-C', repo, '-c', 'user.name=GitNova Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', 'commit-tree', tree, ...parents.flatMap(parent => ['-p', parent]), '-m', message], { encoding: 'utf8', env: { ...process.env, GIT_AUTHOR_DATE: `${seconds} +0000`, GIT_COMMITTER_DATE: `${seconds} +0000`, GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' } }).trim();
  const subjects = ['Refine workspace context', 'Preserve session cache', 'Improve release checks', 'Index historical changes'];
  for (let generation = 0; generation < 8; generation++) for (let branch = 0; branch < tips.length; branch++) tips[branch] = create(`${subjects[branch]} · ${generation + 1}`, [tips[branch]], 1_790_000_000 + generation * 60 + branch);
  const merge = create('Merge workspace navigation', [tips[0], tips[1]], 1_790_001_000);
  git(repo, ['update-ref', 'refs/heads/main', merge]);
  git(repo, ['update-ref', 'refs/remotes/origin/main', merge]);
  git(repo, ['update-ref', 'refs/tags/v0.2.0', merge]);
  for (let branch = 1; branch < tips.length; branch++) git(repo, ['update-ref', `refs/heads/${['', 'fix/session-cache', 'release/next', 'feature/history'][branch]}`, tips[branch]]);
  await connectRealCore(page, join(folder, 'parallel-state.json')); await page.goto('/');
  await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  const row = page.locator(`#commit-${merge}`);
  await expect(row).toBeVisible(); await expect(page.locator('.history-heading')).toContainText('Branch / Tag');
  await expect(row.locator('.graph-ref.local')).toHaveCount(1); await expect(row.locator('.graph-ref.tag')).toHaveText('v0.2.0');
  await expect(row.locator('.merge-node')).toHaveCount(1); await expect(row.locator('.head-ring')).toHaveCount(1);
  expect(await row.evaluate(node => node.getBoundingClientRect().height)).toBe(36);
  const lanes = await page.locator('.graph-mark').evaluateAll(nodes => nodes.map(node => Number(node.getAttribute('data-lane'))));
  expect(Math.max(...lanes)).toBeGreaterThanOrEqual(3);
  const branchRow = page.locator(`#commit-${tips[3]}`); await branchRow.click();
  await expect(page.getByRole('heading', { name: `${subjects[3]} · 8` })).toBeVisible();
  const lane = await branchRow.locator('.graph-mark').getAttribute('data-lane'); const color = await branchRow.locator('.graph-mark').getAttribute('data-color');
  await page.locator('.history-scroll').evaluate(node => { node.scrollTop = node.scrollHeight; });
  await expect.poll(() => page.locator('.commit-row').filter({ hasText: `${subjects[3]} · 1` }).count()).toBe(1);
  await expect(page.locator('.commit-row').filter({ hasText: `${subjects[3]} · 1` }).locator('.graph-mark')).toHaveAttribute('data-lane', lane!);
  await expect(page.locator('.commit-row').filter({ hasText: `${subjects[3]} · 1` }).locator('.graph-mark')).toHaveAttribute('data-color', color!);
  await page.locator('.history-scroll').evaluate(node => { node.scrollTop = 0; });
  await expect(branchRow).toBeVisible();
  await page.getByRole('button', { name: 'Collapse diff', exact: true }).click();
  await page.screenshot({ path: testInfo.outputPath('parallel-graph.png') });
  await page.setViewportSize({ width: 820, height: 900 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(820);
  await page.locator('.history-scroll').evaluate(node => { node.scrollLeft = 100; });
  await expect.poll(async () => Math.round(await page.locator('.history-heading-viewport').evaluate(node => node.scrollLeft))).toBe(await page.locator('.history-scroll').evaluate(node => Math.round(node.scrollLeft)));
  const headerLeft = await page.locator('.history-heading > span:nth-child(3)').evaluate(node => node.getBoundingClientRect().left);
  const subjectLeft = await branchRow.locator('.commit-description').evaluate(node => node.getBoundingClientRect().left);
  expect(Math.abs(headerLeft - subjectLeft)).toBeLessThan(2);
});

test('remote selection fetches latest, creates tracking local and pulls an existing branch', async ({ page }, testInfo) => {
  test.setTimeout(120000);
  const writer = join(folder, 'remote-writer'); const repo = join(folder, 'remote-local'); mkdirSync(writer); git(writer, ['init', '-b', 'main']);
  writeFileSync(join(writer, 'README.md'), '# Remote workspace\n'); git(writer, ['add', '.']); git(writer, ['commit', '-m', 'Start remote project']); git(folder, ['clone', writer, repo]);
  git(repo, ['config', 'core.hooksPath', '.git/empty-hooks']); git(repo, ['config', 'core.attributesFile', '.git/empty-attributes']);
  // Created after cloning: no cached ref can expose it until the explicit Fetch action.
  git(writer, ['switch', '-c', 'feature/remote-api']); writeFileSync(join(writer, 'api.ts'), 'export const revision = 1;\n'); git(writer, ['add', '.']); git(writer, ['commit', '-m', 'Add remote API']);
  await connectRealCore(page, join(folder, 'remote-ui-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  const switcher = page.getByRole('button', { name: 'Switch branch', exact: true }); await switcher.click(); await page.getByRole('button', { name: /^Remotes / }).click();
  await expect(page.getByRole('button', { name: 'Open remote origin/feature/remote-api', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Fetch branches from origin', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Open remote origin/feature/remote-api', exact: true })).toBeVisible();
  expect(git(repo, ['branch', '--show-current'])).toBe('main');
  // Selecting the discovered remote fetches again, so the review sees this newer revision.
  writeFileSync(join(writer, 'api.ts'), 'export const revision = 2;\n'); git(writer, ['commit', '-am', 'Refresh remote API']); const latest = git(writer, ['rev-parse', 'HEAD']);
  await page.getByRole('button', { name: 'Open remote origin/feature/remote-api', exact: true }).click();
  await expect(page.locator('.remote-integration-summary')).toContainText('Create a local tracking branch'); await expect(page.locator('.branch-preview')).toContainText(latest.slice(0, 8));
  expect(git(repo, ['branch', '--show-current'])).toBe('main');
  await page.screenshot({ path: testInfo.outputPath('remote-branch-review.png') });
  await page.getByRole('button', { name: 'Create & open feature/remote-api', exact: true }).click(); await expect(switcher).toBeEnabled({ timeout: 45000 }); await expect(switcher).toContainText('feature/remote-api');
  expect(git(repo, ['rev-parse', 'HEAD'])).toBe(latest); expect(git(repo, ['rev-parse', '--symbolic-full-name', '@{upstream}'])).toBe('refs/remotes/origin/feature/remote-api');
  await switcher.click(); await page.getByRole('button', { name: /^Local / }).click(); await page.getByRole('button', { name: 'Select branch main', exact: true }).click(); await page.getByRole('button', { name: 'Switch to main', exact: true }).click(); await expect(switcher).toBeEnabled({ timeout: 45000 }); await expect(switcher).toContainText('main');
  writeFileSync(join(writer, 'api.ts'), 'export const revision = 3;\n'); git(writer, ['commit', '-am', 'Ship remote API']); const pulled = git(writer, ['rev-parse', 'HEAD']);
  await switcher.click(); await page.getByRole('button', { name: /^Remotes / }).click(); await page.getByRole('button', { name: 'Open remote origin/feature/remote-api', exact: true }).click(); await expect(page.locator('.remote-integration-summary')).toContainText('1 commit to pull');
  await page.getByRole('button', { name: 'Open & pull feature/remote-api', exact: true }).click(); await expect(switcher).toBeEnabled({ timeout: 45000 }); await expect(switcher).toContainText('feature/remote-api');
  expect(git(repo, ['rev-parse', 'HEAD'])).toBe(pulled); expect(git(repo, ['show', 'HEAD:api.ts'])).toBe('export const revision = 3;');
});

test('switching parent branch with a submodule pointer change preserves the child checkout', async ({ page }, testInfo) => {
  test.setTimeout(90000);
  const childSource = join(folder, 'child-source'); const repo = join(folder, 'submodule-parent'); mkdirSync(childSource); mkdirSync(repo);
  git(childSource, ['init', '-b', 'main']); writeFileSync(join(childSource, 'child.ts'), 'export const version = 1;\n'); git(childSource, ['add', '.']); git(childSource, ['commit', '-m', 'Module version one']); const first = git(childSource, ['rev-parse', 'HEAD']);
  writeFileSync(join(childSource, 'child.ts'), 'export const version = 2;\n'); git(childSource, ['commit', '-am', 'Module version two']); const second = git(childSource, ['rev-parse', 'HEAD']);
  git(repo, ['init', '-b', 'main']); git(repo, ['-c', 'protocol.file.allow=always', 'submodule', 'add', childSource, 'packages/module']); const child = join(repo, 'packages/module'); git(child, ['switch', '--detach', first]); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Record module version one']);
  git(repo, ['switch', '-c', 'develop']); git(child, ['switch', '--detach', second]); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Record module version two']); git(repo, ['switch', '--no-recurse-submodules', 'main']); git(child, ['switch', '--detach', first]);
  await connectRealCore(page, join(folder, 'submodule-ui-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  const switcher = page.getByRole('button', { name: 'Switch branch', exact: true }); await switcher.click(); await page.getByRole('button', { name: 'Select branch develop', exact: true }).click();
  await expect(page.locator('.branch-submodules')).toContainText('Submodule checkouts stay in place'); await expect(page.locator('.branch-submodules')).toContainText(first.slice(0, 8)); await expect(page.locator('.branch-submodules')).toContainText(second.slice(0, 8));
  await expect(page.getByRole('button', { name: 'Switch to develop', exact: true })).toBeEnabled(); await page.screenshot({ path: testInfo.outputPath('submodule-branch-review.png') });
  await page.getByRole('button', { name: 'Switch to develop', exact: true }).click(); await expect(switcher).toContainText('develop'); await expect(switcher).toBeEnabled();
  expect(git(child, ['rev-parse', 'HEAD'])).toBe(first); expect(git(repo, ['rev-parse', 'HEAD:packages/module'])).toBe(second); await expect(page.locator('.status-line')).toContainText('1 working tree change');
  await switcher.click(); await page.getByRole('button', { name: 'Select branch main', exact: true }).click(); await page.getByRole('button', { name: 'Switch to main', exact: true }).click(); await expect(switcher).toContainText('main'); await expect(switcher).toBeEnabled();
  expect(git(child, ['rev-parse', 'HEAD'])).toBe(first); expect(git(repo, ['status', '--porcelain'])).toBe('');
});

test('working changes keep both stages, composer and selected diff coherent through review', async ({ page }, testInfo) => {
  test.setTimeout(90000);
  const repo = join(folder, 'workflow-layout'); mkdirSync(repo); git(repo, ['init', '-b', 'main']);
  writeFileSync(join(repo, 'partial.txt'), 'base-version\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Start workflow']);
  writeFileSync(join(repo, 'partial.txt'), 'staged-version\n'); writeFileSync(join(repo, 'ready.txt'), 'SECOND-STAGED-FILE\n'); git(repo, ['add', '.']);
  writeFileSync(join(repo, 'partial.txt'), 'working-version\n');
  for (let i = 0; i < 45; i++) writeFileSync(join(repo, `pending-${i.toString().padStart(2, '0')}.txt`), 'UNSTAGED-ONLY\n');
  await connectRealCore(page, join(folder, 'workflow-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  await page.locator('.working-row').click();
  await expect(page.getByRole('region', { name: 'Unstaged files', exact: true })).toBeVisible(); await expect(page.getByRole('region', { name: 'Staged files', exact: true })).toBeVisible();
  await expect(page.locator('.unstaged-group').getByRole('button',{name:'Stage all changes…',exact:true})).toBeEnabled(); await expect(page.locator('.staged-group')).toContainText('In next commit');
  const composer = page.locator('.commit-compose'); const initialButtonBottom = await page.getByRole('button', { name: 'Review commit', exact: true }).evaluate(node => node.getBoundingClientRect().bottom);
  await page.locator('.unstaged-group .changed-file').filter({ hasText: 'partial.txt' }).click(); await expect(page.getByLabel('Working file diff')).toContainText('+working-version'); await expect(page.getByLabel('Working file diff')).toContainText('Index → Working tree');
  await page.locator('.staged-group .changed-file').filter({ hasText: 'partial.txt' }).click(); await expect(page.getByLabel('Working file diff')).toContainText('+staged-version'); await expect(page.getByLabel('Working file diff')).not.toContainText('+working-version');
  await page.getByLabel('Summary', { exact: true }).fill('Keep the prepared changes together'); await page.getByLabel('Description optional').fill('The working copy still has another revision.');
  await page.screenshot({ path: testInfo.outputPath('working-sequence.png') });
  await page.getByRole('button', { name: 'Review commit', exact: true }).click(); await expect(page.getByLabel('Staged commit preview')).toContainText('+staged-version');
  await page.locator('.staged-group .changed-file').filter({ hasText: 'ready.txt' }).click(); await expect(page.getByLabel('Staged commit preview')).toContainText('SECOND-STAGED-FILE');
  await expect(page.getByLabel('Staged commit preview')).not.toContainText('UNSTAGED-ONLY'); await expect(page.getByRole('button', { name: 'Commit staged changes', exact: true })).toBeEnabled();
  expect(Math.abs(await page.getByRole('button', { name: 'Commit staged changes', exact: true }).evaluate(node => node.getBoundingClientRect().bottom) - initialButtonBottom)).toBeLessThan(2);
  await page.getByRole('button', { name: 'Back to changes', exact: true }).click(); await expect(page.getByLabel('Summary', { exact: true })).toHaveValue('Keep the prepared changes together'); await expect(page.getByLabel('Description optional')).toHaveValue('The working copy still has another revision.');
  await page.getByRole('button', { name: 'Review commit', exact: true }).click(); await expect(page.getByRole('button', { name: 'Commit staged changes', exact: true })).toBeEnabled();
  writeFileSync(join(repo, 'ready.txt'), 'EXTERNALLY-CHANGED-STAGED-CONTENT\n'); git(repo, ['add', 'ready.txt']);
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await expect(page.getByRole('alert')).toContainText('Working changes changed'); await expect(page.getByRole('button', { name: 'Commit staged changes', exact: true })).toHaveCount(0);
  await expect(page.getByLabel('Summary', { exact: true })).toHaveValue('Keep the prepared changes together');
  await page.locator('.unstaged-group .change-group-files').evaluate(node => { node.scrollTop = node.scrollHeight; }); await expect(page.locator('.staged-group .change-group-heading')).toBeVisible(); await expect(composer).toBeVisible();
  await page.setViewportSize({ width: 800, height: 600 }); await expect(page.getByRole('button', { name: 'Review commit', exact: true })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(800);
  await page.screenshot({ path: testInfo.outputPath('working-sequence-compact.png') });
  // A clean repository keeps both empty states and explains why commit is unavailable.
  await page.setViewportSize({ width: 1360, height: 900 }); await page.getByRole('button', { name: 'Close working changes', exact: true }).click();
  await page.getByRole('button', { name: /My workspace/ }).click(); await page.getByLabel('Add an existing repository').fill(other); await page.getByRole('button', { name: 'Open', exact: true }).click(); await expect(page.locator('.repo-picker')).toContainText('project-web'); await page.locator('.working-row').click();
  await expect(page.locator('.unstaged-group')).toContainText('No unstaged files.'); await expect(page.locator('.staged-group')).toContainText('Stage a file'); await expect(page.getByRole('button', { name: 'Review commit', exact: true })).toBeDisabled(); await expect(page.locator('.commit-next')).toContainText('Stage files');
  await page.getByRole('button', { name: /My workspace/ }).click(); await page.getByRole('button', { name: /workflow-layout.*main/ }).click();
  await expect(page.locator('.repo-picker')).toContainText('workflow-layout'); await page.locator('.working-row').click();
  await expect(page.getByLabel('Summary', { exact: true })).toHaveValue('Keep the prepared changes together');
  await expect(page.getByLabel('Working file diff')).not.toContainText('# Project');
});

test('staged review selects exact raw paths with spaces, controls, quotes and non-UTF-8 bytes', async ({ page }) => {
  test.setTimeout(90000);
  const repo = join(folder, 'path-regression'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); git(repo, ['commit', '--allow-empty', '-m', 'Start path checks']);
  const names = process.platform === 'win32' ? ['a.txt', 'space name.txt', 'ภาษาไทย.txt'] : ['a.txt', 'space name.txt', 'trailing ', 'quote".txt', 'tab\tname.txt', 'line\nname.txt', 'ภาษาไทย.txt'];
  const paths = names.map(name => Buffer.from(name));
  if (process.platform === 'linux') paths.push(Buffer.from([0xff, 46, 116, 120, 116]), Buffer.from([0xfe, 46, 116, 120, 116]));
  for (const [index, bytes] of paths.entries()) writeFileSync(Buffer.concat([Buffer.from(repo + '/'), bytes]), `PATH_MARKER_${index}\n++ b/not-a-header\n`);
  git(repo, ['add', '.']); git(repo, ['config', 'diff.noprefix', 'true']); git(repo, ['config', 'diff.mnemonicPrefix', 'true']);
  await connectRealCore(page, join(folder, 'path-regression.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click(); await expect(page.locator('.repo-picker')).toContainText('path-regression');
  await page.locator('.working-row').click(); await page.getByLabel('Summary', { exact: true }).fill('Review actual path identities'); await page.getByRole('button', { name: 'Review commit', exact: true }).click(); await expect(page.getByRole('button', { name: 'Commit staged changes', exact: true })).toBeEnabled();
  for (const [index, bytes] of paths.entries()) {
    // Native Unix paths are byte identities, even when two labels render identically.
    const row = process.platform === 'win32' ? page.locator('.staged-group .changed-file').filter({ hasText: bytes.toString() }) : page.locator(`.staged-group .changed-file[data-path-hex="${bytes.toString('hex')}"]`);
    await row.click(); const diff = page.getByLabel('Staged commit preview'); await expect(diff).toContainText(`PATH_MARKER_${index}`); await expect(diff).toContainText('++ b/not-a-header');
    for (const otherIndex of paths.keys()) if (otherIndex !== index) await expect(diff).not.toContainText(`PATH_MARKER_${otherIndex}`);
  }
});

test('refresh and external refs preserve graph position across repository switches and older pages', async ({ page }) => {
  test.setTimeout(90000);
  const repo = join(folder, 'position-regression'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); git(repo, ['commit', '--allow-empty', '-m', 'History root']);
  const tree = git(repo, ['rev-parse', 'HEAD^{tree}']); let tip = git(repo, ['rev-parse', 'HEAD']);
  for (let i = 1; i <= 120; i++) tip = git(repo, ['commit-tree', tree, '-p', tip, '-m', `Position entry ${i}`]); git(repo, ['update-ref', 'refs/heads/main', tip]);
  await connectRealCore(page, join(folder, 'position-regression.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click(); await expect(page.locator('.repo-picker')).toContainText('position-regression');
  await page.getByRole('button', { name: 'Load older commits', exact: true }).click(); await expect(page.locator('.app-footer')).toContainText('1–121 commits');
  const history = page.locator('.history-scroll'); await history.evaluate(node => { node.scrollTop = 105 * 36; }); await expect.poll(() => history.evaluate(node => node.scrollTop)).toBe(105 * 36); const before = await history.evaluate(node => node.scrollTop);
  await page.getByRole('button', { name: 'Refresh', exact: true }).click(); await expect(page.getByRole('button', { name: 'Refresh', exact: true })).toBeEnabled({ timeout: 30000 });
  await page.getByRole('button', { name: /My workspace/ }).click(); await page.getByLabel('Add an existing repository').fill(other); await page.getByRole('button', { name: 'Open', exact: true }).click(); await expect(page.locator('.repo-picker')).toContainText('project-web');
  await page.getByRole('button', { name: /My workspace/ }).click(); await page.getByRole('button', { name: /position-regression.*main/ }).click(); await expect(page.locator('.repo-picker')).toContainText('position-regression'); await expect.poll(() => history.evaluate(node => node.scrollTop)).toBe(before);
  const next = git(repo, ['commit-tree', tree, '-p', tip, '-m', 'New external head']); git(repo, ['update-ref', 'refs/heads/main', next]); await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await expect(page.locator('.app-footer')).toContainText('1–122 commits', { timeout: 30000 }); await expect.poll(() => history.evaluate(node => node.scrollTop)).toBe(before + 36);
});

test('failed commit result survives leaving working changes while a hook is pending', async ({ page }) => {
  test.setTimeout(90000);
  const repo = join(folder, 'receipt-regression'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); writeFileSync(join(repo, 'file.txt'), 'baseline\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Receipt history root']);
  git(repo, ['config', 'user.name', 'GitNova Test']); git(repo, ['config', 'user.email', 'test@example.invalid']); git(repo, ['config', 'commit.gpgsign', 'false']); git(repo, ['config', 'core.hooksPath', '.git/hooks']);
  writeFileSync(join(repo, 'file.txt'), 'prepared\n'); git(repo, ['add', '.']); const before = git(repo, ['rev-parse', 'HEAD']);
  const hook = join(repo, '.git/hooks/pre-commit'); writeFileSync(hook, '#!/bin/sh\nsleep 3\necho EXPECTED_HOOK_FAILURE >&2\nexit 1\n'); chmodSync(hook, 0o755);
  await connectRealCore(page, join(folder, 'receipt-regression.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click(); await expect(page.locator('.repo-picker')).toContainText('receipt-regression');
  await page.locator('.working-row').click(); await page.getByLabel('Summary', { exact: true }).fill('Review a pending operation'); await page.getByRole('button', { name: 'Review commit', exact: true }).click(); await page.getByRole('button', { name: 'Commit staged changes', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Committing…', exact: true })).toBeDisabled(); await expect(page.getByRole('button', { name: 'Switch branch', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: /Receipt history root/ }).click(); await expect(page.getByRole('heading', { name: 'Receipt history root' })).toBeVisible();
  await expect(page.getByRole('alert')).toContainText('EXPECTED_HOOK_FAILURE', { timeout: 45000 }); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(before); expect(git(repo, ['show', ':file.txt'])).toBe('prepared');
  await page.locator('.working-row').click(); await expect(page.getByRole('alert')).toContainText('EXPECTED_HOOK_FAILURE'); await expect(page.getByLabel('Summary', { exact: true })).toHaveValue('Review a pending operation');
  await page.getByRole('button', { name: 'Dismiss error', exact: true }).click(); await expect(page.getByRole('alert')).toHaveCount(0);
});

test('related submodule initialize and recorded restore are previewed without changing parent staging', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  const source = join(folder, 'initialize-source'); const parent = join(folder, 'initialize-parent'); const repo = join(folder, 'initialize-local'); mkdirSync(source); mkdirSync(parent);
  git(source, ['init', '-b', 'main']); writeFileSync(join(source, 'file.txt'), 'first\n'); git(source, ['add', '.']); git(source, ['commit', '-m', 'Module first']); const first = git(source, ['rev-parse', 'HEAD']);
  writeFileSync(join(source, 'file.txt'), 'second\n'); git(source, ['commit', '-am', 'Module second']); const second = git(source, ['rev-parse', 'HEAD']);
  git(parent, ['init', '-b', 'main']); git(parent, ['-c', 'protocol.file.allow=always', 'submodule', 'add', source, 'packages/module']); git(join(parent, 'packages/module'), ['switch', '--detach', first]); git(parent, ['add', '.']); git(parent, ['commit', '-m', 'Recorded first module']); git(folder, ['clone', parent, repo]);
  await connectRealCore(page, join(folder, 'initialize-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click(); await expect(page.locator('.repo-picker')).toContainText('initialize-local');
  await page.getByRole('button', { name: /My workspace/ }).click(); await page.getByRole('button', { name: 'Initialize…', exact: true }).click();
  const review = page.getByRole('region', { name: 'Review submodule packages/module', exact: true }); await expect(review).toContainText(first.slice(0, 12), { timeout: 30000 }); await expect(review).toContainText('Not initialized');
  // Inspection between preview and apply must not refresh the index and stale its token.
  const reviewedIndex = readFileSync(join(repo, '.git/index'));
  expect(git(repo, ['--no-optional-locks', 'status', '--porcelain'])).toBe('');
  expect(readFileSync(join(repo, '.git/index'))).toEqual(reviewedIndex);
  await page.screenshot({ path: testInfo.outputPath('initialize-recorded.png') });
  await page.getByRole('button', { name: 'Initialize recorded version', exact: true }).click(); await expect(page.locator('.submodule-open')).toContainText(`checkout ${first.slice(0, 8)}`, { timeout: 45000 }); await expect(page.getByRole('button', { name: 'Switch branch', exact: true })).toBeEnabled({ timeout: 30000 });
  const child = join(repo, 'packages/module'); expect(git(child, ['rev-parse', 'HEAD'])).toBe(first); expect(git(repo, ['status', '--porcelain'])).toBe('');
  // Stage a different recorded pointer without moving the child checkout: Restore must use INDEX, not HEAD.
  git(repo, ['update-index', '--cacheinfo', `160000,${second},packages/module`]);
  await expect(page.getByRole('region', { name: 'Workspaces and repositories' })).toBeVisible();
  await page.getByRole('button', { name: /My workspace/ }).click(); await expect(page.getByRole('region', { name: 'Workspaces and repositories' })).toBeHidden(); // close picker
  await page.evaluate(() => window.dispatchEvent(new Event('focus'))); await page.getByRole('button', { name: /My workspace/ }).click();
  // Reopening the picker refreshes discovery; the staged pointer is shown independently.
  await expect(page.getByRole('region', { name: 'Workspaces and repositories' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Restore recorded…', exact: true })).toBeVisible({ timeout: 30000 });
  await page.getByRole('button', { name: 'Restore recorded…', exact: true }).click(); await expect(review).toContainText(second.slice(0, 12), { timeout: 30000 }); await expect(review).toContainText('staged pointer');
  await page.screenshot({ path: testInfo.outputPath('restore-recorded.png') }); await page.getByRole('button', { name: 'Restore recorded version', exact: true }).click(); await expect(page.locator('.submodule-open')).toContainText(`checkout ${second.slice(0, 8)}`, { timeout: 45000 });
  expect(git(child, ['rev-parse', 'HEAD'])).toBe(second); expect(git(repo, ['rev-parse', 'HEAD:packages/module'])).toBe(first); expect(git(repo, ['status', '--porcelain'])).toBe('M  packages/module');
  await expect(page.getByRole('button', { name: 'Restore recorded…', exact: true })).toHaveCount(0);
  await page.locator('.submodule-open').click(); await expect(page.locator('.repo-picker')).toContainText('module'); await expect(page.locator('.branch-current')).toContainText(second.slice(0, 8));
});

test('working count expands 127 untracked files consistently across graph, status and staging', async ({ page }, testInfo) => {
  test.setTimeout(90000);
  const repo = join(folder, 'count-127'); mkdirSync(repo); git(repo, ['init', '-b', 'main']);
  for (let i = 0; i < 127; i++) { const bucket = join(repo, `bucket-${i % 12}`); mkdirSync(bucket, { recursive: true }); writeFileSync(join(bucket, `file${i}.txt`), 'new file\n'); }
  await connectRealCore(page, join(folder, 'count-127-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  await expect(page.locator('.working-row .change-count')).toHaveText('127'); await expect(page.locator('.status-line')).toContainText('127 working tree changes'); await expect(page.locator('.app-footer')).toContainText('0 commits loaded');
  await page.locator('.working-row').click(); await expect(page.locator('.working-context')).toContainText('127 changed files'); await expect(page.locator('.unstaged-group .changed-file')).toHaveCount(127);
  await page.locator('.unstaged-group .changed-file').filter({ hasText: 'bucket-0/file0.txt' }).click(); await page.getByRole('button', { name: 'Stage file', exact: true }).click();
  await expect(page.locator('.staged-group .changed-file')).toHaveCount(1); await expect(page.locator('.unstaged-group .changed-file')).toHaveCount(126); await expect(page.locator('.working-row .change-count')).toHaveText('127'); await expect(page.locator('.working-context')).toContainText('127 changed files');
  await page.screenshot({ path: testInfo.outputPath('consistent-127-files.png') });
});

test('graph context actions and double click checkout preserve scoped branches, tags and detached commits', async ({ page }, testInfo) => {
  test.setTimeout(180000); page.setDefaultTimeout(15000); const browserErrors: string[] = []; page.on('console', message => { if (message.type() === 'error') browserErrors.push(message.text()); }); page.on('pageerror', error => browserErrors.push(error.message));
  const repo = join(folder, 'graph-actions'); mkdirSync(repo); git(repo, ['init', '-b', 'main']);
  writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Graph action base']); const base = git(repo, ['rev-parse', 'HEAD']);
  git(repo, ['switch', '-c', 'feature/graph']); writeFileSync(join(repo, 'file.txt'), 'middle\n'); git(repo, ['commit', '-am', 'Unlabelled graph commit']); const middle = git(repo, ['rev-parse', 'HEAD']);
  writeFileSync(join(repo, 'file.txt'), 'tip\n'); git(repo, ['commit', '-am', 'Graph action feature']); const tip = git(repo, ['rev-parse', 'HEAD']); git(repo, ['switch', 'main']);
  await connectRealCore(page, join(folder, 'graph-actions-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  const feature = page.locator(`button#commit-${tip}`); const root = page.locator(`button#commit-${base}`); const node = page.locator(`button#commit-${middle} .graph-mark`);
  await feature.click({ button: 'right' }); const menu = page.getByRole('menu', { name: 'Commit actions' }); await expect(menu).toBeVisible(); await expect(menu.getByRole('menuitem', { name: 'Checkout feature/graph', exact: true })).toBeVisible(); await page.screenshot({ path: testInfo.outputPath('graph-context-menu.png') });
  await menu.getByRole('menuitem', { name: 'Create branch here…', exact: true }).click(); await page.getByLabel('New branch name', { exact: true }).fill('review/from-graph'); await page.getByRole('button', { name: 'Review branch', exact: true }).click();
  const review = page.getByRole('region', { name: 'Review graph action' }); await expect(review).toContainText(tip.slice(0, 12)); expect(git(repo, ['branch', '--show-current'])).toBe('main'); await page.screenshot({ path: testInfo.outputPath('graph-create-branch-review.png') });
  await page.getByRole('button', { name: 'Create & open review/from-graph', exact: true }).click(); await expect(review).toBeHidden({ timeout: 30000 }); await expect(page.locator('.branch-current')).toBeEnabled(); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(tip); expect(git(repo, ['rev-parse', 'main'])).toBe(base); expect(git(repo, ['rev-parse', 'feature/graph'])).toBe(tip);
  // Two co-located local branches are ambiguous: double clicking the node opens choices.
  await feature.locator('.graph-mark').dblclick(); await expect(menu).toBeVisible(); await expect(menu.getByRole('menuitem', { name: 'Checkout feature/graph', exact: true })).toBeVisible(); await expect(menu.getByRole('menuitem', { name: /Checkout review\/from-graph/ })).toBeDisabled(); await page.keyboard.press('Escape');
  // A specific badge has exact identity even when the same commit has multiple branches.
  await feature.locator('[data-ref-name="refs/heads/feature/graph"]').dblclick(); await expect(page.getByRole('button', { name: 'Switch to feature/graph', exact: true })).toBeEnabled(); await page.getByRole('button', { name: 'Switch to feature/graph', exact: true }).click(); await expect(review).toBeHidden({ timeout: 30000 }); expect(git(repo, ['branch', '--show-current'])).toBe('feature/graph');
  await node.dblclick(); await expect(review).toContainText('Detached HEAD:'); await expect(review).toContainText(middle.slice(0, 12)); expect(git(repo, ['branch', '--show-current'])).toBe('feature/graph'); await page.getByRole('button', { name: 'Checkout commit (detached)', exact: true }).click(); await expect(review).toBeHidden({ timeout: 30000 }); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(middle); expect(git(repo, ['branch', '--show-current'])).toBe('');
  // A retained historical commit can be left by double clicking a local branch node.
  await root.locator('.graph-mark').dblclick(); await expect(page.getByRole('button', { name: 'Switch to main', exact: true })).toBeEnabled(); await page.getByRole('button', { name: 'Switch to main', exact: true }).click(); await expect(review).toBeHidden({ timeout: 30000 }); expect(git(repo, ['branch', '--show-current'])).toBe('main');
  await feature.focus(); await page.keyboard.press('Shift+F10'); await expect(menu).toBeVisible(); await page.keyboard.press('End'); await expect(menu.getByRole('menuitem').last()).toBeFocused(); await page.keyboard.press('Escape'); await expect(feature).toBeFocused();
  await root.click({ button: 'right' }); await menu.getByRole('menuitem', { name: 'Create tag…', exact: true }).click(); await page.getByLabel('Tag name', { exact: true }).fill('graph/v1'); await page.getByLabel('Tag type',{exact:true}).selectOption('lightweight'); await page.getByRole('button', { name: 'Review tag', exact: true }).click(); await expect(review).toContainText(base.slice(0, 12)); await page.getByRole('button', { name: 'Create reviewed tag', exact: true }).click(); await expect(review).toContainText('Tag created: graph/v1'); await review.getByRole('button',{name:'Close',exact:true}).click(); await expect(review).toBeHidden({ timeout: 30000 }); expect(git(repo, ['rev-parse', 'refs/tags/graph/v1'])).toBe(base); expect(git(repo, ['branch', '--show-current'])).toBe('main');
  // Keep menu controls reachable at the viewport boundary on a small desktop window.
  await page.setViewportSize({ width: 800, height: 600 }); await root.evaluate(node => node.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 790, clientY: 590, button: 2 }))); await expect(menu).toBeVisible();
  const box = await menu.boundingBox(); expect(box!.x).toBeGreaterThanOrEqual(0); expect(box!.y).toBeGreaterThanOrEqual(0); expect(box!.x + box!.width).toBeLessThanOrEqual(800); expect(box!.y + box!.height).toBeLessThanOrEqual(600); await page.screenshot({ path: testInfo.outputPath('graph-context-compact.png') });
  await page.keyboard.press('Escape'); await root.dispatchEvent('contextmenu'); await expect(menu).toBeVisible(); expect(browserErrors).toEqual([]);
});

test('double clicking a remote graph ref fetches and reviews its exact local tracking checkout', async ({ page }, testInfo) => {
  test.setTimeout(120000);
  const writer = join(folder, 'graph-remote-writer'); const repo = join(folder, 'graph-remote-local'); mkdirSync(writer); git(writer, ['init', '-b', 'main']); writeFileSync(join(writer, 'file.txt'), 'base\n'); git(writer, ['add', '.']); git(writer, ['commit', '-m', 'Remote graph base']);
  git(writer, ['switch', '-c', 'feature/context']); writeFileSync(join(writer, 'feature.txt'), 'cached\n'); git(writer, ['add', '.']); git(writer, ['commit', '-m', 'Remote graph cached']); git(writer, ['switch', 'main']); git(folder, ['clone', writer, repo]); git(repo, ['config', 'core.hooksPath', '.git/empty-hooks']);
  git(writer, ['switch', 'feature/context']); writeFileSync(join(writer, 'feature.txt'), 'latest\n'); git(writer, ['commit', '-am', 'Remote graph latest']); const latest = git(writer, ['rev-parse', 'HEAD']);
  await connectRealCore(page, join(folder, 'graph-remote-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  await page.locator('[data-ref-name="refs/remotes/origin/feature/context"]').dblclick(); const review = page.getByRole('region', { name: 'Review graph action' }); await expect(review).toContainText(latest.slice(0, 12), { timeout: 45000 }); expect(git(repo, ['branch', '--show-current'])).toBe('main'); await page.screenshot({ path: testInfo.outputPath('graph-remote-review.png') });
  await page.getByRole('button', { name: 'Create & open feature/context', exact: true }).click(); await expect(review).toBeHidden({ timeout: 30000 }); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(latest); expect(git(repo, ['rev-parse', '--symbolic-full-name', '@{upstream}'])).toBe('refs/remotes/origin/feature/context');
});

async function managementFixture(page: Page, label: string) {
  const repo = join(folder, label), bare = join(folder, `${label}.git`); mkdirSync(repo); git(repo, ['init', '-b', 'main']);
  writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Management root']); const base = git(repo, ['rev-parse', 'HEAD']);
  git(repo, ['switch', '-c', 'feature/manage']); writeFileSync(join(repo, 'file.txt'), 'feature\n'); git(repo, ['commit', '-am', 'Management feature']); const tip = git(repo, ['rev-parse', 'HEAD']); git(repo, ['branch', 'spare']); git(repo, ['-c', 'tag.gpgSign=false', 'tag', '-a', 'review/tag', '-m', 'Keep annotation']); const tagObject = git(repo, ['rev-parse', 'refs/tags/review/tag']); git(repo, ['switch', 'main']);
  git(folder, ['init', '--bare', bare]); git(repo, ['remote', 'add', 'origin', bare]); git(repo, ['push', 'origin', 'main']); git(repo, ['config', 'branch.main.description', 'Keep description']); writeFileSync(join(repo, 'file.txt'), 'uncommitted work stays\n');
  await connectRealCore(page, join(folder, `${label}-state.json`)); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  const manage = async (oid: string, kind: string, name: string) => { await page.locator(`button#commit-${oid}`).click({ button: 'right' }); await page.getByRole('menuitem', { name: `Manage ${kind} ${name}…`, exact: true }).click(); };
  return { repo, bare, base, tip, tagObject, manage, panel: page.locator('.ref-manager') };
}
test('graph management renames, configures upstream and retains deleted branch and annotated tag objects', async ({ page }, testInfo) => {
  test.setTimeout(180000); page.setDefaultTimeout(20000); const {repo,bare,base,tip,tagObject,manage,panel}=await managementFixture(page,'manage-refs');
  await manage(base,'branch','main'); await panel.getByRole('button',{name:'Rename branch…',exact:true}).click(); await page.getByLabel('New branch name',{exact:true}).fill('renamed/main'); await page.getByRole('button',{name:'Review rename',exact:true}).click(); await expect(panel).toContainText('main → renamed/main'); expect(git(repo,['branch','--show-current'])).toBe('main'); await page.screenshot({path:testInfo.outputPath('rename-branch-review.png')}); await panel.getByRole('button',{name:'Rename branch',exact:true}).click(); await expect(panel).toBeHidden({timeout:30000}); expect(git(repo,['branch','--show-current'])).toBe('renamed/main'); expect(git(repo,['config','branch.renamed/main.description'])).toBe('Keep description');
  await manage(base,'branch','renamed/main'); await panel.getByRole('button',{name:'Set or change upstream…',exact:true}).click(); await page.getByLabel('Remote',{exact:true}).selectOption('origin'); await page.getByLabel('Remote branch to track',{exact:true}).selectOption('main'); await page.getByRole('button',{name:'Review upstream',exact:true}).click(); await expect(panel).toContainText('refs/remotes/origin/main'); await panel.getByRole('button',{name:'Set upstream',exact:true}).click(); await expect(panel).toBeHidden({timeout:30000}); expect(git(repo,['rev-parse','--symbolic-full-name','@{upstream}'])).toBe('refs/remotes/origin/main');
  await manage(base,'branch','renamed/main'); await panel.getByRole('button',{name:'Remove upstream…',exact:true}).click(); await panel.getByRole('button',{name:'Remove upstream',exact:true}).click(); await expect(panel).toBeHidden({timeout:30000}); expect(git(repo,['for-each-ref','--format=%(upstream)','refs/heads/renamed/main'])).toBe('');
  await manage(tip,'branch','spare'); await panel.getByRole('button',{name:'Delete local branch…',exact:true}).click(); const backup=await panel.locator('.recovery-ref').innerText(); await page.screenshot({path:testInfo.outputPath('delete-branch-recovery-review.png')}); await panel.getByRole('button',{name:'Delete local branch',exact:true}).click(); await expect(panel).toBeHidden({timeout:30000}); expect(git(repo,['branch','--list','spare'])).toBe(''); expect(git(repo,['rev-parse',backup])).toBe(tip); await expect(page.getByRole('button',{name:'Copy recovery ref',exact:true})).toHaveAttribute('title',backup);
  await manage(tip,'tag','review/tag'); await panel.getByRole('button',{name:'Delete local tag…',exact:true}).click(); await expect(panel.locator('summary')).toBeVisible(); await panel.getByRole('button',{name:'Back',exact:true}).focus(); await page.keyboard.press('Shift+Tab'); await expect(panel.locator('summary')).toBeFocused(); await page.keyboard.press('Enter'); await expect(panel).toContainText('Keep annotation'); const tagBackup=await panel.locator('.recovery-ref').innerText(); await panel.getByRole('button',{name:'Delete local tag',exact:true}).click(); await expect(panel).toBeHidden({timeout:30000}); expect(git(repo,['tag','--list'])).toBe(''); expect(git(repo,['rev-parse',tagBackup])).toBe(tagObject); expect(git(bare,['rev-parse','main'])).toBe(base); expect(git(repo,['diff','--','file.txt'])).toContain('uncommitted work stays');
});
test('annotated tag review retains keyboard focus after pending animation frames', async ({page}) => {
  const {tip, manage, panel} = await managementFixture(page, 'tag-keyboard-focus');
  await manage(tip, 'tag', 'review/tag');
  await expect(panel.getByRole('button', {name:'Delete local tag…', exact:true})).toBeEnabled();
  // Release delayed focus callbacks after keyboard navigation, so the race
  // is reproducible without relying on CI load or arbitrary sleeps.
  const now = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({time:now});
  await page.clock.pauseAt(new Date(now.getTime()+1000));
  await panel.getByRole('button', {name:'Delete local tag…', exact:true}).click();
  const summary = panel.locator('summary');
  await expect(summary).toBeVisible();
  const back = panel.getByRole('button', {name:'Back', exact:true});
  await back.focus();
  await page.keyboard.press('Shift+Tab');
  await expect(summary).toBeFocused();
  await page.clock.runFor(32);
  await expect(summary).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(panel).toContainText('Keep annotation');
  await expect(panel.locator('details')).toHaveAttribute('open', '');
  await page.keyboard.press('Tab');
  await expect(back).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(panel.getByRole('button', {name:'Delete local tag', exact:true})).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(panel).toBeHidden();
});
test('graph push reviews first publication and sets upstream while preserving dirty work', async ({page},testInfo)=>{
  test.setTimeout(120000); const {repo,bare,base,tip,manage,panel}=await managementFixture(page,'push-graph'); await manage(tip,'branch','feature/manage'); await panel.getByRole('button',{name:'Push feature/manage…',exact:true}).click(); await page.getByLabel('Remote',{exact:true}).selectOption('origin'); await page.getByLabel('Destination branch',{exact:true}).fill('published/feature'); await page.getByLabel('Set this destination as upstream after push',{exact:true}).check(); await panel.getByText('Upload compatibility',{exact:true}).click();const buffered=panel.getByRole('checkbox',{name:'Use buffered HTTPS upload (16 MiB)',exact:true});await expect(buffered).not.toBeChecked();await buffered.check();await page.getByRole('button',{name:'Check destination & review',exact:true}).click();await expect(panel).toContainText('Buffered upload is available only for HTTPS destinations.');expect(git(bare,['for-each-ref','--format=%(refname)','refs/heads/published/feature'])).toBe('');await buffered.uncheck();await page.getByRole('button',{name:'Check destination & review',exact:true}).click(); await expect(panel).toContainText('Creates a new remote branch'); await expect(panel).toContainText('2 commits in this branch history'); await expect(panel).toContainText('First publication to this branch'); await expect(panel).toContainText(tip.slice(0,12)); expect(git(bare,['for-each-ref','--format=%(refname)','refs/heads/published/feature'])).toBe(''); await page.screenshot({path:testInfo.outputPath('push-outgoing-review.png')}); await panel.getByRole('button',{name:'Push reviewed branch',exact:true}).click(); await expect(panel).toBeHidden({timeout:45000}); expect(git(bare,['rev-parse','refs/heads/published/feature'])).toBe(tip); expect(git(bare,['rev-parse','main'])).toBe(base); expect(git(bare,['show','refs/heads/published/feature:file.txt'])).toBe('feature'); expect(git(repo,['config','branch.feature/manage.merge'])).toBe('refs/heads/published/feature'); expect(git(repo,['branch','--show-current'])).toBe('main'); await expect(page.locator('.status-line')).toContainText('Pushed feature/manage'); expect(git(repo,['diff','--','file.txt'])).toContain('uncommitted work stays');
});
test('graph push stops on a divergent destination',async({page},testInfo)=>{
  test.setTimeout(90000); const {repo,bare,tip,manage,panel}=await managementFixture(page,'divergent-push'); git(repo,['switch','-c','remote-side','main']); writeFileSync(join(repo,'remote.txt'),'remote work\n'); git(repo,['add','remote.txt']); git(repo,['commit','-m','Remote divergence']); const foreign=git(repo,['rev-parse','HEAD']); git(repo,['push','origin','remote-side:main']); git(repo,['switch','main']);
  await manage(tip,'branch','feature/manage'); await panel.getByRole('button',{name:'Push feature/manage…',exact:true}).click(); await page.getByLabel('Remote',{exact:true}).selectOption('origin'); await page.getByLabel('Destination branch',{exact:true}).fill('main'); await page.getByRole('button',{name:'Check destination & review',exact:true}).click(); await expect(panel).toContainText('destination is ahead or diverged'); await expect(panel.getByRole('button',{name:'Push reviewed branch',exact:true})).toBeDisabled(); await page.screenshot({path:testInfo.outputPath('push-divergence-blocked.png')}); expect(git(bare,['rev-parse','main'])).toBe(foreign);
});

test('graph merge conflict editor reviews actual result and finishes a two-parent commit', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  const repo = join(folder, 'merge-editor'); mkdirSync(repo);
  git(repo, ['init', '-b', 'main']); git(repo, ['config', 'user.name', 'Merge Test']); git(repo, ['config', 'user.email', 'merge@example.invalid']); git(repo, ['config', 'commit.gpgsign', 'false']);
  writeFileSync(join(repo, 'conflict.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Merge base']);
  git(repo, ['switch', '-c', 'incoming']); writeFileSync(join(repo, 'conflict.txt'), 'incoming version\n'); git(repo, ['commit', '-am', 'Incoming merge work']);
  const incoming = git(repo, ['rev-parse', 'HEAD']); git(repo, ['switch', 'main']); writeFileSync(join(repo, 'conflict.txt'), 'current version\n' + Array.from({ length: 20 }, (_, i) => `current context ${i}\n`).join('')); git(repo, ['commit', '-am', 'Current merge work']);
  const original = git(repo, ['rev-parse', 'HEAD']);
  await connectRealCore(page, join(folder, 'merge-editor-state.json')); await page.goto('/');
  await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  await page.getByRole('button', { name: /Incoming merge work/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Merge into main…', exact: true }).click();
  await expect(page.getByText('1 predicted conflicts.', { exact: false })).toBeVisible({ timeout: 30000 });
  expect(git(repo, ['rev-parse', 'HEAD'])).toBe(original);
  await page.getByRole('button', { name: 'Start merge', exact: true }).click();
  const inspector = page.getByLabel('Merge working changes', { exact: true });
  await expect(inspector).toBeVisible({ timeout: 45000 });
  await expect(inspector).toContainText('1 unresolved conflicts');
  await expect(page.getByLabel('Current conflict version',{exact:true})).toContainText('current version');
  await expect(page.getByLabel('Incoming conflict version',{exact:true})).toContainText('incoming version');
  await expect(page.getByRole('textbox',{name:'Edit merge result',exact:true})).toHaveAttribute('readonly','');
  await page.getByRole('button', { name: 'Edit result', exact: true }).click();
  const editor = page.getByRole('textbox', { name: 'Edit merge result', exact: true });
  await editor.fill(Array.from({ length: 400 }, (_, i) => `edited line ${i}`).join('\n'));
  await editor.evaluate(async node => { node.scrollTop = 1000; await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))); });
  // The shorter read-only pane must not drag the longer active editor back up.
  expect(await editor.evaluate(node => node.scrollTop)).toBeGreaterThanOrEqual(999);
  await editor.fill('combined reviewed result\n');
  await page.getByRole('button', { name: 'Review edited result', exact: true }).click();
  await expect(page.getByLabel('Merge diff', { exact: true })).toContainText('combined reviewed result', { timeout: 30000 });
  expect(readFileSync(join(repo, 'conflict.txt'), 'utf8')).toContain('<<<<<<<');
  await page.screenshot({ path: testInfo.outputPath('merge-resolution-review.png') });
  await page.getByRole('button', { name: 'Apply resolution', exact: true }).click();
  await expect(inspector).toContainText('All conflicts resolved.', { timeout: 45000 });
  expect(git(repo, ['show', ':conflict.txt'])).toBe('combined reviewed result');
  await page.getByLabel('Merge message', { exact: true }).fill('Merge incoming with reviewed resolution');
  await page.getByRole('button', { name: 'Review merge commit', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Finish merge', exact: true })).toBeEnabled({ timeout: 30000 });
  await page.getByRole('button', { name: 'Finish merge', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Merge incoming with reviewed resolution', exact: true })).toBeVisible({ timeout: 45000 });
  expect(git(repo, ['show', '-s', '--format=%P', 'HEAD'])).toBe(`${original} ${incoming}`);
  expect(readFileSync(join(repo, 'conflict.txt'), 'utf8')).toBe('combined reviewed result\n');
});

test('graph fast-forward and reviewed abort use the existing inspector', async ({ page }) => {
  test.setTimeout(180000);
  const repo = join(folder, 'merge-abort'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); git(repo, ['config', 'user.name', 'Merge Test']); git(repo, ['config', 'user.email', 'merge@example.invalid']); git(repo, ['config', 'commit.gpgsign', 'false']);
  writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Base for abort']);
  git(repo, ['switch', '-c', 'incoming']); writeFileSync(join(repo, 'file.txt'), 'incoming\n'); git(repo, ['commit', '-am', 'Incoming abort work']); const incoming = git(repo, ['rev-parse', 'HEAD']);
  git(repo, ['switch', 'main']); writeFileSync(join(repo, 'file.txt'), 'current\n'); git(repo, ['commit', '-am', 'Current abort work']); const original = git(repo, ['rev-parse', 'HEAD']);
  await connectRealCore(page, join(folder, 'merge-abort-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  await page.getByRole('button', { name: /Incoming abort work/ }).click({ button: 'right' }); await page.getByRole('menuitem', { name: 'Merge into main…', exact: true }).click(); await page.getByRole('button', { name: 'Start merge', exact: true }).click();
  await expect(page.getByLabel('Merge working changes', { exact: true })).toBeVisible({ timeout: 45000 });
  writeFileSync(join(repo, 'file.txt'), 'manual work before abort\n');
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(page.getByLabel('Merge diff', { exact: true })).toContainText('manual work before abort');
  await page.getByRole('button', { name: 'Review abort…', exact: true }).click();
  await expect(page.getByText('Recovery copy:', { exact: false })).toBeVisible({ timeout: 30000 });
  await page.getByRole('button', { name: 'Abort merge & save edits', exact: true }).click();
  await expect(page.getByLabel('Merge working changes', { exact: true })).toHaveCount(0, { timeout: 45000 });
  expect(git(repo, ['rev-parse', 'HEAD'])).toBe(original); expect(readFileSync(join(repo, 'file.txt'), 'utf8')).toBe('current\n');
  expect(git(repo, ['for-each-ref', '--format=%(objectname)', 'refs/gitnova/recovery/'])).toContain(incoming);
  // A separate clean branch fast-forwards to the same incoming commit without creating a merge commit.
  git(repo, ['switch', '-c', 'ff-check', `${incoming}^`]); await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await page.getByRole('button', { name: /Incoming abort work/ }).click({ button: 'right' }); await page.getByRole('menuitem', { name: 'Merge into ff-check…', exact: true }).click();
  await page.getByLabel('Merge method', { exact: true }).selectOption('fastForwardOnly');
  await page.getByRole('button', { name: 'Fast-forward branch', exact: true }).click();
  await expect.poll(() => git(repo, ['rev-parse', 'HEAD']), { timeout: 45000 }).toBe(incoming);
});

test('graph cherry-pick resolves and commits one parent, then revert adds an inverse commit', async ({ page }, testInfo) => {
  test.setTimeout(240000);
  const repo = join(folder, 'patch-ui'); mkdirSync(repo); git(repo, ['init', '-b', 'main']);
  writeFileSync(join(repo, 'change.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Patch base']);
  git(repo, ['switch', '-c', 'source']); writeFileSync(join(repo, 'change.txt'), 'selected source\n'); git(repo, ['commit', '-am', 'Selected patch', '--author=Original Writer <original@example.invalid>']); const source = git(repo, ['rev-parse', 'HEAD']);
  git(repo, ['switch', 'main']); writeFileSync(join(repo, 'change.txt'), 'current destination\n'); git(repo, ['commit', '-am', 'Destination change']); const original = git(repo, ['rev-parse', 'HEAD']);
  await connectRealCore(page, join(folder, 'patch-ui-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  await page.locator(`button#commit-${source}`).click({ button: 'right' }); await page.getByRole('menuitem', { name: 'Cherry-pick onto main…', exact: true }).click();
  await expect(page.getByText('1 predicted conflicts.', { exact: false })).toBeVisible({ timeout: 30000 }); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(original);
  await page.getByRole('button', { name: 'Start cherry-pick', exact: true }).click(); const inspector = page.getByLabel('Cherry-pick working changes', { exact: true }); await expect(inspector).toBeVisible({ timeout: 45000 });
  await inspector.getByText('Whole-file choices',{exact:true}).click(); await page.getByRole('button', { name: 'Use incoming', exact: true }).click(); await expect(page.getByRole('button', { name: 'Apply resolution', exact: true })).toBeEnabled({ timeout: 30000 });
  await page.screenshot({ path: testInfo.outputPath('cherry-pick-resolution.png') });
  await page.getByRole('button', { name: 'Apply resolution', exact: true }).click(); await expect(inspector).toContainText('All conflicts resolved.', { timeout: 45000 });
  await page.getByRole('button', { name: 'Review cherry-pick commit', exact: true }).click(); await page.getByRole('button', { name: 'Finish cherry-pick', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Selected patch', exact: true })).toBeVisible({ timeout: 45000 }); const picked = git(repo, ['rev-parse', 'HEAD']); expect(git(repo, ['show', '-s', '--format=%P', 'HEAD'])).toBe(original); expect(git(repo, ['show', '-s', '--format=%an', 'HEAD'])).toBe('Original Writer');
  await page.locator(`button#commit-${picked}`).click({ button: 'right' }); await page.getByRole('menuitem', { name: 'Revert on main…', exact: true }).click(); await page.getByRole('button', { name: 'Start revert', exact: true }).click();
  await expect(page.getByLabel('Revert working changes', { exact: true })).toBeVisible({ timeout: 45000 }); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(picked);
  await page.getByRole('button', { name: 'Review revert commit', exact: true }).click(); await page.getByRole('button', { name: 'Finish revert', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Revert "Selected patch"', exact: true })).toBeVisible({ timeout: 45000 }); expect(git(repo, ['show', '-s', '--format=%P', 'HEAD'])).toBe(picked); expect(readFileSync(join(repo, 'change.txt'), 'utf8')).toBe('current destination\n'); expect(git(repo, ['rev-parse', 'source'])).toBe(source);
});

test('reverting a graph merge requires a mainline parent and reviewed abort restores it', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  const repo = join(folder, 'mainline-ui'); mkdirSync(repo); git(repo, ['init', '-b', 'main']);
  writeFileSync(join(repo, 'base.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Mainline base']);
  git(repo, ['switch', '-c', 'source']); writeFileSync(join(repo, 'source.txt'), 'source\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Source change']);
  git(repo, ['switch', 'main']); writeFileSync(join(repo, 'main.txt'), 'main\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Mainline change']); git(repo, ['merge', '--no-ff', 'source', '-m', 'Selected merge for revert']); const original = git(repo, ['rev-parse', 'HEAD']);
  await connectRealCore(page, join(folder, 'mainline-ui-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  await page.locator(`button#commit-${original}`).click({ button: 'right' }); await page.getByRole('menuitem', { name: 'Revert on main…', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Start revert', exact: true })).toBeDisabled(); await page.getByLabel('Mainline parent · change baseline', { exact: true }).selectOption('1');
  await expect(page.getByRole('button', { name: 'Start revert', exact: true })).toBeEnabled({ timeout: 30000 }); await page.screenshot({ path: testInfo.outputPath('revert-mainline-review.png') });
  await page.getByRole('button', { name: 'Start revert', exact: true }).click(); const inspector = page.getByLabel('Revert working changes', { exact: true }); await expect(inspector).toBeVisible({ timeout: 45000 }); await expect(inspector).toContainText('parent 1');
  await page.getByRole('button', { name: 'Review abort…', exact: true }).click(); await expect(inspector).toContainText('Git reset --merge', { timeout: 30000 }); await page.getByRole('button', { name: 'Abort revert & save edits', exact: true }).click();
  await expect(inspector).toHaveCount(0, { timeout: 45000 }); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(original); expect(readFileSync(join(repo, 'source.txt'), 'utf8')).toBe('source\n');
});

function searchHistory(repo: string, count: number) {
  const parent = git(repo, ['rev-parse', 'HEAD']); let script = '';
  for (let i = 0; i < count; i++) {
    const message = i === 10 ? 'Workspace search needle\n\nNOVA-SEARCH older body line\n' : `Web history ${i}\n`;
    const data = `value ${i}\n`;
    script += `commit refs/heads/main\ncommitter Bob Workspace <bob@example.invalid> ${1700000000 + i} +0000\ndata ${Buffer.byteLength(message)}\n${message}${i === 0 ? `from ${parent}\n` : ''}M 100644 inline history.txt\ndata ${Buffer.byteLength(data)}\n${data}\n`;
  }
  script += 'done\n'; execFileSync('git', ['-C', repo, 'fast-import', '--quiet'], { input: script, stdio: ['pipe', 'pipe', 'pipe'] }); git(repo, ['reset', '--hard', 'HEAD']);
}
async function addSearchRepo(page: Page, path: string) {
  await page.locator('.repo-picker').click(); await page.getByLabel('Add an existing repository').fill(path); await page.getByRole('button', { name: 'Open', exact: true }).click();
}
test('workspace commit search scopes locate old history without checkout and preserve repository context', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  const origin = join(folder, 'search-api'), web = join(folder, 'search-web');
  for (const repo of [origin, web]) { mkdirSync(repo); git(repo, ['init', '-b', 'main']); writeFileSync(join(repo, 'base.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Search fixture base']); }
  searchHistory(web, 260); const target = git(web, ['rev-parse', 'HEAD~249']); const head = git(web, ['rev-parse', 'HEAD']); const index = readFileSync(join(web, '.git/index'));
  await connectRealCore(page, join(folder, 'search-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(origin); await page.getByRole('button', { name: 'Open', exact: true }).click(); await addSearchRepo(page, web);
  await page.locator('.repo-picker').click(); await page.locator('.repo-option-main').filter({ hasText: 'search-api' }).click();
  await expect(page.locator('.repo-picker')).toContainText('search-api'); await expect(page.getByRole('button', { name: /Search commits/ })).toBeEnabled();
  await page.keyboard.press('ControlOrMeta+k'); const search = page.getByRole('region', { name: 'Commit search', exact: true }); await expect(search).toBeVisible();
  await page.getByLabel('Search commit text, author or SHA', { exact: true }).fill('Workspace search needle'); await expect(search).toContainText('No matching commits in this scope.');
  await page.getByLabel('Commit search scope', { exact: true }).selectOption('workspace'); const result = search.locator('.commit-search-result'); await expect(result).toHaveCount(1); await expect(result).toContainText('search-web');
  await page.getByLabel('Commit search scope', { exact: true }).selectOption('selected'); await page.getByRole('checkbox', { name: /search-api/ }).uncheck(); await page.getByRole('checkbox', { name: /search-web/ }).check(); await expect(result).toHaveCount(1);
  await page.getByLabel('Commit search scope', { exact: true }).selectOption('workspace'); await expect(result).toHaveCount(1); await page.screenshot({ path: testInfo.outputPath('workspace-search-results.png') });
  await page.getByLabel('Search commit text, author or SHA', { exact: true }).focus(); await page.keyboard.press('ArrowDown'); await expect(result).toBeFocused(); await page.keyboard.press('Enter');
  await expect(search).toBeHidden({ timeout: 30000 }); await expect(page.locator('.repo-picker')).toContainText('search-web'); await expect(page.getByRole('heading', { name: 'Workspace search needle', exact: true })).toBeVisible(); await expect(page.locator(`button#commit-${target}`)).toBeFocused();
  await expect(page.getByLabel('Commit diff', { exact: true })).toContainText('value 10'); expect(git(web, ['rev-parse', 'HEAD'])).toBe(head); expect(git(web, ['symbolic-ref', '--short', 'HEAD'])).toBe('main'); expect(readFileSync(join(web, '.git/index'))).toEqual(index);
  await expect(page.locator('.history-wrap')).toContainText('Showing older history'); await page.screenshot({ path: testInfo.outputPath('workspace-search-focused-history.png') });
  await page.locator('.repo-picker').click(); await page.locator('.repo-option-main').filter({ hasText: 'search-api' }).click(); await expect(page.locator('.commit-row.selected')).toContainText('Search fixture base');
});

test('search reports unavailable repositories and retains failed-navigation context', async ({ page }, testInfo) => {
  test.setTimeout(120000); await page.setViewportSize({ width: 980, height: 760 });
  const origin = join(folder, 'search-origin'), target = join(folder, 'search-target'), moved = join(folder, 'search-target-moved');
  for (const [repo, subject] of [[origin, 'Stable search origin'], [target, 'Distinct target needle']]) { mkdirSync(repo); git(repo, ['init', '-b', 'main']); writeFileSync(join(repo, 'base.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', subject]); }
  await connectRealCore(page, join(folder, 'search-failure-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(origin); await page.getByRole('button', { name: 'Open', exact: true }).click(); await addSearchRepo(page, target);
  await page.locator('.repo-picker').click(); await page.locator('.repo-option-main').filter({ hasText: 'search-origin' }).click(); await page.getByRole('button', { name: /Search commits/ }).click(); const search = page.getByRole('region', { name: 'Commit search', exact: true });
  await page.getByLabel('Commit search scope', { exact: true }).selectOption('workspace'); await page.getByLabel('Search commit text, author or SHA', { exact: true }).fill('Distinct target needle'); await expect(search.locator('.commit-search-result')).toHaveCount(1);
  renameSync(target, moved); await search.locator('.commit-search-result').click(); await expect(search.getByRole('alert')).toContainText(/cannot change|No such file|not.*repository/i); await expect(page.locator('.repo-picker')).toContainText('search-origin');
  await page.getByLabel('Search commit text, author or SHA', { exact: true }).fill('Stable search origin'); await expect(search).toContainText('Results are incomplete.'); await expect(search.locator('.commit-search-result')).toHaveCount(1); await expect(search.locator('.commit-search-result')).toContainText('search-origin');
  const bounds = await search.boundingBox(); expect(bounds!.x).toBeGreaterThanOrEqual(0); expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(981); await page.screenshot({ path: testInfo.outputPath('workspace-search-partial.png') });
  renameSync(moved, target); await search.getByRole('button', { name: 'Search', exact: true }).click(); await expect(search.getByText('Results are incomplete.', { exact: true })).toHaveCount(0); await page.keyboard.press('Escape'); await expect(page.getByRole('button', { name: /Search commits/ })).toBeFocused();
});

test('blocked merge leads to working changes without claiming a predicted merge', async ({ page }, testInfo) => {
  const repo = join(folder, 'blocked-merge'); mkdirSync(repo); git(repo, ['init', '-b', 'main']);
  writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Working changes']);
  git(repo, ['switch', '-c', 'feature']); writeFileSync(join(repo, 'feature.txt'), 'feature\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Feature change']); const tip = git(repo, ['rev-parse', 'HEAD']); git(repo, ['switch', 'main']); writeFileSync(join(repo, 'file.txt'), 'keep my draft\n');
  const head = git(repo, ['rev-parse', 'HEAD']), index = readFileSync(join(repo, '.git/index'));
  await connectRealCore(page, join(folder, 'blocked-merge-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', {name: 'Open', exact: true}).click();
  await page.locator(`button#commit-${tip}`).click({button: 'right'}); await page.getByRole('menuitem', {name: 'Merge into main…', exact: true}).click();
  const review = page.locator('.merge-start-review'); await expect(review).toContainText('Commit or preserve uncommitted work'); await expect(review).not.toContainText('Merge commit:'); await expect(review.getByRole('button', {name: 'Start merge', exact: true})).toBeDisabled();
  await page.screenshot({path: testInfo.outputPath('blocked-merge-review.png')}); await review.getByRole('button', {name: 'Review working changes', exact: true}).click(); await expect(review).toBeHidden(); await expect(page.getByLabel('Summary', {exact: true})).toBeVisible();
  expect(git(repo, ['rev-parse', 'HEAD'])).toBe(head); expect(readFileSync(join(repo, '.git/index'))).toEqual(index); expect(readFileSync(join(repo, 'file.txt'), 'utf8')).toBe('keep my draft\n');
});

test('large historical commits retain all files and load an exact late patch', async ({ page }, testInfo) => {
  const repo=join(folder,'large-history');mkdirSync(repo);git(repo,['init','-b','main']);
  writeFileSync(join(repo,'a-large.txt'),'large line content\n'.repeat(40000));for(let i=0;i<254;i++)writeFileSync(join(repo,`z-${String(i).padStart(3,'0')}.txt`),`late-file-${i}\n`);
  writeFileSync(join(repo,'z-binary.dat'),Buffer.from([0,255,1,2]));git(repo,['add','.']);git(repo,['commit','-m','Large historical comparison']);const head=git(repo,['rev-parse','HEAD']);
  await connectRealCore(page,join(folder,'large-history-state.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  await expect(page.getByRole('button',{name:'View 256 changed files',exact:true})).toBeVisible();expect(await page.locator('.dock-files button').count()).toBeLessThan(40);await expect(page.locator('.dock-file-heading')).toContainText('+40000');
  await page.locator('.dock-files button').first().focus();await page.keyboard.press('End');await expect(page.locator('.dock-files button[title="z-binary.dat"]')).toBeFocused();await page.keyboard.press('ArrowUp');await expect(page.locator('.dock-files button[title="z-253.txt"]')).toBeFocused();await page.keyboard.press('Home');await expect(page.locator('.dock-files button[title="a-large.txt"]')).toBeFocused();await page.keyboard.press('End');await page.keyboard.press('ArrowUp');await page.locator('.dock-files button').filter({hasText:'z-253.txt'}).click();await expect(page.locator('.dock-file-heading')).toContainText('z-253.txt');await expect(page.getByRole('region',{name:'After code',exact:true})).toContainText('late-file-253');await expect(page.getByRole('region',{name:'After code',exact:true})).not.toContainText('large line content');await expect(page.locator('.commit-diff-dock .notice')).toHaveCount(0);
  await page.screenshot({path:testInfo.outputPath('complete-historical-file-list.png')});expect(git(repo,['rev-parse','HEAD'])).toBe(head);
});

test('English and Thai persist without translating repository data or losing a draft', async ({ page }, testInfo) => {
  await page.setViewportSize({width:1100,height:800});const repo=join(folder,'locale-project');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'Working changes\n');git(repo,['add','.']);git(repo,['commit','-m','Working changes']);writeFileSync(join(repo,'file.txt'),'Commit message\n');
  await connectRealCore(page,join(folder,'locale-state.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.commit-row .subject')).toHaveText('Working changes');
  await page.locator('.working-row').click();await page.getByLabel('Summary',{exact:true}).fill('Keep this exact draft {0}');await page.getByLabel('Description',{exact:false}).fill('Description in my own language');
  await page.keyboard.press('Control+,');await page.getByLabel('Language',{exact:true}).selectOption('th');await expect(page.locator('html')).toHaveAttribute('lang','th');await page.keyboard.press('Escape');
  await expect(page.getByLabel('สรุป',{exact:true})).toHaveValue('Keep this exact draft {0}');await expect(page.getByLabel('รายละเอียด',{exact:false})).toHaveValue('Description in my own language');await expect(page.getByRole('button',{name:'ตรวจ commit',exact:true})).toBeDisabled();await expect(page.locator('.commit-row .subject')).toHaveText('Working changes');
  await expect(page.getByRole('region',{name:'โค้ดหลังเปลี่ยน',exact:true})).toContainText('Commit message');await page.screenshot({path:testInfo.outputPath('thai-working-changes.png')});
  await page.keyboard.press('Control+k');await expect(page.getByRole('region',{name:'ค้นหา commit',exact:true})).toBeVisible();await page.getByLabel('ขอบเขตค้นหา commit',{exact:true}).selectOption('workspace');await page.getByLabel('ค้นหาข้อความ commit ผู้เขียน หรือ SHA',{exact:true}).fill('Working changes');await expect(page.locator('.commit-search-result strong')).toHaveText('Working changes');await page.screenshot({path:testInfo.outputPath('thai-search.png')});await page.keyboard.press('Escape');
  await page.reload();await expect(page.locator('html')).toHaveAttribute('lang','th');await expect(page.locator('.working-row')).toContainText('งานที่เปลี่ยน');await page.keyboard.press('Control+,');await page.getByLabel('ภาษา',{exact:true}).selectOption('en');await page.keyboard.press('Escape');await expect(page.locator('html')).toHaveAttribute('lang','en');await expect(page.locator('.working-row')).toContainText('Working changes');
});

test('Thai local and remote branch reviews localize confirmation without checking out', async ({ page }, testInfo) => {
  test.setTimeout(120000);
  const writer=join(folder,'thai-branch-writer'),repo=join(folder,'thai-branch-local');mkdirSync(writer);git(writer,['init','-b','main']);writeFileSync(join(writer,'file.txt'),'base\n');git(writer,['add','.']);git(writer,['commit','-m','Keep repository message']);git(writer,['branch','feature/locale']);git(folder,['clone',writer,repo]);git(repo,['branch','local/preview']);
  const head=git(repo,['rev-parse','HEAD']);const index=readFileSync(join(repo,'.git/index'));
  await connectRealCore(page,join(folder,'thai-branch-state.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.getByRole('button',{name:'Switch branch',exact:true})).toBeEnabled();
  await page.keyboard.press('Control+,');await page.getByLabel('Language',{exact:true}).selectOption('th');await page.keyboard.press('Escape');await page.getByRole('button',{name:'สลับ branch',exact:true}).click();await page.getByRole('button',{name:'เลือก branch local/preview',exact:true}).click();await expect(page.locator('.branch-preview .primary')).toHaveText('สลับไป local/preview');await page.keyboard.press('Escape');
  await page.getByRole('button',{name:'สลับ branch',exact:true}).click();await page.getByRole('button',{name:/^Remote /}).click();await page.getByRole('button',{name:'เปิด remote origin/feature/locale',exact:true}).click();await expect(page.locator('.branch-preview .primary')).toHaveText('สร้างและเปิด feature/locale',{timeout:30000});await expect(page.locator('.branch-preview .primary')).toBeEnabled();await page.screenshot({path:testInfo.outputPath('thai-remote-branch-review.png')});await page.keyboard.press('Escape');
  await page.locator(`button#commit-${head}`).click({button:'right'});await page.getByRole('menuitem',{name:/^Fetch และเปิด origin\/feature\/locale/}).click();await expect(page.locator('.graph-actions .primary')).toHaveText('สร้างและเปิด feature/locale',{timeout:30000});await expect(page.locator('.graph-actions .primary')).toBeEnabled();await page.keyboard.press('Escape');
  expect(git(repo,['branch','--show-current'])).toBe('main');expect(git(repo,['rev-parse','HEAD'])).toBe(head);expect(readFileSync(join(repo,'.git/index'))).toEqual(index);
});

test('large stash loads selected files beyond the code budget and pops complete working and index contents',async({page},testInfo)=>{
 test.setTimeout(240000);const repo=join(folder,'large-stash-ui');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Large stash base']);const head=git(repo,['rev-parse','HEAD']);const staged='staged start\n'+'staged line\n'.repeat(60000),working='working start\n'+'working line\n'.repeat(60000);writeFileSync(join(repo,'file.txt'),staged);git(repo,['add','file.txt']);writeFileSync(join(repo,'file.txt'),working);writeFileSync(join(repo,'zz-note.txt'),'last saved file\n');const index=readFileSync(join(repo,'.git/index'));
 await connectRealCore(page,join(folder,'large-stash-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-stash-actions').getByRole('button',{name:'Stash…',exact:true}).click();const panel=page.locator('.stash-controls');await expect(panel).toContainText('The complete file list is included.');await expect(panel.getByRole('button',{name:'Stash on main',exact:true})).toBeEnabled({timeout:45000});await expect(page.getByText('This file preview is partial. The stash operation includes the complete reviewed scope.',{exact:true})).toBeVisible();await panel.locator('.stash-review-files').getByRole('button',{name:'zz-note.txt',exact:true}).click();await expect(page.getByRole('region',{name:'After code',exact:true})).toContainText('last saved file');await panel.getByText('Options and recovery',{exact:true}).click();await panel.getByRole('button',{name:'Saved index',exact:true}).click();await expect(page.getByText('This path has no changes in the selected saved version.',{exact:true})).toBeVisible();await panel.locator('.stash-review-files').getByRole('button',{name:'file.txt',exact:true}).click();await expect(page.getByRole('region',{name:'After code',exact:true})).toContainText('staged start');await panel.getByRole('button',{name:'Saved working tree',exact:true}).click();await expect(page.getByRole('region',{name:'After code',exact:true})).toContainText('working start');expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe(working);expect(readFileSync(join(repo,'.git/index'))).toEqual(index);await page.screenshot({path:testInfo.outputPath('large-stash-file-review.png')});await panel.getByRole('button',{name:'Stash on main',exact:true}).click();await expect(panel).toHaveCount(0,{timeout:45000});expect(git(repo,['status','--porcelain'])).toBe('');
 await page.locator('.working-stash-actions').getByRole('button',{name:'Pop…',exact:true}).click();await expect(panel.getByRole('button',{name:'Pop on main',exact:true})).toBeEnabled({timeout:45000});await panel.locator('.stash-review-files').getByRole('button',{name:'zz-note.txt',exact:true}).click();await expect(page.getByRole('region',{name:'After code',exact:true})).toContainText('last saved file');await panel.getByText('Options and recovery',{exact:true}).click();await panel.getByRole('checkbox',{name:'Restore the saved staging state',exact:true}).check();await panel.getByRole('button',{name:'Pop on main',exact:true}).click();await expect(panel).toHaveCount(0,{timeout:45000});expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe(working);expect(git(repo,['show',':file.txt'])).toBe(staged.trim());expect(readFileSync(join(repo,'zz-note.txt'),'utf8')).toBe('last saved file\n');expect(git(repo,['rev-parse','HEAD'])).toBe(head);expect(git(repo,['stash','list'])).toBe('');
});
test('stash advanced options retain saved staging, apply/drop and composer drafts', async ({ page }, testInfo) => {
  test.setTimeout(300000);
  const repo = join(folder, 'stash-roundtrip'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); git(repo, ['config', 'core.hooksPath', '/dev/null']); writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Stash base']); const head = git(repo, ['rev-parse', 'HEAD']); writeFileSync(join(repo, 'file.txt'), 'staged version\n'); git(repo, ['add', 'file.txt']); writeFileSync(join(repo, 'file.txt'), 'working version\n'); writeFileSync(join(repo, 'new ภาษาไทย.txt'), 'saved untracked\n');
  await connectRealCore(page, join(folder, 'stash-roundtrip.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click(); await page.locator('.working-row').click(); await page.getByLabel('Summary', { exact: true }).fill('Keep my commit draft');
  const panel = page.locator('.stash-controls');
  const save = async () => { await page.locator('.working-stash-actions').getByRole('button', { name: 'Stash…', exact: true }).click(); await expect(panel.getByRole('button', { name: 'Stash on main', exact: true })).toBeEnabled(); await expect(panel.locator('.stash-review-files')).toContainText('new ภาษาไทย.txt'); await panel.getByText('Options and recovery', { exact: true }).click(); await panel.getByRole('button', { name: 'Saved index', exact: true }).click(); await expect(page.getByRole('region', { name: 'After code', exact: true })).toContainText('staged version'); await panel.getByRole('button', { name: 'Saved working tree', exact: true }).click(); await expect(page.getByRole('region', { name: 'After code', exact: true })).toContainText('working version'); await page.screenshot({ path: testInfo.outputPath('stash-capture-review.png') }); await panel.getByRole('button', { name: 'Stash on main', exact: true }).click(); await expect(panel).toHaveCount(0, { timeout: 45000 }); await expect(page.getByLabel('Summary', { exact: true })).toHaveValue('Keep my commit draft'); };
  await save(); expect(git(repo, ['status', '--porcelain'])).toBe(''); await page.locator('.stash-graph-row').click(); await panel.getByText('Options and recovery', { exact: true }).click(); await panel.getByLabel('Keep the stash after restoring', { exact: true }).check(); await expect(panel.getByRole('button', { name: 'Apply on main', exact: true })).toBeEnabled(); await panel.getByLabel('Restore the saved staging state', { exact: true }).check(); await panel.getByRole('button', { name: 'Apply on main', exact: true }).click(); await expect(panel).toHaveCount(0, { timeout: 45000 }); expect(git(repo, ['show', ':file.txt'])).toBe('staged version'); expect(readFileSync(join(repo, 'file.txt'), 'utf8')).toBe('working version\n');
  await page.locator('.stash-graph-row').click(); await panel.getByText('Options and recovery', { exact: true }).click(); await panel.getByRole('button', { name: 'Remove saved stash…', exact: true }).click(); await expect(panel.getByRole('button', { name: 'Remove saved stash', exact: true })).toBeEnabled(); const recovery = await panel.locator('.recovery-ref').innerText(); await panel.getByRole('button', { name: 'Remove saved stash', exact: true }).click(); await expect(panel).toHaveCount(0, { timeout: 45000 }); expect(git(repo, ['cat-file', '-t', recovery])).toBe('commit'); expect(readFileSync(join(repo, 'file.txt'), 'utf8')).toBe('working version\n');
  await save(); await page.locator('.working-stash-actions').getByRole('button', { name: 'Pop…', exact: true }).click(); await panel.getByText('Options and recovery', { exact: true }).click(); await panel.getByLabel('Restore the saved staging state', { exact: true }).check(); await panel.getByRole('button', { name: 'Pop on main', exact: true }).click(); await expect(panel).toHaveCount(0, { timeout: 45000 }); expect(git(repo, ['stash', 'list'])).toBe(''); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(head); expect(git(repo, ['show', ':file.txt'])).toBe('staged version'); await expect(page.getByLabel('Summary', { exact: true })).toHaveValue('Keep my commit draft');
});

test('Thai conflicted stash pop keeps saved entry and recovery with explicit Git resolution guidance', async ({ page }, testInfo) => {
  test.setTimeout(120000); const repo = join(folder, 'stash-conflict'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Base']); writeFileSync(join(repo, 'file.txt'), 'saved version\n'); git(repo, ['stash', 'push', '-m', 'External saved work']); const saved = git(repo, ['rev-parse', 'refs/stash']); writeFileSync(join(repo, 'file.txt'), 'destination version\n'); git(repo, ['commit', '-am', 'Destination']); const head = git(repo, ['rev-parse', 'HEAD']);
  await connectRealCore(page, join(folder, 'stash-conflict.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click(); await page.keyboard.press('Control+,'); await page.getByLabel('Language', { exact: true }).selectOption('th'); await page.keyboard.press('Escape'); await page.locator('.stash-graph-row').click(); const panel = page.locator('.stash-controls'); await expect(panel.getByRole('button', { name: 'นำ stash กลับลง main', exact: true })).toBeEnabled(); await panel.getByText('ตัวเลือกและการกู้คืน', { exact: true }).click(); await expect(panel.getByLabel('คืนสถานะ staging ที่บันทึกไว้', { exact: true })).not.toBeChecked(); const recovery = await panel.locator('.recovery-ref').innerText(); await panel.getByRole('button', { name: 'นำ stash กลับลง main', exact: true }).click(); await expect(panel).toContainText('แก้ conflict ของ stash ด้วย Git', { timeout: 45000 }); expect(git(repo, ['rev-parse', 'refs/stash'])).toBe(saved); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(head); expect(git(repo, ['ls-files', '--unmerged'])).not.toBe(''); expect(git(repo, ['cat-file', '-t', recovery])).toBe('commit'); await page.screenshot({ path: testInfo.outputPath('thai-stash-conflict-retained.png') }); await panel.getByRole('button', { name: 'ตรวจงานที่เปลี่ยน', exact: true }).click(); await expect(page.locator('.working-detail')).toContainText('แก้ conflict ของ stash ด้วย Git');
});

test('graph creates a reviewed worktree and opens it without disturbing source drafts', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  const source = join(folder, 'worktree-source'); const target = join(folder, 'งาน-worktree');
  mkdirSync(source); git(source, ['init', '-b', 'main']); writeFileSync(join(source, 'file.txt'), 'base\n'); git(source, ['add', '.']); git(source, ['commit', '-m', 'Worktree source commit']); git(source, ['config', 'core.hooksPath', '/dev/null']);
  writeFileSync(join(source, 'file.txt'), 'staged\n'); git(source, ['add', '.']); writeFileSync(join(source, 'file.txt'), 'working\n');
  const index = readFileSync(join(source, '.git/index')); const head = git(source, ['rev-parse', 'HEAD']);
  await connectRealCore(page, join(folder, 'worktree-ui-state.json')); await page.goto('/');
  await page.getByLabel('Add an existing repository').fill(source); await page.getByRole('button', { name: 'Open', exact: true }).click();
  await page.locator('.working-row').click(); await page.getByLabel('Summary', { exact: true }).fill('Keep my source draft');
  await page.getByRole('button', { name: /Worktree source commit/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Create worktree here…', exact: true }).click();
  await page.getByLabel('Branch name', { exact: true }).fill('feature/worktree'); await page.getByLabel('New folder (absolute path)').fill(target);
  await page.getByRole('button', { name: 'Review worktree', exact: true }).click();
  await expect(page.getByText('1 path checked out into a separate folder.', { exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('worktree-create-review.png') });
  expect(git(source, ['worktree', 'list', '--porcelain'])).not.toContain('feature/worktree');
  await page.getByRole('button', { name: 'Create worktree', exact: true }).click();
  await expect(page.getByText('Worktree created; open it from Related repositories', { exact: true })).toBeVisible({ timeout: 45000 });
  expect(git(source, ['branch', '--show-current'])).toBe('main'); expect(git(source, ['rev-parse', 'HEAD'])).toBe(head); expect(readFileSync(join(source, '.git/index'))).toEqual(index);
  expect(readFileSync(join(source, 'file.txt'), 'utf8')).toBe('working\n'); expect(git(target, ['rev-parse', 'HEAD'])).toBe(head); expect(readFileSync(join(target, 'file.txt'), 'utf8')).toBe('base\n');
  await page.locator('.working-row').click(); await expect(page.getByLabel('Summary', { exact: true })).toHaveValue('Keep my source draft');
  await page.keyboard.press('Control+o');
  await page.locator('.related').getByRole('button', { name: /feature\/worktree/ }).click();
  await expect(page.getByText('Working tree clean', { exact: true })).toBeVisible();
  await expect(page.locator('.branch-current')).toContainText('feature/worktree');
});

test('Thai worktree creation blocks an occupied branch and stale destination', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  const source = join(folder, 'worktree-th-source'); const target = join(folder, 'worktree-th-target');
  mkdirSync(source); git(source, ['init', '-b', 'main']); writeFileSync(join(source, 'file.txt'), 'base\n'); git(source, ['add', '.']); git(source, ['commit', '-m', 'Thai worktree source']); git(source, ['config', 'core.hooksPath', '/dev/null']);
  await connectRealCore(page, join(folder, 'worktree-th-state.json'));
  await page.addInitScript(() => localStorage.setItem('gitnova.locale', 'th'));
  await page.goto('/');
  await page.getByLabel('เพิ่ม repository ที่มีอยู่').fill(source); await page.getByRole('button', { name: 'เปิด', exact: true }).click();
  await page.getByRole('button', { name: /Thai worktree source/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'สร้าง worktree ที่นี่…', exact: true }).click();
  await page.getByLabel('เช็คเอาต์ในโฟลเดอร์ใหม่').selectOption('existingBranch'); await page.getByLabel('โฟลเดอร์ใหม่ (พาธแบบเต็ม)').fill(target);
  await page.getByRole('button', { name: 'ตรวจทาน worktree', exact: true }).click();
  await expect(page.getByText('Branch นี้ถูกเช็คเอาต์ใน worktree อื่นแล้ว เลือก branch อื่นหรือ detached HEAD', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'สร้าง worktree', exact: true })).toBeDisabled();
  await page.screenshot({ path: testInfo.outputPath('thai-worktree-occupied.png') });
  await page.getByRole('button', { name: 'กลับ', exact: true }).click();
  await page.getByLabel('เช็คเอาต์ในโฟลเดอร์ใหม่').selectOption('detached'); await page.getByRole('button', { name: 'ตรวจทาน worktree', exact: true }).click();
  await expect(page.getByRole('button', { name: 'สร้าง worktree', exact: true })).toBeEnabled();
  mkdirSync(target); writeFileSync(join(target, 'keep.txt'), 'external work\n');
  await page.getByRole('button', { name: 'สร้าง worktree', exact: true }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'ปลายทาง worktree, branch หรือคลังเปลี่ยนไป' }).first()).toBeVisible();
  expect(readFileSync(join(target, 'keep.txt'), 'utf8')).toBe('external work\n');
  expect(git(source, ['worktree', 'list', '--porcelain'])).not.toContain(target);
});

test('saved work appears at each base in the graph and opens stash actions without checkout', async ({ page }, testInfo) => {
  test.setTimeout(120000);
  const repo = join(folder, 'stash-graph-source'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); git(repo, ['config', 'core.hooksPath', '/dev/null']);
  writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Stash graph base']); const firstBase = git(repo, ['rev-parse', 'HEAD']);
  writeFileSync(join(repo, 'file.txt'), 'first saved version\n'); git(repo, ['stash', 'push', '-m', 'First graph saved work']);
  writeFileSync(join(repo, 'second.txt'), 'second base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Another graph base']); const head = git(repo, ['rev-parse', 'HEAD']);
  writeFileSync(join(repo, 'file.txt'), 'second saved version\n'); git(repo, ['stash', 'push', '-m', 'Second graph saved work']);
  await connectRealCore(page, join(folder, 'stash-graph-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  const saved = page.locator('.stash-graph-row'); await expect(saved).toHaveCount(2); await expect(saved.nth(0)).toHaveAttribute('data-base-oid', head); await expect(saved.nth(1)).toHaveAttribute('data-base-oid', firstBase);
  await expect(page.locator('.commit-row:not(.stash-graph-row)')).toHaveCount(2);
  await saved.nth(1).locator('.graph-mark').dblclick(); await expect(page.getByLabel('Stashes', { exact: true })).toBeVisible(); await expect(page.getByLabel('Commit actions', { exact: true })).toHaveCount(0);
  await expect(page.getByLabel('Stash review', { exact: true })).toContainText('first saved version');
  await page.screenshot({ path: testInfo.outputPath('saved-work-in-graph.png') });
  expect(git(repo, ['branch', '--show-current'])).toBe('main'); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(head);
  await page.getByText('Options and recovery', { exact: true }).click(); await page.getByLabel('Keep the stash after restoring', { exact: true }).check(); await page.getByRole('button', { name: 'Apply on main', exact: true }).click(); await expect(page.getByText('Stash applied; saved entry retained', { exact: true }).first()).toBeVisible();
  expect(readFileSync(join(repo, 'file.txt'), 'utf8')).toBe('first saved version\n'); expect(git(repo, ['stash', 'list']).split('\n')).toHaveLength(2);
  await page.keyboard.press('Control+k'); await page.getByLabel('Search commit text, author or SHA', { exact: true }).fill('Stash graph base'); await page.locator('.commit-search-result').filter({ hasText: 'Stash graph base' }).click(); await expect(page.locator(`button#commit-${firstBase}`)).toHaveAttribute('aria-pressed', 'true'); await expect(page.locator(`button#commit-${firstBase}`)).toBeVisible(); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(head);
});

test('large merge review finishes and blocked repeat start returns to the pending operation', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  const repo = join(folder, 'large-merge-ui'); mkdirSync(repo); git(repo, ['init', '-b', 'main']);
  git(repo, ['config', 'user.name', 'GitNova Test']); git(repo, ['config', 'user.email', 'test@example.invalid']); git(repo, ['config', 'commit.gpgsign', 'false']); git(repo, ['config', 'core.hooksPath', '/dev/null']);
  writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Large merge base']); git(repo, ['switch', '-c', 'feature/large']);
  writeFileSync(join(repo, 'large.bin'), Buffer.alloc(17 * 1024 * 1024)); writeFileSync(join(repo, 'large.txt'), 'Large staged code preview line with known fixture content\n'.repeat(16000)); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Large merge incoming']); const incoming = git(repo, ['rev-parse', 'HEAD']);
  git(repo, ['switch', 'main']); writeFileSync(join(repo, 'main.txt'), 'main\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Independent main commit']); const original = git(repo, ['rev-parse', 'HEAD']);
  await connectRealCore(page, join(folder, 'large-merge-ui-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  const row = page.locator(`button#commit-${incoming}`); await row.click({ button: 'right' }); await page.getByRole('menuitem', { name: 'Merge into main…', exact: true }).click(); await page.getByRole('button', { name: 'Start merge', exact: true }).click();
  await expect(page.getByLabel('Merge working changes', { exact: true })).toBeVisible(); await expect(page.locator('.status-line')).toContainText('Operation pending · 2 changed paths');
  await row.click({ button: 'right' }); await page.getByRole('menuitem', { name: 'Merge into main…', exact: true }).click(); await page.getByRole('button', { name: 'Review current operation', exact: true }).click(); await expect(page.getByLabel('Merge working changes', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Review merge commit', exact: true }).click(); await expect(page.getByText('The full staged tree is selected. Code preview is partial; select files to inspect their patches.', { exact: true })).toBeVisible();
  await page.locator('.merge-detail .changed-file').filter({ hasText: 'large.txt' }).click();
  await expect(page.getByLabel('Merge diff', { exact: true })).toContainText('Large staged code preview line with known fixture content');
  await page.screenshot({ path: testInfo.outputPath('large-merge-ready-to-finish.png') });
  await page.getByRole('button', { name: 'Finish merge', exact: true }).click();
  await expect(page.getByText('Working tree clean', { exact: true })).toBeVisible({ timeout: 45000 });
  expect(git(repo, ['show', '-s', '--format=%P', 'HEAD'])).toBe(`${original} ${incoming}`); expect(git(repo, ['status', '--porcelain'])).toBe('');
});

test('interrupted owned merge closes its session record after review while preserving new work', async ({ page }, testInfo) => {
  test.setTimeout(120000);
  const repo = join(folder, 'interrupted-merge-ui'); mkdirSync(repo); git(repo, ['init', '-b', 'main']);
  for (const [key, value] of [['user.name', 'GitNova Test'], ['user.email', 'test@example.invalid'], ['commit.gpgsign', 'false'], ['core.hooksPath', '/dev/null']]) git(repo, ['config', key, value]);
  writeFileSync(join(repo, 'base.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Interrupted base']); git(repo, ['switch', '-c', 'feature/session']); writeFileSync(join(repo, 'incoming.txt'), 'incoming\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Interrupted incoming']); const incoming = git(repo, ['rev-parse', 'HEAD']); git(repo, ['switch', 'main']); writeFileSync(join(repo, 'main.txt'), 'main\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Interrupted main']); const original = git(repo, ['rev-parse', 'HEAD']);
  await connectRealCore(page, join(folder, 'interrupted-merge-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click(); await page.locator(`button#commit-${incoming}`).click({ button: 'right' }); await page.getByRole('menuitem', { name: 'Merge into main…', exact: true }).click(); await page.getByRole('button', { name: 'Start merge', exact: true }).click(); await expect(page.getByLabel('Merge working changes', { exact: true })).toBeVisible();
  git(repo, ['merge', '--abort']);git(repo,['commit','--allow-empty','-m','Continued after external recovery']);const continued=git(repo,['rev-parse','HEAD']);const bare=join(folder,'interrupted-merge-upstream.git');git(folder,['init','--bare',bare]);git(repo,['remote','add','origin',bare]);git(repo,['push','-u','origin','main']);const tree=git(repo,['rev-parse','HEAD^{tree}']);const first=git(repo,['commit-tree',tree,'-p',continued,'-m','Remote work one']);const second=git(repo,['commit-tree',tree,'-p',first,'-m','Remote work two']);git(repo,['push','origin',`${second}:refs/heads/main`]); writeFileSync(join(repo, 'base.txt'), 'new staged\n'); git(repo, ['add', 'base.txt']); writeFileSync(join(repo, 'base.txt'), 'new working\n'); const index = readFileSync(join(repo, '.git/index'));
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();await page.locator(`button#commit-${continued}`).click();await expect(page.locator('.sync-pull')).toBeDisabled();await page.getByRole('button',{name:'Inspect current operation',exact:true}).click(); await expect(page.getByRole('button', { name: 'Review session cleanup…', exact: true })).toBeEnabled(); await page.getByRole('button', { name: 'Review session cleanup…', exact: true }).click(); await expect(page.getByText('Closes only the saved GitNova session record. Current files, staged content, branch and recovery refs stay in place; the session metadata is archived.', { exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('interrupted-session-cleanup-review.png') });
  await page.getByRole('button', { name: 'Close session & keep work', exact: true }).click(); await expect(page.getByText('Interrupted session closed; files, staging and recovery refs preserved', { exact: true })).toBeVisible();
  expect(continued).not.toBe(original);expect(git(repo, ['rev-parse', 'HEAD'])).toBe(continued);await expect(page.locator('.sync-pull')).toBeEnabled();await expect(page.getByRole('button',{name:'Inspect current operation',exact:true})).toHaveCount(0); expect(readFileSync(join(repo, '.git/index'))).toEqual(index); expect(readFileSync(join(repo, 'base.txt'), 'utf8')).toBe('new working\n'); expect(git(repo, ['show', ':base.txt'])).toBe('new staged');
});

test('direct main stash pops onto develop and reserved graph lanes avoid the neighboring main branch', async ({ page }, testInfo) => {
  test.setTimeout(240000);
  const datedCommit = (repo: string, subject: string, day: number) => execFileSync('git', ['-C', repo, '-c', 'user.name=GitNova Test', '-c', 'user.email=test@example.invalid', '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', 'commit', '-m', subject], { env: { ...process.env, GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: process.platform === 'win32' ? 'NUL' : '/dev/null', GIT_COMMITTER_DATE: `2026-01-0${day}T12:00:00Z`, GIT_AUTHOR_DATE: `2026-01-0${day}T12:00:00Z` } });
  const repo = join(folder, 'direct-cross-branch-stash'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); git(repo, ['config', 'core.hooksPath', '/dev/null']); writeFileSync(join(repo, 'file.txt'), 'base\n'); writeFileSync(join(repo, '.gitignore'), 'ignored.txt\n'); git(repo, ['add', '.']); datedCommit(repo, 'Shared base', 1);
  git(repo, ['switch', '-c', 'develop']); writeFileSync(join(repo, 'develop-only.txt'), 'develop tip\n'); git(repo, ['add', '.']); datedCommit(repo, 'Develop progress', 2); const develop = git(repo, ['rev-parse', 'HEAD']); git(repo, ['switch', 'main']); writeFileSync(join(repo, 'main-only.txt'), 'main tip\n'); git(repo, ['add', '.']); datedCommit(repo, 'Main progress', 3); const main = git(repo, ['rev-parse', 'HEAD']);
  writeFileSync(join(repo, 'file.txt'), 'saved on main\n'); writeFileSync(join(repo, 'new ภาษาไทย.txt'), 'new saved work\n'); writeFileSync(join(repo, 'ignored.txt'), 'keep ignored\n');
  await connectRealCore(page, join(folder, 'direct-cross-branch.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click();
  const actions = page.getByRole('group', { name: 'Save and restore work', exact: true }); const panel = page.locator('.stash-controls');
  await actions.getByRole('button', { name: 'Stash…', exact: true }).click(); await expect(panel.getByRole('button', { name: 'Stash on main', exact: true })).toBeEnabled(); await expect(panel.getByLabel('Stash files')).toContainText('new ภาษาไทย.txt'); await expect(panel.locator('.recovery-ref')).toBeHidden(); await panel.getByRole('button', { name: 'Stash on main', exact: true }).click(); await expect(panel).toHaveCount(0, { timeout: 45000 });
  const switcher = page.getByRole('button', { name: 'Switch branch', exact: true }); await switcher.click(); await page.getByRole('button', { name: 'Select branch develop', exact: true }).click(); await page.getByRole('button', { name: 'Switch to develop', exact: true }).click(); await expect(switcher).toContainText('develop');
  await actions.getByRole('button', { name: 'Pop…', exact: true }).click(); await expect(panel.getByRole('button', { name: 'Pop on develop', exact: true })).toBeEnabled(); await expect(panel.locator('.stash-destination')).toContainText('On main:'); await expect(panel.locator('.recovery-ref')).toBeHidden(); await page.screenshot({ path: testInfo.outputPath('pop-main-stash-onto-develop.png') }); await panel.getByRole('button', { name: 'Pop on develop', exact: true }).click(); await expect(panel).toHaveCount(0, { timeout: 45000 });
  expect(git(repo, ['branch', '--show-current'])).toBe('develop'); expect(git(repo, ['rev-parse', 'main'])).toBe(main); expect(git(repo, ['rev-parse', 'HEAD'])).toBe(develop); expect(readFileSync(join(repo, 'file.txt'), 'utf8')).toBe('saved on main\n'); expect(readFileSync(join(repo, 'new ภาษาไทย.txt'), 'utf8')).toBe('new saved work\n'); expect(readFileSync(join(repo, 'develop-only.txt'), 'utf8')).toBe('develop tip\n'); expect(readFileSync(join(repo, 'ignored.txt'), 'utf8')).toBe('keep ignored\n'); expect(git(repo, ['stash', 'list'])).toBe(''); expect(git(repo, ['diff', '--cached', '--name-only'])).toBe('');
  await actions.getByRole('button', { name: 'Stash…', exact: true }).click(); await panel.getByRole('button', { name: 'Stash on develop', exact: true }).click(); await expect(panel).toHaveCount(0, { timeout: 45000 });
  const marker = page.locator('.stash-mark'); await expect(marker).toHaveCount(1); expect(await marker.evaluate(svg => { const rect = svg.querySelector('rect')!; const center = Number(rect.getAttribute('x')) + Number(rect.getAttribute('width')) / 2; const lines = [...svg.querySelectorAll('path')].filter(path => path.getAttribute('stroke') !== '#e5b35c' && !path.getAttribute('stroke')?.includes('graph-stash')); return lines.length > 0 && lines.every(path => Math.abs(path.getPointAtLength(0).x - center) >= 20); })).toBe(true);
  const graphDivider=page.getByRole('separator',{name:'Resize Graph column',exact:true});await graphDivider.focus();await page.keyboard.press('ArrowLeft');await page.keyboard.press('ArrowLeft');
  expect(await marker.evaluate(svg=>{const rect=svg.querySelector('rect')!;const center=Number(rect.getAttribute('x'))+5;return [...svg.querySelectorAll('path')].filter(path=>!path.getAttribute('stroke')?.includes('graph-stash')).every(path=>Math.abs(path.getPointAtLength(0).x-center)>=12);})).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('stash-reserved-lane-beside-main.png') }); await actions.getByRole('button', { name: 'Pop…', exact: true }).click(); await panel.getByRole('button', { name: 'Pop on develop', exact: true }).click(); await expect(panel).toHaveCount(0, { timeout: 45000 }); expect(git(repo, ['stash', 'list'])).toBe('');
});

test('packed stash metadata offers explicit apply-and-retain when pop removal is unsupported', async ({ page }) => {
  test.setTimeout(90000); const repo = join(folder, 'packed-stash-ui'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); git(repo, ['config', 'core.hooksPath', '/dev/null']); writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Packed stash base']); writeFileSync(join(repo, 'file.txt'), 'saved packed work\n'); git(repo, ['stash', 'push', '-m', 'Packed saved work']); const saved = git(repo, ['rev-parse', 'refs/stash']); git(repo, ['pack-refs', '--all', '--prune']);
  await connectRealCore(page, join(folder, 'packed-stash-ui.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click(); await page.locator('.working-stash-actions').getByRole('button', { name: 'Pop…', exact: true }).click(); const panel = page.locator('.stash-controls'); await panel.getByRole('button', { name: 'Restore and keep stash', exact: true }).click(); await panel.getByRole('button', { name: 'Apply on main', exact: true }).click(); await expect(panel).toHaveCount(0, { timeout: 45000 }); expect(readFileSync(join(repo, 'file.txt'), 'utf8')).toBe('saved packed work\n'); expect(git(repo, ['rev-parse', 'refs/stash'])).toBe(saved);
});


test('reviewed worktree removal preserves source drafts and cleans registered workspace entries', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  const repo = join(folder, 'remove-worktree-source'); const linked = join(folder, 'remove-worktree-target'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); git(repo, ['config', 'core.hooksPath', '/dev/null']); writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Worktree removal base']); const head = git(repo, ['rev-parse', 'HEAD']); git(repo, ['worktree', 'add', '-b', 'topic', linked]); writeFileSync(join(repo, 'file.txt'), 'source work stays\n');
  await connectRealCore(page, join(folder, 'worktree-remove-state.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(linked); await page.getByRole('button', { name: 'Open', exact: true }).click(); await expect(page.locator('.commit-row')).toHaveCount(1);
  await page.keyboard.press('Control+o'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click(); await page.locator('.working-row').click(); await page.getByLabel('Summary', { exact: true }).fill('Keep draft while removing another checkout');
  await page.keyboard.press('Control+o'); const entry = page.locator('.worktree-entry').filter({ has: page.locator('small', { hasText: linked }) }); await entry.getByRole('button', { name: 'Remove worktree…', exact: true }).click(); const review = entry.getByRole('region', { name: 'Review worktree removal', exact: true }); await expect(review.getByRole('button', { name: 'Remove checkout', exact: true })).toBeEnabled(); await expect(review).toContainText('My workspace / remove-worktree-target'); await review.getByText('Recovery details', { exact: true }).click(); const recovery = await review.locator('.worktree-recovery').innerText(); expect(existsSync(linked)).toBe(true); await page.screenshot({ path: testInfo.outputPath('worktree-removal-review.png') });
  await review.getByRole('button', { name: 'Cancel', exact: true }).click(); expect(existsSync(linked)).toBe(true); await entry.getByRole('button', { name: 'Remove worktree…', exact: true }).click(); const confirmed = entry.getByRole('region', { name: 'Review worktree removal', exact: true }); await expect(confirmed.getByRole('button', { name: 'Remove checkout', exact: true })).toBeEnabled(); await confirmed.getByText('Recovery details', { exact: true }).click(); const confirmedRecovery = await confirmed.locator('.worktree-recovery').innerText(); expect(confirmedRecovery).not.toBe(recovery);
  await confirmed.getByRole('button', { name: 'Remove checkout', exact: true }).click(); await expect(page.getByText('Linked worktree removed; branch and commit retained for recovery', { exact: true })).toBeVisible({ timeout: 45000 }); await expect(entry).toHaveCount(0); expect(existsSync(linked)).toBe(false); expect(git(repo, ['rev-parse', 'topic'])).toBe(head); expect(git(repo, ['rev-parse', confirmedRecovery])).toBe(head); expect(readFileSync(join(repo, 'file.txt'), 'utf8')).toBe('source work stays\n');
  await expect(page.locator('.repo-option')).toHaveCount(1); await page.keyboard.press('Escape'); await expect(page.getByLabel('Summary', { exact: true })).toHaveValue('Keep draft while removing another checkout'); await expect(page.getByRole('button', { name: 'Switch branch', exact: true })).toContainText('main');
});

test('worktree removal explains ignored files and stale reviews without deleting local work', async ({ page }, testInfo) => {
  test.setTimeout(180000); const repo = join(folder, 'blocked-worktree-source'); const linked = join(folder, 'blocked-worktree-target'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); git(repo, ['config', 'core.hooksPath', '/dev/null']); writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Safe removal base']); git(repo, ['worktree', 'add', '-b', 'topic', linked]); writeFileSync(join(repo, '.git/info/exclude'), 'ignored.txt\n'); writeFileSync(join(linked, 'ignored.txt'), 'keep ignored work\n');
  await connectRealCore(page, join(folder, 'worktree-remove-blocked.json')); await page.goto('/'); await page.getByLabel('Add an existing repository').fill(repo); await page.getByRole('button', { name: 'Open', exact: true }).click(); await expect(page.locator('.commit-row')).toHaveCount(1); await page.keyboard.press('Control+o'); const entry = page.locator('.worktree-entry').filter({ has: page.locator('small', { hasText: linked }) }); await entry.getByRole('button', { name: 'Remove worktree…', exact: true }).click(); const review = entry.getByRole('region', { name: 'Review worktree removal', exact: true }); await expect(review).toContainText('Ignored files are still local work.'); await expect(review.getByRole('button', { name: 'Remove checkout', exact: true })).toBeDisabled(); await page.screenshot({ path: testInfo.outputPath('worktree-removal-ignored-files.png') });
  await review.getByRole('button', { name: 'Cancel', exact: true }).click(); rmSync(join(linked, 'ignored.txt')); await entry.getByRole('button', { name: 'Remove worktree…', exact: true }).click(); await expect(review.getByRole('button', { name: 'Remove checkout', exact: true })).toBeEnabled(); writeFileSync(join(linked, 'file.txt'), 'edited after review\n'); await review.getByRole('button', { name: 'Remove checkout', exact: true }).click(); await expect(entry.getByRole('alert')).toContainText('Review removal again.'); expect(readFileSync(join(linked, 'file.txt'), 'utf8')).toBe('edited after review\n'); expect(existsSync(linked)).toBe(true);
});

test('Thai removal review retains detached unpublished commits and original paths', async ({ page }, testInfo) => {
  test.setTimeout(180000); const repo = join(folder, 'thai-worktree-source'); const linked = join(folder, 'งาน แยก'); mkdirSync(repo); git(repo, ['init', '-b', 'main']); git(repo, ['config', 'core.hooksPath', '/dev/null']); writeFileSync(join(repo, 'file.txt'), 'base\n'); git(repo, ['add', '.']); git(repo, ['commit', '-m', 'Thai worktree base']); git(repo, ['worktree', 'add', '--detach', linked]); writeFileSync(join(linked, 'file.txt'), 'detached unpublished\n'); git(linked, ['add', '.']); git(linked, ['commit', '-m', 'Detached unpublished งาน']); const head = git(linked, ['rev-parse', 'HEAD']);
  await connectRealCore(page, join(folder, 'thai-worktree-remove.json')); await page.addInitScript(() => localStorage.setItem('gitnova.locale', 'th')); await page.goto('/'); await page.getByLabel('เพิ่ม repository ที่มีอยู่').fill(repo); await page.getByRole('button', { name: 'เปิด', exact: true }).click(); await expect(page.locator('.commit-row')).toHaveCount(1); await page.keyboard.press('Control+o'); const entry = page.locator('.worktree-entry').filter({ has: page.locator('small', { hasText: linked }) }); await entry.getByRole('button', { name: 'ลบ worktree…', exact: true }).click(); const review = entry.getByRole('region', { name: 'ตรวจสอบการลบ worktree', exact: true }); await expect(review.getByRole('button', { name: 'ลบ checkout', exact: true })).toBeEnabled(); await expect(review).toContainText(linked); await review.getByText('รายละเอียดการกู้คืน', { exact: true }).click(); const recovery = await review.locator('.worktree-recovery').innerText(); await page.screenshot({ path: testInfo.outputPath('worktree-removal-thai-detached.png') }); await review.getByRole('button', { name: 'ลบ checkout', exact: true }).click(); await expect(page.getByText('ลบ linked worktree แล้ว โดยเก็บ branch และ commit ไว้สำหรับกู้คืน', { exact: true })).toBeVisible({ timeout: 45000 }); expect(existsSync(linked)).toBe(false); expect(git(repo, ['rev-parse', recovery])).toBe(head); expect(git(repo, ['show', `${recovery}:file.txt`])).toBe('detached unpublished');
});

test('discard all previews explicit files and retains recovery and composer draft', async ({ page }, testInfo) => {
  test.setTimeout(180000); const repo=join(folder,'discard-all-ui');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['config','core.hooksPath','/dev/null']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Discard UI base']);const head=git(repo,['rev-parse','HEAD']);writeFileSync(join(repo,'file.txt'),'staged\n');git(repo,['add','.']);writeFileSync(join(repo,'file.txt'),'working\n');writeFileSync(join(repo,'new.txt'),'untracked valuable\n');writeFileSync(join(repo,'keep.txt'),'keep unselected\n');
  await connectRealCore(page,join(folder,'discard-all-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();await page.getByLabel('Summary',{exact:true}).fill('Keep this exact draft');await page.getByRole('button',{name:'Discard changes…',exact:true}).click();const review=page.getByRole('region',{name:'Review discard',exact:true});await expect(review.getByRole('button',{name:'Discard 3 files',exact:true})).toBeEnabled();
  await page.evaluate(() => { const bridge=(window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args:unknown)=>Promise<unknown>}}).__TAURI_INTERNALS__; const invoke=bridge.invoke.bind(bridge); bridge.invoke=async (command,args) => { const result=await invoke(command,args); if(command==='preview_discard') await new Promise(resolve=>setTimeout(resolve,1200)); return result; }; });
  await review.getByLabel('Discard keep.txt',{exact:true}).uncheck();await expect(review.getByRole('button',{name:'Discard 2 files',exact:true})).toBeDisabled();await expect(review.getByRole('button',{name:'Discard 2 files',exact:true})).toBeEnabled();await page.screenshot({path:testInfo.outputPath('discard-selected-review.png')});
  await review.getByRole('button',{name:'Cancel',exact:true}).click();expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('working\n');expect(existsSync(join(repo,'new.txt'))).toBe(true);await expect(page.getByLabel('Summary',{exact:true})).toHaveValue('Keep this exact draft');
  await page.getByRole('button',{name:'Discard changes…',exact:true}).click();await review.getByLabel('Discard keep.txt',{exact:true}).uncheck();await expect(review.getByRole('button',{name:'Discard 2 files',exact:true})).toBeEnabled();await review.getByRole('button',{name:'Discard 2 files',exact:true}).click();await expect(page.getByText('Selected changes discarded; original content saved for recovery',{exact:true})).toBeVisible({timeout:45000});await expect(review).toHaveCount(0);await expect(page.getByLabel('Summary',{exact:true})).toHaveValue('Keep this exact draft');expect(git(repo,['rev-parse','HEAD'])).toBe(head);expect(git(repo,['show',':file.txt'])).toBe('base');expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('base\n');expect(readFileSync(join(repo,'keep.txt'),'utf8')).toBe('keep unselected\n');expect(existsSync(join(repo,'new.txt'))).toBe(false);const recovery=await page.getByRole('button',{name:'Copy recovery location',exact:true}).getAttribute('title');expect(recovery).toBeTruthy();expect(existsSync(join(recovery!,'manifest.json'))).toBe(true);expect(readFileSync(join(recovery!,'work-0'),'utf8')).toBe('working\n');
  await page.reload();await page.locator('.working-row').click();await page.locator('.recorded-activity > summary').click();const activity=page.getByRole('region',{name:'Recorded GitNova operations',exact:true});await expect(activity.getByText('Discard',{exact:true})).toBeVisible();await activity.getByText('Recorded details',{exact:true}).click();await expect(activity.getByText(recovery!,{exact:true})).toBeVisible();await expect(activity.getByRole('button',{name:'Copy recorded location',exact:true})).toBeEnabled();await page.screenshot({path:testInfo.outputPath('persisted-discard-recovery.png')});
  await page.keyboard.press('Control+,');await page.getByLabel('Language',{exact:true}).selectOption('th');await page.keyboard.press('Escape');const thaiActivity=page.getByRole('region',{name:'การทำงานที่ GitNova บันทึกไว้',exact:true});await expect(thaiActivity).toContainText(recovery!);await page.screenshot({path:testInfo.outputPath('persisted-discard-recovery-thai.png')});
});

test('file context discard keeps staged content and rejects a stale preview', async ({ page }, testInfo) => {
  test.setTimeout(180000);const repo=join(folder,'discard-file-ui');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['config','core.hooksPath','/dev/null']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','File discard base']);writeFileSync(join(repo,'file.txt'),'staged\n');git(repo,['add','.']);writeFileSync(join(repo,'file.txt'),'working\n');
  await connectRealCore(page,join(folder,'discard-file-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();await page.getByRole('region',{name:'Unstaged files',exact:true}).locator('.changed-file').click({button:'right'});await page.getByRole('menuitem',{name:'Discard file…',exact:true}).click();const review=page.getByRole('region',{name:'Review discard',exact:true});await expect(review.getByLabel('What to discard')).toHaveValue('unstaged');await expect(review.getByRole('button',{name:'Discard 1 file',exact:true})).toBeEnabled();writeFileSync(join(repo,'file.txt'),'late edit\n');await review.getByRole('button',{name:'Discard 1 file',exact:true}).click();await expect(review.getByRole('alert')).toContainText('Discard selection changed.');expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('late edit\n');
  await review.getByRole('button',{name:'Cancel',exact:true}).click();await page.getByRole('region',{name:'Unstaged files',exact:true}).locator('.changed-file').click({button:'right'});await page.getByRole('menuitem',{name:'Discard file…',exact:true}).click();await expect(review.getByRole('button',{name:'Discard 1 file',exact:true})).toBeEnabled();await page.screenshot({path:testInfo.outputPath('discard-unstaged-file.png')});await review.getByRole('button',{name:'Discard 1 file',exact:true}).click();await expect(review).toHaveCount(0,{timeout:45000});expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('staged\n');expect(git(repo,['show',':file.txt'])).toBe('staged');
});

test('Thai discard uses the same reviewed scope and preserves original names', async ({ page }, testInfo) => {
  test.setTimeout(180000);const repo=join(folder,'discard-thai-ui');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['config','core.hooksPath','/dev/null']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Thai discard base']);writeFileSync(join(repo,'งาน.txt'),'งานที่ยังไม่บันทึก\n');
  await connectRealCore(page,join(folder,'discard-thai-ui.json'));await page.addInitScript(()=>localStorage.setItem('gitnova.locale','th'));await page.goto('/');await page.getByLabel('เพิ่ม repository ที่มีอยู่').fill(repo);await page.getByRole('button',{name:'เปิด',exact:true}).click();await page.locator('.working-row').click();await page.getByRole('button',{name:'ทิ้งการเปลี่ยนแปลง…',exact:true}).click();const review=page.getByRole('region',{name:'ตรวจสอบการทิ้งการเปลี่ยนแปลง',exact:true});await expect(review.getByRole('button',{name:'ทิ้งการเปลี่ยนแปลง 1 ไฟล์',exact:true})).toBeEnabled();await expect(review).toContainText('งาน.txt');await page.screenshot({path:testInfo.outputPath('discard-thai-review.png')});await review.getByRole('button',{name:'ทิ้งการเปลี่ยนแปลง 1 ไฟล์',exact:true}).click();await expect(review).toHaveCount(0,{timeout:45000});expect(existsSync(join(repo,'งาน.txt'))).toBe(false);expect(git(repo,['status','--porcelain'])).toBe('');
});

test('Working changes shows a submodule pointer diff and directly restores its recorded commit', async ({ page }, testInfo) => {
  test.setTimeout(180000);const repo=join(folder,'discard-submodule-parent');const child=join(folder,'discard-submodule-source');for(const path of [repo,child]){mkdirSync(path);git(path,['init','-b','main']);git(path,['config','core.hooksPath','/dev/null']);writeFileSync(join(path,'file.txt'),'base\n');git(path,['add','.']);git(path,['commit','-m','Submodule working base']);}
  git(repo,['-c','protocol.file.allow=always','submodule','add',child,'_dags']);git(repo,['commit','-am','Record _dags']);const checkout=join(repo,'_dags');const recorded=git(checkout,['rev-parse','HEAD']);writeFileSync(join(checkout,'file.txt'),'child next\n');git(checkout,['commit','-am','Child next']);const next=git(checkout,['rev-parse','HEAD']);const parent=git(repo,['rev-parse','HEAD']);const index=readFileSync(join(repo,'.git/index'));
  await connectRealCore(page,join(folder,'discard-submodule-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();await expect(page.getByRole('region',{name:'Before code',exact:true})).toContainText(recorded);await expect(page.getByRole('region',{name:'After code',exact:true})).toContainText(next);await expect(page.getByRole('alert')).toHaveCount(0);const related=page.locator('.working-submodule');await related.getByRole('button',{name:'Restore recorded…',exact:true}).click();await expect(related.getByRole('button',{name:'Restore recorded version',exact:true})).toBeEnabled();await page.screenshot({path:testInfo.outputPath('working-submodule-recorded-restore.png')});await related.getByRole('button',{name:'Restore recorded version',exact:true}).click();await expect(page.getByText('Working tree clean',{exact:true})).toBeVisible({timeout:45000});expect(git(checkout,['rev-parse','HEAD'])).toBe(recorded);expect(git(checkout,['rev-parse','main'])).toBe(next);expect(git(repo,['rev-parse','HEAD'])).toBe(parent);expect(readFileSync(join(repo,'.git/index'))).toEqual(index);
});


test('pending merge unstages one or all files, reviews working diffs and finishes only restaged content', async ({page},testInfo)=>{
  test.setTimeout(240000);const repo=join(folder,'merge-unstage-ui');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['config','core.hooksPath','/dev/null']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Unstage base']);git(repo,['switch','-c','incoming']);writeFileSync(join(repo,'file.txt'),'incoming version\n');writeFileSync(join(repo,'new.txt'),'new incoming content\n');git(repo,['add','.']);git(repo,['commit','-m','Incoming unstage work']);const incoming=git(repo,['rev-parse','HEAD']);git(repo,['switch','main']);writeFileSync(join(repo,'destination.txt'),'destination\n');git(repo,['add','.']);git(repo,['commit','-m','Destination work']);const head=git(repo,['rev-parse','HEAD']);
  await connectRealCore(page,join(folder,'merge-unstage-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.getByRole('button',{name:/Incoming unstage work/}).click({button:'right'});await page.getByRole('menuitem',{name:'Merge into main…',exact:true}).click();await page.getByRole('button',{name:'Start merge',exact:true}).click();
  const inspector=page.locator('.merge-detail');const staged=inspector.getByRole('region',{name:'Staged files',exact:true});const unstaged=inspector.getByRole('region',{name:'Unstaged files',exact:true});await expect(staged.locator('.changed-file')).toHaveCount(2,{timeout:45000});
  await inspector.getByRole('button',{name:'Unstage file.txt',exact:true}).click();await expect(inspector.getByRole('button',{name:'Unstage file',exact:true})).toBeEnabled();expect(git(repo,['diff','--cached','--name-only'])).toContain('file.txt');await inspector.getByRole('button',{name:'Unstage file',exact:true}).click();await expect(unstaged.locator('.changed-file')).toHaveCount(1,{timeout:45000});await expect(staged.locator('.changed-file')).toHaveCount(1);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('incoming version\n');
  await unstaged.locator('.changed-file').filter({hasText:'file.txt'}).click();await expect(page.getByRole('region',{name:'After code',exact:true})).toContainText('incoming version');await inspector.getByRole('button',{name:'Review & stage working result',exact:true}).click();await inspector.getByRole('button',{name:'Apply resolution',exact:true}).click();await expect(staged.locator('.changed-file')).toHaveCount(2,{timeout:45000});
  await inspector.getByRole('button',{name:'Unstage all changes',exact:true}).click();await expect(inspector.getByRole('button',{name:'Unstage 2 files',exact:true})).toBeEnabled();await expect(page.getByRole('region',{name:'After code',exact:true})).toContainText('incoming version');await page.screenshot({path:testInfo.outputPath('merge-unstage-all-review.png')});await inspector.getByRole('button',{name:'Unstage 2 files',exact:true}).click();await expect(staged.locator('.changed-file')).toHaveCount(0,{timeout:45000});await expect(unstaged.locator('.changed-file')).toHaveCount(2);expect(git(repo,['rev-parse','HEAD'])).toBe(head);expect(readFileSync(join(repo,'.git/MERGE_HEAD'),'utf8').trim()).toBe(incoming);expect(readFileSync(join(repo,'new.txt'),'utf8')).toBe('new incoming content\n');
  await unstaged.locator('.changed-file').filter({hasText:'new.txt'}).click();await expect(page.getByRole('region',{name:'After code',exact:true})).toContainText('new incoming content');await page.keyboard.press('Control+,');await page.getByLabel('Language',{exact:true}).selectOption('th');await page.keyboard.press('Escape');await expect(inspector.getByRole('button',{name:'นำการเปลี่ยนแปลงทั้งหมดออกจาก staged',exact:true})).toBeDisabled();await page.screenshot({path:testInfo.outputPath('merge-unstaged-thai.png')});await page.keyboard.press('Control+,');await page.getByLabel('ภาษา',{exact:true}).selectOption('en');await page.keyboard.press('Escape');
  await unstaged.locator('.changed-file').filter({hasText:'file.txt'}).click();await inspector.getByRole('button',{name:'Review & stage working result',exact:true}).click();await inspector.getByRole('button',{name:'Apply resolution',exact:true}).click();await expect(staged.locator('.changed-file')).toHaveCount(1,{timeout:45000});await inspector.getByLabel('Merge message',{exact:true}).fill('Merge only reviewed tracked file');await inspector.getByRole('button',{name:'Review merge commit',exact:true}).click();await inspector.getByRole('button',{name:'Finish merge',exact:true}).click();await expect(page.getByRole('heading',{name:'Merge only reviewed tracked file',exact:true})).toBeVisible({timeout:45000});expect(git(repo,['show','-s','--format=%P','HEAD'])).toBe(`${head} ${incoming}`);expect(git(repo,['ls-tree','--name-only','HEAD'])).not.toContain('new.txt');expect(readFileSync(join(repo,'new.txt'),'utf8')).toBe('new incoming content\n');
});


test('long merge paths keep unstage controls visible and clickable in English and Thai',async({page},testInfo)=>{
  test.setTimeout(120000);const repo=join(folder,'merge-long-path-controls');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['config','core.hooksPath','/dev/null']);writeFileSync(join(repo,'base.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Long paths base']);git(repo,['switch','-c','incoming']);const directory='artifacts/customer-guide/generated-video-output';mkdirSync(join(repo,directory),{recursive:true});for(const name of ['finetune-hyperframes-rock-customer-guide-narration-neural-male.srt','finetune-hyperframes-customer-guide-full-delivery-notes-english.txt'])writeFileSync(join(repo,directory,name),'incoming content\n');git(repo,['add','.']);git(repo,['commit','-m','Incoming long path work']);git(repo,['switch','main']);writeFileSync(join(repo,'destination.txt'),'destination\n');git(repo,['add','.']);git(repo,['commit','-m','Destination']);
  await connectRealCore(page,join(folder,'merge-long-path-controls.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.getByRole('button',{name:/Incoming long path work/}).click({button:'right'});await page.getByRole('menuitem',{name:'Merge into main…',exact:true}).click();await page.getByRole('button',{name:'Start merge',exact:true}).click();const inspector=page.locator('.merge-detail');await expect(inspector.locator('.file-stage-action')).toHaveCount(2,{timeout:45000});
  for(const locale of ['en','th']){
    if(locale==='th'){await page.keyboard.press('Control+,');await page.getByLabel('Language',{exact:true}).selectOption('th');await page.keyboard.press('Escape');}
    for(const width of [1360,1000]){
      await page.setViewportSize({width,height:900});const all=inspector.getByRole('button',{name:locale==='en'?'Unstage all changes':'นำการเปลี่ยนแปลงทั้งหมดออกจาก staged',exact:true});await expect(all).toBeEnabled();
      for(const button of [all,inspector.locator('.file-stage-action').first(),inspector.locator('.file-stage-action').last()]){
        const pane=await inspector.boundingBox();const box=await button.boundingBox();expect(box).not.toBeNull();expect(box!.x).toBeGreaterThanOrEqual(pane!.x);expect(box!.x+box!.width).toBeLessThanOrEqual(pane!.x+pane!.width);
        expect(await button.evaluate(node=>{const r=node.getBoundingClientRect();return document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)?.closest('button')===node;})).toBe(true);
      }
      expect(await inspector.evaluate(node=>node.scrollWidth<=node.clientWidth+1)).toBe(true);
      await page.screenshot({path:testInfo.outputPath(`merge-long-path-controls-${locale}-${width}.png`)});
    }
  }
});

test('pending merge discard all reviews and cancels, then removes unstaged files while keeping the merge',async({page},testInfo)=>{
  test.setTimeout(240000);
  const repo=join(folder,'merge-discard-ui');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['config','core.hooksPath','/dev/null']);
  writeFileSync(join(repo,'base.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Discard merge base']);git(repo,['switch','-c','incoming']);
  const directory='artifacts/customer-guide/long-generated-output';mkdirSync(join(repo,directory),{recursive:true});
  const files=['finetune-customer-guide-video-narration-delivery-notes.txt','finetune-customer-guide-neural-narration-subtitles-th.srt'];
  for(const name of files)writeFileSync(join(repo,directory,name),'incoming saved content\n');git(repo,['add','.']);git(repo,['commit','-m','Incoming discard work']);
  const incoming=git(repo,['rev-parse','HEAD']);writeFileSync(join(repo,'saved-note.txt'),'Saved across branches\n');git(repo,['stash','push','--include-untracked','-m','Saved before merge']);const saved=git(repo,['rev-parse','refs/stash']);git(repo,['switch','main']);writeFileSync(join(repo,'destination.txt'),'destination\n');git(repo,['add','.']);git(repo,['commit','-m','Destination']);const head=git(repo,['rev-parse','HEAD']);
  await connectRealCore(page,join(folder,'merge-discard-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  await page.getByRole('button',{name:/Incoming discard work/}).click({button:'right'});await page.getByRole('menuitem',{name:'Merge into main…',exact:true}).click();await page.getByRole('button',{name:'Start merge',exact:true}).click();
  const inspector=page.locator('.merge-detail');await expect(inspector.getByRole('button',{name:'Unstage all changes',exact:true})).toBeEnabled({timeout:45000});await inspector.getByRole('button',{name:'Unstage all changes',exact:true}).click();await inspector.getByRole('button',{name:'Unstage 2 files',exact:true}).click();
  await expect(inspector.getByRole('region',{name:'Unstaged files',exact:true}).locator('.changed-file')).toHaveCount(2,{timeout:45000});
  const trash=inspector.getByRole('button',{name:'Discard all changes…',exact:true});await expect(trash).toBeEnabled();await trash.click();const review=inspector.locator('.discard-review');await expect(review.getByRole('button',{name:'Discard 2 files',exact:true})).toBeEnabled();await expect(review).toContainText('Discard keeps this operation pending.');
  await review.getByRole('button',{name:'Cancel',exact:true}).click();for(const name of files)expect(existsSync(join(repo,directory,name))).toBe(true);
  await trash.click();await expect(review.getByRole('button',{name:'Discard 2 files',exact:true})).toBeEnabled();await page.screenshot({path:testInfo.outputPath('merge-discard-review.png')});
  await page.keyboard.press('Control+,');await page.getByLabel('Language',{exact:true}).selectOption('th');await page.keyboard.press('Escape');await expect(review).toContainText('Discard จะคงการดำเนินการนี้ไว้');await page.screenshot({path:testInfo.outputPath('merge-discard-review-thai.png')});
  await review.getByRole('button',{name:'ทิ้งการเปลี่ยนแปลง 2 ไฟล์',exact:true}).click();await expect(review.getByRole('progressbar')).toBeVisible();await expect(review.getByRole('progressbar')).toHaveAttribute('max','2');await expect(review.locator('.discard-progress small')).toContainText('%');const progressBox=await review.getByRole('progressbar').boundingBox();const paneBox=await inspector.boundingBox();expect(progressBox!.y+progressBox!.height).toBeLessThanOrEqual(paneBox!.y+paneBox!.height);await page.screenshot({path:testInfo.outputPath('merge-discard-progress.png')});await expect(review).toHaveCount(0,{timeout:45000});await expect(inspector).toBeVisible();
  for(const name of files)expect(existsSync(join(repo,directory,name))).toBe(false);expect(git(repo,['rev-parse','HEAD'])).toBe(head);expect(readFileSync(join(repo,'.git/MERGE_HEAD'),'utf8').trim()).toBe(incoming);
  const recovery=await page.locator('.copy-recovery').getAttribute('title');expect(recovery).toBeTruthy();expect(readFileSync(join(recovery!,'work-0'),'utf8')).toBe('incoming saved content\n');
  const events=await page.evaluate(()=>(window as unknown as {gitnovaProgressEvents: {phase:string;completed:number;total:number}[]}).gitnovaProgressEvents);for(const phase of ['checking','backing-up','discarding'])expect(events.some(e=>e.phase===phase)).toBe(true);expect(events.every(e=>e.completed<=e.total)).toBe(true);
  await changeLanguageThroughSettings(page,'en');
  await expect(inspector).toContainText('No file changes remain, but Merge is still active.');
  await expect(page.locator('.working-stash-actions').getByRole('button',{name:'Pop…',exact:true})).toBeDisabled();
  await page.locator('.stash-graph-row').click();const stash=page.locator('.stash-controls');
  await expect(stash).toContainText('even with zero changed files');await expect(stash.getByRole('button',{name:'Pop on main',exact:true})).toBeDisabled();
  expect(git(repo,['rev-parse','refs/stash'])).toBe(saved);expect(existsSync(join(repo,'saved-note.txt'))).toBe(false);
  await stash.getByRole('button',{name:'Review pending Merge',exact:true}).click();
  await inspector.getByRole('button',{name:'Review abort…',exact:true}).click();await expect(inspector.getByRole('button',{name:'Abort merge & save edits',exact:true})).toBeEnabled();
  await inspector.getByRole('button',{name:'Back',exact:true}).click();expect(existsSync(join(repo,'.git/MERGE_HEAD'))).toBe(true);expect(git(repo,['rev-parse','refs/stash'])).toBe(saved);
  await inspector.getByRole('button',{name:'Review abort…',exact:true}).click();await inspector.getByRole('button',{name:'Abort merge & save edits',exact:true}).click();
  await expect(inspector).toHaveCount(0,{timeout:45000});expect(existsSync(join(repo,'.git/MERGE_HEAD'))).toBe(false);
  const pop=page.locator('.working-stash-actions').getByRole('button',{name:'Pop…',exact:true});await expect(pop).toBeEnabled();await pop.click();
  await expect(stash.getByRole('button',{name:'Pop on main',exact:true})).toBeEnabled();await stash.getByRole('button',{name:'Pop on main',exact:true}).click();await expect(stash).toHaveCount(0,{timeout:45000});
  expect(readFileSync(join(repo,'saved-note.txt'),'utf8')).toBe('Saved across branches\n');expect(git(repo,['stash','list'])).toBe('');expect(git(repo,['rev-parse','HEAD'])).toBe(head);expect(git(repo,['rev-parse','incoming'])).toBe(incoming);

});

test('open Pop review follows an external branch change without replaying a write', async ({ page }) => {
  test.setTimeout(120000);
  const repo=join(folder,'pop-live-context');mkdirSync(repo);git(repo,['init','-b','main']);
  writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Pop context base']);
  git(repo,['switch','-c','develop']);writeFileSync(join(repo,'develop.txt'),'develop only\n');git(repo,['add','.']);git(repo,['commit','-m','Develop tip']);const develop=git(repo,['rev-parse','HEAD']);
  git(repo,['switch','main']);const main=git(repo,['rev-parse','HEAD']);writeFileSync(join(repo,'file.txt'),'saved on main\n');git(repo,['stash','push','-m','Live Pop review']);const saved=git(repo,['rev-parse','refs/stash']);
  await connectRealCore(page,join(folder,'pop-live-context.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  await page.locator('.working-stash-actions').getByRole('button',{name:'Pop…',exact:true}).click();const panel=page.locator('.stash-controls');await expect(panel.getByRole('button',{name:'Pop on main',exact:true})).toBeEnabled();
  await panel.getByText('Options and recovery',{exact:true}).click();await panel.getByRole('button',{name:'Saved index',exact:true}).click();
  git(repo,['switch','develop']);
  await expect(panel.getByRole('button',{name:'Pop on develop',exact:true})).toBeEnabled({timeout:30000});await expect(panel.getByRole('button',{name:'Saved index',exact:true})).toHaveAttribute('aria-pressed','true');
  expect(git(repo,['rev-parse','refs/stash'])).toBe(saved);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('base\n');
  await panel.getByRole('button',{name:'Pop on develop',exact:true}).click();await expect(panel).toHaveCount(0,{timeout:45000});
  expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('saved on main\n');expect(git(repo,['stash','list'])).toBe('');expect(git(repo,['rev-parse','HEAD'])).toBe(develop);expect(git(repo,['rev-parse','main'])).toBe(main);
});

test('merge method distinguishes a real two-parent merge from explicit fast-forward on linear history', async ({page},testInfo) => {
  test.setTimeout(180000);
  const repo=join(folder,'merge-method-linear');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['config','user.name','Merge Method Test']);git(repo,['config','user.email','method@example.invalid']);git(repo,['config','commit.gpgsign','false']);git(repo,['config','core.hooksPath','/dev/null']);
  writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Method base']);const original=git(repo,['rev-parse','HEAD']);
  git(repo,['switch','-c','feature/linear']);writeFileSync(join(repo,'file.txt'),'one\n');git(repo,['commit','-am','Feature one']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['commit','-am','Feature two']);const source=git(repo,['rev-parse','HEAD']);git(repo,['switch','main']);
  await connectRealCore(page,join(folder,'merge-method-linear.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  await page.locator(`button#commit-${source}`).click({button:'right'});await page.getByRole('menuitem',{name:'Merge into main…',exact:true}).click();
  const method=page.getByLabel('Merge method',{exact:true});await expect(method).toHaveValue('mergeCommit');await expect(page.getByRole('button',{name:'Start merge',exact:true})).toBeEnabled();
  await method.selectOption('fastForwardOnly');await expect(page.getByRole('button',{name:'Fast-forward branch',exact:true})).toBeEnabled();expect(git(repo,['rev-parse','HEAD'])).toBe(original);
  await method.selectOption('mergeCommit');await expect(page.getByRole('button',{name:'Start merge',exact:true})).toBeEnabled();await page.getByRole('button',{name:'Start merge',exact:true}).click();
  const inspector=page.getByLabel('Merge working changes',{exact:true});await expect(inspector).toBeVisible({timeout:45000});expect(git(repo,['rev-parse','HEAD'])).toBe(original);await expect(inspector).toContainText('Finishing still records both parents');await expect(inspector.getByRole('button',{name:'Review merge commit',exact:true})).toHaveClass(/primary/);
  await inspector.getByRole('button',{name:'Review merge commit',exact:true}).click();await inspector.getByRole('button',{name:'Finish merge',exact:true}).click();
  await expect(inspector).toHaveCount(0,{timeout:45000});const merged=git(repo,['rev-parse','HEAD']);expect(merged).not.toBe(source);expect(git(repo,['show','-s','--format=%P','HEAD'])).toBe(`${original} ${source}`);expect(git(repo,['rev-parse','feature/linear'])).toBe(source);
  const node=page.locator(`button#commit-${merged}`);await expect(node.locator('.merge-node')).toBeVisible();await expect(node.locator('.graph-mark path:not(.ref-connector)')).toHaveCount(2);
  await expect(page.locator(`button#commit-${source}`)).toBeVisible();await page.screenshot({path:testInfo.outputPath('linear-history-explicit-merge-junction.png')});
});

test('visible Push and graph counters publish three reviewed commits then synchronize', async ({page},testInfo) => {
  test.setTimeout(180000);
  const repo=join(folder,'visible-push'),bare=join(folder,'visible-push.git');mkdirSync(repo);git(repo,['init','-b','main']);git(folder,['init','--bare',bare]);
  writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Push base']);const base=git(repo,['rev-parse','HEAD']);git(repo,['remote','add','origin',bare]);git(repo,['push','-u','origin','main']);
  git(repo,['switch','-c','feature']);writeFileSync(join(repo,'file.txt'),'one\n');git(repo,['commit','-am','Feature one']);writeFileSync(join(repo,'file.txt'),'two\n');git(repo,['commit','-am','Feature two']);git(repo,['switch','main']);git(repo,['merge','--no-ff','-m','Merge feature','feature']);const tip=git(repo,['rev-parse','HEAD']);
  await connectRealCore(page,join(folder,'visible-push.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  const current=page.locator('.branch-current .branch-tracking');await expect(current).toContainText('3↑');const badge=page.locator('[data-ref-name="refs/heads/main"] .branch-tracking');await expect(badge).toContainText('3↑');
  await page.getByRole('button',{name:'Switch branch',exact:true}).click();await expect(page.getByRole('button',{name:'Select branch main',exact:true})).toContainText('3↑');await page.keyboard.press('Escape');
  await expect(page.locator('.push-current')).toBeVisible();await page.locator('.push-current').click();const panel=page.locator('.ref-manager');await expect(page.getByLabel('Remote',{exact:true})).toHaveValue('origin');await expect(page.getByLabel('Destination branch',{exact:true})).toHaveValue('main');
  await panel.getByRole('button',{name:'Check destination & review',exact:true}).click();await expect(panel).toContainText('3 outgoing commits');expect(git(bare,['rev-parse','main'])).toBe(base);await page.screenshot({path:testInfo.outputPath('visible-push-three-ahead-preview.png')});
  await panel.getByRole('button',{name:'Push reviewed branch',exact:true}).click();await expect(panel).toHaveCount(0,{timeout:45000});await expect(current).toContainText('✓');await expect(badge).toContainText('✓');expect(git(bare,['rev-parse','main'])).toBe(tip);expect(git(repo,['rev-parse','HEAD'])).toBe(tip);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('two\n');
});

test('configured upstream counters show both directions and direct graph Push keeps the actual destination',async({page},testInfo)=>{
  test.setTimeout(180000);await page.setViewportSize({width:1000,height:850});
  const repo=join(folder,'tracking-target'),bare=join(folder,'tracking-target.git'),other=join(folder,'tracking-writer');mkdirSync(repo);git(repo,['init','-b','main']);git(folder,['init','--bare',bare]);
  writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Tracking base']);git(repo,['remote','add','origin',bare]);git(repo,['push','-u','origin','main:release']);git(repo,['config','push.default','upstream']);git(repo,['remote','add','backup',bare]);
  git(folder,['clone','--branch','release',bare,other]);git(repo,['commit','--allow-empty','-m','Local ahead']);const local=git(repo,['rev-parse','HEAD']);for(const message of ['Remote one','Remote two'])git(other,['commit','--allow-empty','-m',message]);git(other,['push','origin','release']);const remote=git(other,['rev-parse','HEAD']);git(repo,['fetch','origin']);
  await connectRealCore(page,join(folder,'tracking-target.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  const counter=page.locator('.branch-current .branch-tracking');await expect(counter).toContainText('1↑');await expect(counter).toContainText('2↓');await expect(counter).toHaveAttribute('title',/origin\/release/);
  expect(await page.locator('.topbar').evaluate(node=>node.scrollWidth<=node.clientWidth+1)).toBe(true);
  await page.locator(`button#commit-${local}`).click({button:'right'});await page.getByRole('menuitem',{name:'Push main…',exact:true}).click();await expect(page.getByLabel('Remote',{exact:true})).toHaveValue('origin');await expect(page.getByLabel('Destination branch',{exact:true})).toHaveValue('release');
  await page.getByRole('button',{name:'Check destination & review',exact:true}).click();const panel=page.locator('.ref-manager');await expect(panel.locator('.push-review')).toContainText('main → origin/release');await expect(panel.getByRole('button',{name:'Push reviewed branch',exact:true})).toBeDisabled();await expect(panel.locator('.push-review')).not.toContainText('0 outgoing commits');expect(git(bare,['rev-parse','release'])).toBe(remote);expect(git(repo,['rev-parse','HEAD'])).toBe(local);await page.screenshot({path:testInfo.outputPath('ahead-behind-diverged-push-review.png')});
});

test('remote picker Fetch refreshes counters and Pull reviews current branch with differently named upstream', async ({page},testInfo) => {
  test.setTimeout(180000);await page.setViewportSize({width:1000,height:850});
  const repo=join(folder,'direct-sync'),bare=join(folder,'direct-sync.git'),writer=join(folder,'direct-sync-writer');mkdirSync(repo);git(repo,['init','-b','main']);git(folder,['init','--bare',bare]);
  writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Sync base']);const base=git(repo,['rev-parse','HEAD']);git(repo,['remote','add','origin',bare]);git(repo,['push','-u','origin','main:release']);git(repo,['branch','release']);git(folder,['clone','--branch','release',bare,writer]);
  await connectRealCore(page,join(folder,'direct-sync.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.branch-current .branch-tracking')).toHaveText('✓');
  await expect(page.locator('.topbar .branch-current,.topbar .sync-pull,.topbar .push-current')).toHaveCount(0);await expect(page.locator('.contextbar .branch-current')).toBeVisible();await expect(page.locator('.contextbar .sync-pull')).toBeVisible();await expect(page.locator('.contextbar .push-current')).toBeVisible();
  for(const width of [1360,1000,780]){
    await page.setViewportSize({width,height:width===780?560:900});
    for(const selector of ['.topbar','.contextbar'])expect(await page.locator(selector).evaluate(node=>node.scrollWidth<=node.clientWidth+1)).toBe(true);
    expect((await page.locator('.brand-mark').boundingBox())!.width).toBe(width<=850?40:44);await expect(page.locator('.app-footer')).toContainText('GitNova · by edocstudio');await expect(page.locator('.context-subtitle')).toHaveText('direct-sync');
    await page.locator('.branch-current').click();const box=(await page.locator('.branch-menu').boundingBox())!;expect(box.x).toBeGreaterThanOrEqual(0);expect(box.x+box.width).toBeLessThanOrEqual(width);expect(box.y+box.height).toBeLessThanOrEqual(width===780?560:900);await page.keyboard.press('Escape');
    await page.screenshot({path:testInfo.outputPath(`repository-toolbar-${width}.png`)});
  }
  await page.setViewportSize({width:1000,height:850});
  writeFileSync(join(writer,'file.txt'),'remote change\n');git(writer,['commit','-am','Incoming remote change']);git(writer,['push','origin','release']);const target=git(writer,['rev-parse','HEAD']);
  await expect(page.locator('.sync-fetch')).toHaveCount(0);await page.locator('.branch-current').click();await page.getByRole('button',{name:/^Remotes/}).click();await page.getByRole('button',{name:'Fetch branches from origin',exact:true}).click();await expect(page.getByRole('status').filter({hasText:'Remote branches refreshed.'})).toBeVisible({timeout:45000});await page.keyboard.press('Escape');await expect(page.locator('.branch-current .branch-tracking')).toHaveText('1↓');expect(git(repo,['rev-parse','HEAD'])).toBe(base);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('base\n');
  expect(await page.locator('.topbar').evaluate(node=>node.scrollWidth<=node.clientWidth+1)).toBe(true);
  await page.locator('.sync-pull').click();const panel=page.locator('.sync-review');await expect(panel.locator('.pull-preview')).toContainText('origin/release → main',{timeout:45000});await expect(panel.locator('.pull-preview')).toContainText('1 to pull');expect(git(repo,['rev-parse','HEAD'])).toBe(base);await page.screenshot({path:testInfo.outputPath('fetch-pull-current-review.png')});await panel.getByRole('button',{name:'Close',exact:true}).click();expect(git(repo,['rev-parse','HEAD'])).toBe(base);
  await page.locator(`button#commit-${base}`).click({button:'right'});await page.getByRole('menuitem',{name:'Pull into main…',exact:true}).click();await expect(panel.locator('.pull-preview')).toContainText('origin/release → main',{timeout:45000});await panel.getByRole('button',{name:'Pull into main',exact:true}).click();await expect(panel).toHaveCount(0,{timeout:45000});await expect(page.locator('.branch-current .branch-tracking')).toHaveText('✓');expect(git(repo,['branch','--show-current'])).toBe('main');expect(git(repo,['rev-parse','HEAD'])).toBe(target);expect(git(repo,['rev-parse','release'])).toBe(base);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('remote change\n');
});

test('remote picker Fetch selects among multiple remotes and Pull explains missing upstream without changing dirty work', async ({page}) => {
  test.setTimeout(120000);const repo=join(folder,'sync-choice'),one=join(folder,'sync-one.git'),two=join(folder,'sync-two.git');mkdirSync(repo);git(repo,['init','-b','main']);git(folder,['init','--bare',one]);git(folder,['init','--bare',two]);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Choice base']);const base=git(repo,['rev-parse','HEAD']);git(repo,['remote','add','one',one]);git(repo,['remote','add','two',two]);git(repo,['push','one','main']);writeFileSync(join(repo,'file.txt'),'dirty\n');
  await connectRealCore(page,join(folder,'sync-choice.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.branch-current').click();await page.getByRole('button',{name:/^Remotes/}).click();await expect(page.getByRole('button',{name:'Fetch branches from two',exact:true})).toBeVisible();await page.getByRole('button',{name:'Fetch branches from one',exact:true}).click();await expect(page.getByRole('status').filter({hasText:'Remote branches refreshed.'})).toBeVisible({timeout:45000});await page.keyboard.press('Escape');const panel=page.locator('.sync-review');expect(git(repo,['rev-parse','HEAD'])).toBe(base);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('dirty\n');await page.locator('.sync-pull').click();await expect(panel.getByRole('alert')).toContainText('Configure a remote upstream');expect(git(repo,['rev-parse','HEAD'])).toBe(base);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('dirty\n');
});

test('clone reviews the absent destination then opens real history with native loading phases',async({page},testInfo)=>{
  test.setTimeout(120000);const source=join(folder,'clone-source'),destination=join(folder,'cloned-project');mkdirSync(source);git(source,['init','-b','main']);writeFileSync(join(source,'file.txt'),'real clone\n');git(source,['add','.']);git(source,['commit','-m','Clone browser fixture']);const expected=git(source,['rev-parse','HEAD']);
  await connectRealCore(page,join(folder,'clone-ui.json'));await page.goto('/');await page.locator('.clone-controls > summary').click();const controls=page.locator('.clone-controls');await controls.getByLabel('Repository URL or local source',{exact:true}).fill(source);await controls.getByLabel('New clone folder',{exact:true}).fill(destination);await controls.getByRole('button',{name:'Review clone',exact:true}).click();await expect(controls.locator('.clone-preview')).toContainText('cloned-project');expect(existsSync(destination)).toBe(false);await controls.getByRole('button',{name:'Clone & open graph',exact:true}).scrollIntoViewIfNeeded();await page.screenshot({path:testInfo.outputPath('clone-workspace-review.png')});await controls.getByRole('button',{name:'Clone & open graph',exact:true}).click();await expect(page.locator(`button#commit-${expected}`)).toBeVisible({timeout:45000});await expect(page.locator('.repo-picker')).toContainText('cloned-project');expect(readFileSync(join(destination,'file.txt'),'utf8')).toBe('real clone\n');expect(git(destination,['rev-parse','--symbolic-full-name','@{upstream}'])).toBe('refs/remotes/origin/main');
  const phases=await page.evaluate(()=>(window as unknown as {gitnovaProgressEvents?:{phase:string}[]}).gitnovaProgressEvents?.map(event=>event.phase)??[]);expect(phases).toContain('cloning');expect(phases).toContain('verifying-clone');
});

test('workspace clone retains drafts and refuses a destination created after preview',async({page})=>{
  test.setTimeout(120000);const source=join(folder,'clone-existing-source'),destination=join(folder,'clone-raced-destination');mkdirSync(source);git(source,['init','-b','main']);git(source,['commit','--allow-empty','-m','Existing source']);
  await connectRealCore(page,join(folder,'clone-race.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(source);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.repo-picker').click();await page.locator('.clone-controls > summary').click();const controls=page.locator('.clone-controls');await controls.getByLabel('Repository URL or local source',{exact:true}).fill(source);await controls.getByLabel('New clone folder',{exact:true}).fill(destination);await page.keyboard.press('Escape');await expect(controls).toHaveCount(0);await page.locator('.repo-picker').click();await page.locator('.clone-controls > summary').click();await expect(controls.getByLabel('New clone folder',{exact:true})).toHaveValue(destination);await controls.getByRole('button',{name:'Review clone',exact:true}).click();await expect(controls.locator('.clone-preview')).toBeVisible();mkdirSync(destination);writeFileSync(join(destination,'keep.txt'),'untouched');await controls.getByRole('button',{name:'Clone & open graph',exact:true}).click();await expect(controls.getByRole('alert')).toContainText('already exists');expect(readFileSync(join(destination,'keep.txt'),'utf8')).toBe('untouched');expect(existsSync(join(destination,'.git'))).toBe(false);expect(git(source,['log','-1','--format=%s'])).toBe('Existing source');
});

test('failed clone remains visible after restart without creating a repository membership',async({page})=>{
  test.setTimeout(120000);const source=join(folder,'not-a-git-source'),destination=join(folder,'failed-clone');mkdirSync(source);writeFileSync(join(source,'keep.txt'),'source untouched');await connectRealCore(page,join(folder,'clone-recovery.json'));await page.goto('/');await page.locator('.clone-controls > summary').click();const controls=page.locator('.clone-controls');await controls.getByLabel('Repository URL or local source',{exact:true}).fill(source);await controls.getByLabel('New clone folder',{exact:true}).fill(destination);await controls.getByRole('button',{name:'Review clone',exact:true}).click();await controls.getByRole('button',{name:'Clone & open graph',exact:true}).click();await expect(controls.locator('.notice')).toContainText('Clone stopped',{timeout:45000});await page.reload();await page.locator('.clone-controls > summary').click();await controls.locator('.clone-attempts > summary').click();await expect(controls.locator('.clone-attempts')).toContainText('failed-clone');await expect(controls.locator('.clone-attempts')).toContainText('Clone stopped');await expect(page.locator('.repository-view')).toHaveCount(0);expect(readFileSync(join(source,'keep.txt'),'utf8')).toBe('source untouched');
});

function pickerFixture(name: string, pinned: number[]) {
  const parent=join(folder,name),state=join(folder,`${name}.json`);mkdirSync(parent);const paths:string[]=[];
  const invoke=(command:string,args:unknown):Catalog=>{const result=JSON.parse(execFileSync(bridge,[state],{input:JSON.stringify({command,args}),encoding:'utf8',maxBuffer:4*1024*1024}));if(result.error)throw new Error(result.error.message);return result.ok;};
  let catalog=invoke('catalog',{});
  for(let index=0;index<7;index++){const path=join(parent,index===1?'team-a':'team-b',`project-${index}`);mkdirSync(path,{recursive:true});git(path,['init','-b','main']);git(path,['commit','--allow-empty','-m',`Picker project ${index}`]);paths.push(path);catalog=invoke('add_repository',{path});}
  for(const index of pinned){const membership=catalog.workspaces[0].memberships.find((entry:{name:string})=>entry.name===`project-${index}`);catalog=invoke('pin_repository',{membershipId:membership.id});}
  return {state,paths,catalog};
}

test('repository picker limits pages to five, filters every page by name and path, and reads visible statuses only',async({page},testInfo)=>{
  test.setTimeout(150000);const fixture=pickerFixture('picker-pages',[0]);const statusIds:string[]=[];
  await connectRealCore(page,fixture.state,(command,args)=>{if(command==='repository_status')statusIds.push((args as {membershipId:string}).membershipId);});await page.goto('/');await expect(page.locator('.repo-picker')).toContainText('project-6');await page.locator('.repo-picker').click();const list=page.locator('.repo-list');const search=list.getByRole('textbox',{name:'Search repositories',exact:true});await expect(search).toBeFocused();await expect(list.locator('.repo-option')).toHaveCount(5);await expect(list.locator('.repo-option strong').first()).toHaveText('project-0');await expect(list).toContainText('1–5 of 7 repositories');await expect(list.locator('.repo-page')).not.toContainText('Reading status…');
  const firstPageIds=new Set(fixture.catalog.workspaces[0].memberships.filter((entry:{name:string})=>['project-0','project-6','project-5','project-4','project-3'].includes(entry.name)).map((entry:{id:string})=>entry.id));expect(statusIds.every(id=>firstPageIds.has(id))).toBe(true);expect(statusIds.length).toBeLessThanOrEqual(5);statusIds.length=0;
  await list.getByRole('button',{name:'Next repository page',exact:true}).click();await expect(list.locator('.repo-option')).toHaveCount(2);await expect(list).toContainText('Page 2 of 2');await expect(list.locator('.repo-page')).not.toContainText('Reading status…');expect(statusIds.length).toBeLessThanOrEqual(2);await expect(page.locator('.repo-picker')).toContainText('project-6');
  await search.fill('PROJECT-1');await expect(list.locator('.repo-option')).toHaveCount(1);await expect(list.locator('.repo-option strong')).toHaveText('project-1');await expect(list.getByRole('navigation')).toHaveCount(0);await search.fill('TEAM-A');await expect(list.locator('.repo-option strong')).toHaveText('project-1');await expect(list.locator('.repo-option-path')).toContainText('team-a');
  await search.fill('unmatched-repository');await expect(list.locator('.repo-option')).toHaveCount(0);await expect(list).toContainText('No matching repositories.');await list.getByRole('button',{name:'Clear repository search',exact:true}).click();await expect(list.locator('.repo-option')).toHaveCount(5);await expect(list).toContainText('Page 1 of 2');await page.screenshot({path:testInfo.outputPath('repository-picker-five-and-search.png')});
  await list.getByRole('button',{name:'Next repository page',exact:true}).click();await list.getByRole('button',{name:'Pin project-1',exact:true}).click();await expect(list).toContainText('Page 1 of 2');await expect(list.locator('.repo-option strong').first()).toHaveText('project-1');await expect(page.locator('.repo-picker')).toContainText('project-6');
  await search.fill('project-2');await list.locator('.repo-option-main').click();await expect(page.locator('.repo-picker')).toContainText('project-2');await expect(page.locator('.commit-row')).toContainText('Picker project 2');
});

test('repository picker opens the selected page and keeps search and pagination through language changes',async({page})=>{
  test.setTimeout(150000);const fixture=pickerFixture('picker-active-page',[0,1,2,3,4,5]);await connectRealCore(page,fixture.state);await page.goto('/');await expect(page.locator('.repo-picker')).toContainText('project-6');await page.locator('.repo-picker').click();const list=page.locator('.repo-list');await expect(list).toContainText('Page 2 of 2');await expect(list.locator('.repo-option.active')).toContainText('project-6');await expect(list.locator('.repo-option')).toHaveCount(2);
  await changeLanguageThroughSettings(page,'th');await expect(list).toContainText('หน้า 2 จาก 2');await expect(list.locator('.repo-option.active')).toContainText('project-6');const search=list.getByRole('textbox',{name:'ค้นหา repository',exact:true});await search.fill('project-4');await expect(list.locator('.repo-option')).toHaveCount(1);await changeLanguageThroughSettings(page,'en');await expect(list.getByRole('textbox',{name:'Search repositories',exact:true})).toHaveValue('project-4');await expect(list.locator('.repo-option strong')).toHaveText('project-4');await expect(page.locator('.repo-picker')).toContainText('project-6');
});

test('submodule Working changes separates dirty child files from a reviewed parent pointer commit',async({page},testInfo)=>{
  test.setTimeout(180000);const repo=join(folder,'submodule-ux-parent'),source=join(folder,'submodule-ux-source');for(const path of [repo,source]){mkdirSync(path);git(path,['init','-b','main']);writeFileSync(join(path,'file.txt'),'base\n');git(path,['add','.']);git(path,['commit','-m','Submodule UX base']);}git(repo,['-c','protocol.file.allow=always','submodule','add',source,'_dags']);git(repo,['commit','-am','Record child']);const child=join(repo,'_dags'),recorded=git(child,['rev-parse','HEAD']);writeFileSync(join(child,'file.txt'),'committed child\n');git(child,['commit','-am','Child commit']);const next=git(child,['rev-parse','HEAD']);writeFileSync(join(child,'file.txt'),'uncommitted child files\n');writeFileSync(join(child,'notes.txt'),'untracked notes\n');
  await connectRealCore(page,join(folder,'submodule-ux.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();const sub=page.locator('.working-submodule');await expect(sub).toContainText('Commit changed');await expect(sub).toContainText('Uncommitted changes inside');await expect(sub).toContainText(recorded.slice(0,8));await expect(sub).toContainText(next.slice(0,8));await expect(sub.getByRole('button',{name:'Open submodule',exact:true})).toHaveClass(/primary/);await sub.getByRole('button',{name:'Stage pointer…',exact:true}).click();const review=sub.getByRole('region',{name:'Review submodule pointer',exact:true});await expect(review).toContainText('Updates the parent index only.');await expect(review).toContainText('stay outside the parent commit');expect(git(repo,['rev-parse',':_dags'])).toBe(recorded);await page.screenshot({path:testInfo.outputPath('submodule-dirty-pointer-review.png')});await review.getByRole('button',{name:'Stage pointer',exact:true}).click();await expect(page.getByRole('region',{name:'Staged files',exact:true}).locator('.changed-file')).toHaveCount(1,{timeout:45000});expect(git(repo,['rev-parse',':_dags'])).toBe(next);expect(readFileSync(join(child,'file.txt'),'utf8')).toBe('uncommitted child files\n');
  await page.getByLabel('Summary',{exact:true}).fill('Update child pointer');await page.getByRole('button',{name:'Review commit',exact:true}).click();await expect(page.getByRole('button',{name:'Commit staged changes',exact:true})).toBeEnabled();await page.getByRole('button',{name:'Commit staged changes',exact:true}).click();await expect(page.getByRole('region',{name:'Staged files',exact:true}).locator('.changed-file')).toHaveCount(0,{timeout:45000});expect(git(repo,['rev-parse','HEAD:_dags'])).toBe(next);expect(readFileSync(join(child,'file.txt'),'utf8')).toBe('uncommitted child files\n');expect(readFileSync(join(child,'notes.txt'),'utf8')).toBe('untracked notes\n');await sub.getByRole('button',{name:'Open submodule',exact:true}).click();await expect(page.locator('.repo-picker')).toContainText('_dags');await page.locator('.working-row').click();await expect(page.getByRole('region',{name:'Unstaged files',exact:true}).locator('.changed-file')).toHaveCount(2);expect(git(child,['rev-parse','HEAD'])).toBe(next);
});

test('submodule pointer context menu stages and unstages without moving the child checkout',async({page})=>{
  test.setTimeout(150000);const repo=join(folder,'pointer-menu-parent'),source=join(folder,'pointer-menu-source');for(const path of [repo,source]){mkdirSync(path);git(path,['init','-b','main']);git(path,['commit','--allow-empty','-m','Pointer menu base']);}git(repo,['-c','protocol.file.allow=always','submodule','add',source,'module']);git(repo,['commit','-am','Record module']);const child=join(repo,'module'),recorded=git(child,['rev-parse','HEAD']);git(child,['commit','--allow-empty','-m','Child next']);const next=git(child,['rev-parse','HEAD']);await connectRealCore(page,join(folder,'pointer-menu.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();const row=page.getByRole('region',{name:'Unstaged files',exact:true}).locator('.changed-file');await row.click({button:'right'});await row.dispatchEvent('pointerdown',{button:2});await expect(page.getByRole('menuitem',{name:'Stage pointer…',exact:true})).toBeVisible();await page.getByRole('menuitem',{name:'Stage pointer…',exact:true}).click();let review=page.getByRole('region',{name:'Review submodule pointer',exact:true});await expect(review).toContainText(next.slice(0,12));await review.getByRole('button',{name:'Back',exact:true}).click();expect(git(repo,['rev-parse',':module'])).toBe(recorded);await row.click({button:'right'});await page.getByRole('menuitem',{name:'Stage pointer…',exact:true}).click();await page.getByRole('region',{name:'Review submodule pointer',exact:true}).getByRole('button',{name:'Stage pointer',exact:true}).click();const staged=page.getByRole('region',{name:'Staged files',exact:true}).locator('.changed-file');await expect(staged).toHaveCount(1,{timeout:45000});await staged.click({button:'right'});await page.getByRole('menuitem',{name:'Unstage pointer…',exact:true}).click();review=page.getByRole('region',{name:'Review submodule pointer',exact:true});await expect(review).toContainText(recorded.slice(0,12));await review.getByRole('button',{name:'Unstage pointer',exact:true}).click();await expect(staged).toHaveCount(0,{timeout:45000});expect(git(repo,['rev-parse',':module'])).toBe(recorded);expect(git(child,['rev-parse','HEAD'])).toBe(next);await row.dblclick();await expect(page.locator('.repo-picker')).toContainText('module');expect(git(child,['rev-parse','HEAD'])).toBe(next);
});

test('branch submodules stay visible when clean and follow each parent branch without moving the child', async ({page},testInfo)=>{
  test.setTimeout(150000);
  const repo=join(folder,'branch-modules-parent'),source=join(folder,'branch-modules-source');
  for(const path of [repo,source]){mkdirSync(path);git(path,['init','-b','main']);git(path,['commit','--allow-empty','-m','Branch modules base']);}
  git(source,['commit','--allow-empty','-m','Child first']);const first=git(source,['rev-parse','HEAD']);
  git(source,['commit','--allow-empty','-m','Child second']);const second=git(source,['rev-parse','HEAD']);
  git(repo,['-c','protocol.file.allow=always','submodule','add',source,'module']);const child=join(repo,'module');
  git(child,['checkout','--detach',first]);git(repo,['commit','-am','Record first child']);
  git(repo,['checkout','-b','develop']);git(child,['checkout','--detach',second]);git(repo,['commit','-am','Record second child']);
  git(repo,['checkout','main']);git(child,['checkout','--detach',first]);
  await connectRealCore(page,join(folder,'branch-modules.json'));await page.goto('/');
  await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  const group=page.getByRole('region',{name:'Branch submodules',exact:true});
  await expect(group.locator('.graph-submodules-context')).toHaveText('main');
  await expect(group.locator('.branch-submodule-row')).toHaveCount(1);
  await expect(group).toContainText('Files inside are clean');await expect(group).not.toContainText('Commit changed');
  await expect(page.locator('.working-row')).toContainText('Your working tree is clean');
  await page.screenshot({path:testInfo.outputPath('branch-submodules-clean.png')});
  await group.getByRole('button',{name:/Manage submodule/}).click();
  await expect(group.locator('.submodule-summary')).toContainText(first.slice(0,8));
  await expect(group.getByRole('button',{name:'Stage pointer…',exact:true})).toHaveCount(0);
  await group.getByRole('button',{name:'Back',exact:true}).click();
  git(repo,['checkout','develop']);await page.getByRole('button',{name:'Refresh',exact:true}).click();
  await expect(group.locator('.graph-submodules-context')).toHaveText('develop');
  await expect(group).toContainText('Commit changed');expect(git(child,['rev-parse','HEAD'])).toBe(first);
  await group.locator('.branch-submodule-row').click({button:'right'});
  const summary=group.locator('.submodule-summary');await expect(summary).toContainText(second.slice(0,8));await expect(summary).toContainText(first.slice(0,8));
  expect((await page.getByLabel('Commit history',{exact:true}).boundingBox())!.height).toBeGreaterThan(100);
  await expect(group.getByRole('button',{name:'Stage pointer…',exact:true})).toBeInViewport();
  await expect(group.getByRole('button',{name:'Restore recorded…',exact:true})).toBeInViewport();
  await page.screenshot({path:testInfo.outputPath('branch-submodules.png')});
  await group.getByRole('button',{name:'Stage pointer…',exact:true}).click();
  await expect(group.getByRole('region',{name:'Review submodule pointer',exact:true})).toContainText(first.slice(0,12));
  expect(git(repo,['rev-parse',':module'])).toBe(second); // Opening review never stages.
  await group.getByRole('region',{name:'Review submodule pointer',exact:true}).getByRole('button',{name:'Back',exact:true}).click();
  await group.getByRole('button',{name:'Stage pointer…',exact:true}).click();
  await group.getByRole('region',{name:'Review submodule pointer',exact:true}).getByRole('button',{name:'Stage pointer',exact:true}).click();
  await expect(group.locator('.branch-submodule-row')).toContainText('Pointer staged',{timeout:45000});
  expect(git(repo,['rev-parse',':module'])).toBe(first);expect(git(repo,['rev-parse','HEAD:module'])).toBe(second);expect(git(child,['rev-parse','HEAD'])).toBe(first);
  await group.getByRole('button',{name:/Manage submodule/}).click();
  await group.getByRole('button',{name:'Unstage pointer…',exact:true}).click();
  await group.getByRole('region',{name:'Review submodule pointer',exact:true}).getByRole('button',{name:'Unstage pointer',exact:true}).click();
  await expect(group.locator('.branch-submodule-row')).toContainText('Commit changed',{timeout:45000});
  await expect(group).not.toContainText('Pointer staged');expect(git(repo,['rev-parse',':module'])).toBe(second);
  await group.locator('.graph-submodules-heading > button').first().click();
  await expect(group.locator('.branch-submodule-row')).toHaveCount(0);
  await group.locator('.graph-submodules-heading > button').first().click();
  await expect(group.locator('.branch-submodule-row')).toHaveCount(1);
  await group.locator('.branch-submodule-open').click();await expect(page.locator('.repo-picker')).toContainText('module');
  expect(git(child,['rev-parse','HEAD'])).toBe(first);expect(git(repo,['symbolic-ref','--short','HEAD'])).toBe('develop');
  await expect(page.getByRole('region',{name:'Branch submodules',exact:true})).toContainText('No submodules recorded in this checkout.');
});

test('late submodule discovery cannot replace the next branch context',async({page})=>{
  test.setTimeout(120000);const repo=join(folder,'late-module-parent'),source=join(folder,'late-module-source');
  for(const path of [repo,source]){mkdirSync(path);git(path,['init','-b','main']);git(path,['commit','--allow-empty','-m','Late discovery base']);}
  git(repo,['-c','protocol.file.allow=always','submodule','add',source,'module']);git(repo,['commit','-am','Record module']);
  git(repo,['branch','develop']);
  await connectRealCore(page,join(folder,'late-module.json'));
  await page.addInitScript(()=>{
    const w=window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args:unknown)=>Promise<unknown>};snapshotHeld?:boolean;releaseSnapshot?:()=>void};
    const original=w.__TAURI_INTERNALS__.invoke;let first=true;
    w.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      const value=await original(command,args);
      if(command==='submodule_snapshot'&&first){first=false;w.snapshotHeld=true;await new Promise<void>(resolve=>{w.releaseSnapshot=resolve;});}
      return value;
    };
  });
  await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {snapshotHeld?:boolean}).snapshotHeld)).toBe(true);
  const group=page.getByRole('region',{name:'Branch submodules',exact:true});await expect(group).toContainText('Reading branch submodules…');
  git(repo,['checkout','develop']);await page.getByRole('button',{name:'Refresh',exact:true}).click();
  await expect(group.locator('.graph-submodules-context')).toHaveText('develop');await expect(group.locator('.branch-submodule-row')).toHaveCount(1);
  await group.getByRole('button',{name:/Manage submodule/}).click();await expect(group.locator('.submodule-summary')).toBeVisible();
  await page.evaluate(()=>(window as unknown as {releaseSnapshot?:()=>void}).releaseSnapshot?.());
  await expect(group.locator('.graph-submodules-context')).toHaveText('develop');await expect(group.locator('.submodule-summary')).toBeVisible();
  expect(git(repo,['symbolic-ref','--short','HEAD'])).toBe('develop');
});

function divergedPullFixture(name:string){
  const repo=join(folder,`${name}-local`),bare=join(folder,`${name}.git`),writer=join(folder,`${name}-writer`);
  mkdirSync(repo);git(repo,['init','-b','develop']);git(folder,['init','--bare',bare]);
  writeFileSync(join(repo,'base.txt'),'shared base\n');git(repo,['add','.']);git(repo,['commit','-m','Shared pull base']);const base=git(repo,['rev-parse','HEAD']);
  git(repo,['remote','add','origin',bare]);git(repo,['push','-u','origin','develop']);git(folder,['clone','--branch','develop',bare,writer]);
  writeFileSync(join(repo,'local.txt'),'local work preserved\n');git(repo,['add','.']);git(repo,['commit','-m','Local independent commit']);const local=git(repo,['rev-parse','HEAD']);
  for(let i=1;i<=4;i++){writeFileSync(join(writer,'remote.txt'),`incoming revision ${i}\n`);git(writer,['add','.']);git(writer,['commit','-m',`Incoming ${i}`]);}
  git(writer,['push','origin','develop']);return {repo,base,local,remote:git(writer,['rev-parse','HEAD'])};
}

test('diverged Pull reviews and finishes a merge preserving both tips instead of showing zero paths',async({page},testInfo)=>{
  test.setTimeout(240000);const f=divergedPullFixture('diverged-pull');let fetches=0;
  await connectRealCore(page,join(folder,'diverged-pull.json'),command=>{if(command==='fetch_remote')fetches++;});await page.goto('/');
  await page.getByLabel('Add an existing repository').fill(f.repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  const index=readFileSync(join(f.repo,'.git/index'));await page.locator('.sync-pull').click();const panel=page.locator('.sync-review');
  await expect(panel).toContainText('1 local commit · 4 incoming commits',{timeout:45000});await expect(panel).toContainText(f.remote.slice(0,12));await expect(panel).not.toContainText('0 paths change');await expect(panel.getByRole('button',{name:'Pull into develop',exact:true})).toHaveCount(0);
  await page.screenshot({path:testInfo.outputPath('diverged-pull-review.png')});
  await panel.getByRole('button',{name:'Review merge…',exact:true}).click();
  await expect(panel.getByRole('button',{name:'Start merge',exact:true})).toBeEnabled({timeout:45000});await expect(panel).toContainText('origin/develop');await expect(panel.locator('.branch-preview-files')).toContainText('remote.txt');
  expect(git(f.repo,['rev-parse','HEAD'])).toBe(f.local);expect(existsSync(join(f.repo,'.git/MERGE_HEAD'))).toBe(false);expect(readFileSync(join(f.repo,'.git/index'))).toEqual(index);
  await panel.getByRole('button',{name:'Back',exact:true}).click();await expect(panel.getByRole('button',{name:'Review merge…',exact:true})).toBeVisible();expect(fetches).toBe(1);
  await panel.getByRole('button',{name:'Review merge…',exact:true}).click();await panel.getByRole('button',{name:'Start merge',exact:true}).click();
  const inspector=page.getByLabel('Merge working changes',{exact:true});await expect(inspector).toBeVisible({timeout:45000});expect(git(f.repo,['rev-parse','HEAD'])).toBe(f.local);
  await inspector.getByRole('button',{name:'Review merge commit',exact:true}).click();await inspector.getByRole('button',{name:'Finish merge',exact:true}).click();
  await expect(page.locator('.working-row')).toContainText('Your working tree is clean',{timeout:45000});expect(git(f.repo,['show','-s','--format=%P','HEAD'])).toBe(`${f.local} ${f.remote}`);expect(readFileSync(join(f.repo,'local.txt'),'utf8')).toBe('local work preserved\n');expect(readFileSync(join(f.repo,'remote.txt'),'utf8')).toBe('incoming revision 4\n');expect(git(f.repo,['branch','--show-current'])).toBe('develop');
});

test('diverged Pull merge handoff refuses changed branches, dirty work and moved remote refs',async({page})=>{
  test.setTimeout(240000);const f=divergedPullFixture('diverged-pull-guards');
  await connectRealCore(page,join(folder,'diverged-pull-guards.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(f.repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  const panel=page.locator('.sync-review');await page.locator('.sync-pull').click();await expect(panel.getByRole('button',{name:'Review merge…',exact:true})).toBeEnabled({timeout:45000});
  git(f.repo,['checkout','-b','other']);await panel.getByRole('button',{name:'Review merge…',exact:true}).click();await expect(panel.getByRole('alert')).toContainText('Branch or upstream changed',{timeout:45000});await expect(panel.getByRole('button',{name:'Start merge',exact:true})).toBeDisabled();expect(existsSync(join(f.repo,'.git/MERGE_HEAD'))).toBe(false);
  await panel.getByRole('button',{name:'Back',exact:true}).click();await panel.getByRole('button',{name:'Close',exact:true}).click();git(f.repo,['checkout','develop']);writeFileSync(join(f.repo,'local.txt'),'uncommitted work\n');await page.getByRole('button',{name:'Refresh',exact:true}).click();
  await page.locator('.sync-pull').click();await panel.getByRole('button',{name:'Review merge…',exact:true}).click();await expect(panel).toContainText('Commit or preserve uncommitted work',{timeout:45000});await expect(panel.getByRole('button',{name:'Start merge',exact:true})).toBeDisabled();await panel.getByRole('button',{name:'Review working changes',exact:true}).click();
  expect(readFileSync(join(f.repo,'local.txt'),'utf8')).toBe('uncommitted work\n');expect(git(f.repo,['stash','list'])).toBe('');expect(existsSync(join(f.repo,'.git/MERGE_HEAD'))).toBe(false);
  git(f.repo,['restore','--','local.txt']);await page.getByRole('button',{name:'Refresh',exact:true}).click();await page.locator('.sync-pull').click();await expect(panel.getByRole('button',{name:'Review merge…',exact:true})).toBeEnabled({timeout:45000});
  git(f.repo,['update-ref','refs/remotes/origin/develop',f.base]);await panel.getByRole('button',{name:'Review merge…',exact:true}).click();await expect(panel.getByRole('alert')).toContainText('source ref moved',{timeout:45000});await expect(panel.getByRole('button',{name:'Start merge',exact:true})).toBeDisabled();expect(git(f.repo,['rev-parse','HEAD'])).toBe(f.local);expect(existsSync(join(f.repo,'.git/MERGE_HEAD'))).toBe(false);
});

test('annotated version tag is created at the selected old commit and only that tag is published',async({page},testInfo)=>{
 test.setTimeout(180000);const repo=join(folder,'version-tag-ui'),bare=join(folder,'version-tag-ui.git');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['config','user.name','Version User']);git(repo,['config','user.email','version@example.invalid']);git(repo,['config','commit.gpgsign','false']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Version base']);const base=git(repo,['rev-parse','HEAD']);writeFileSync(join(repo,'file.txt'),'second\n');git(repo,['commit','-am','Version next']);const head=git(repo,['rev-parse','HEAD']);git(repo,['tag','not-published']);for(const tag of ['v1.0.0','v2.0.0','v3.0.0','v4.0.0','v9.0.0','v10.0.0'])git(repo,['tag',tag]);git(folder,['init','--bare',bare]);git(repo,['remote','add','origin',bare]);writeFileSync(join(repo,'file.txt'),'working edits\n');const index=readFileSync(join(repo,'.git/index'));
 await connectRealCore(page,join(folder,'version-tag-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator(`button#commit-${base}`).click({button:'right'});await page.getByRole('menuitem',{name:'Create tag…',exact:true}).click();const panel=page.getByLabel('Version tag review',{exact:true});const existing=panel.getByRole('region',{name:'Existing local tags',exact:true});await expect(existing.locator('li')).toHaveCount(5);await expect(existing.locator('li').first()).toContainText('v10.0.0');await existing.getByRole('button',{name:'Next',exact:true}).click();await expect(existing.locator('li')).toHaveCount(2);await existing.getByLabel('Search existing tags',{exact:true}).fill('v1.0.0');await expect(existing.locator('li')).toHaveCount(1);await expect(existing).toContainText('v1.0.0');await existing.getByLabel('Search existing tags',{exact:true}).fill('absent');await expect(existing).toContainText('No matching tags.');await existing.getByLabel('Search existing tags',{exact:true}).fill('');await panel.getByLabel('Tag name',{exact:true}).fill('v1.0.0');await expect(panel).toContainText('This tag name already exists.');await expect(panel.getByRole('button',{name:'Review tag',exact:true})).toBeDisabled();await panel.getByLabel('Tag name',{exact:true}).fill('v1.2.0');await panel.getByLabel('Annotation (optional; defaults to tag name)',{exact:true}).fill('Release 1.2.0');await panel.getByRole('button',{name:'Review tag',exact:true}).click();await expect(panel).toContainText(base.slice(0,12));expect(git(repo,['tag','--list','v1.2.0'])).toBe('');await panel.getByRole('button',{name:'Create reviewed tag',exact:true}).click();await expect(panel).toContainText('Tag created: v1.2.0',{timeout:45000});expect(git(repo,['rev-parse','v1.2.0^{commit}'])).toBe(base);const object=git(repo,['rev-parse','refs/tags/v1.2.0']);expect(object).not.toBe(base);
 await panel.getByRole('button',{name:'Push tag…',exact:true}).click();await panel.getByRole('button',{name:'Check destination & review',exact:true}).click();await expect(panel).toContainText(object);await page.screenshot({path:testInfo.outputPath('version-tag-publish.png')});await panel.getByRole('button',{name:'Push reviewed tag',exact:true}).click();await expect(panel).toHaveCount(0,{timeout:45000});expect(git(bare,['for-each-ref','--format=%(refname)'])).toBe('refs/tags/v1.2.0');expect(git(bare,['rev-parse','refs/tags/v1.2.0'])).toBe(object);expect(git(repo,['rev-parse','HEAD'])).toBe(head);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('working edits\n');expect(readFileSync(join(repo,'.git/index'))).toEqual(index);
 await page.locator(`button#commit-${base}`).click({button:'right'});await page.getByRole('menuitem',{name:'Push tag v1.2.0…',exact:true}).click();await panel.getByRole('button',{name:'Check destination & review',exact:true}).click();await expect(panel).toContainText('Already published at the same tag object.');await panel.getByRole('button',{name:'Verify published tag',exact:true}).click();await expect(panel).toHaveCount(0,{timeout:45000});
});

for(const provider of ['github','gitlab'] as const)test(`provider transport UI ${provider} creates and opens an existing request without secrets in the form`,async({page},testInfo)=>{
 test.setTimeout(120000);const repo=join(folder,`hosting-${provider}`);mkdirSync(repo);git(repo,['init','-b','feature/demo']);git(repo,['commit','--allow-empty','-m','Hosted source']);const oid=git(repo,['rev-parse','HEAD']);git(repo,['remote','add','origin',`git@${provider}.com:team/project.git`]);await connectRealCore(page,join(folder,`hosting-${provider}.json`));
 let needsPublication=provider==='github';let connected=false,postCount=0,existing:Record<string,unknown>|null=null;let lastPreview:Record<string,unknown>|null=null;let outcomeReceipt:Record<string,unknown>={};
 await page.exposeFunction('gitnovaProviderFixture',async(command:string,args:Record<string,unknown>)=>{
  const selection=(args.selection??(args.request as Record<string,unknown>)?.selection) as Record<string,unknown>;
  const context={selection,provider,origin:`https://${provider}.com`,project:'team/project',account:connected?'alice':null};
  if(command==='hosting_context')return context;if(command==='connect_hosting'){connected=true;return {...context,account:'alice'};}if(command==='disconnect_hosting'){connected=false;return {...context,account:null};}
  if(command==='preview_hosting'){const request={...(args.request as Record<string,unknown>),targetProject:'team/project',targetBranch:(args.request as Record<string,unknown>).targetBranch||'develop'};lastPreview={request,context,sourceProjectId:'42',targetProjectId:'42',sourceOid:oid,targetOid:'b'.repeat(40),existing,needsPush:needsPublication,blockedReason:needsPublication?'Publish the selected source branch revision before creating a request.':null,token:'provider-fixture-review'};return lastPreview;}
  if(command==='create_hosting'){const request=(lastPreview!.request as Record<string,unknown>);const created=!existing;if(!existing){postCount++;existing={id:'700',number:17,title:request.title,body:request.body,state:provider==='github'?'open':'opened',draft:request.draft,url:`https://${provider}.com/team/project/${provider==='github'?'pull/':'-/merge_requests/'}17`,sourceBranch:'feature/demo',targetBranch:request.targetBranch,sourceOid:oid};}return {receipt:{...outcomeReceipt,planned:{type:'hosting',preview:lastPreview}},changeRequest:existing,created};}
  throw new Error('Unexpected provider fixture operation');
 });
 await page.addInitScript(()=>{const w=window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args:unknown)=>Promise<unknown>};gitnovaProviderFixture:(command:string,args:unknown)=>Promise<unknown>};const original=w.__TAURI_INTERNALS__.invoke;w.__TAURI_INTERNALS__.invoke=(command,args)=>['hosting_context','connect_hosting','disconnect_hosting','preview_hosting','create_hosting'].includes(command)?w.gitnovaProviderFixture(command,args):original(command,args);});
 await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();const catalog=await page.evaluate(()=>((window as unknown as {__TAURI_INTERNALS__:{invoke:(c:string,a:unknown)=>Promise<Catalog>}}).__TAURI_INTERNALS__.invoke('catalog',{})));const membership=catalog.workspaces[0].memberships[0];outcomeReceipt={id:'provider-ui-receipt',membershipId:membership.id,repositoryId:membership.repositoryId,worktreeId:membership.worktreeId,status:'succeeded',kind:'create_change_request',summary:'Provider request result verified',headBefore:oid,headAfter:oid,startedAt:1,finishedAt:2,recovery:null,graphAction:null,submodule:null,merge:null};
 const open=async()=>{await page.locator(`button#commit-${oid}`).click({button:'right'});await page.getByRole('menuitem',{name:'Create / open PR or MR for feature/demo…',exact:true}).click();};await open();const panel=page.getByLabel('Hosted request review',{exact:true});await expect(panel.getByLabel('Source branch (changes from)',{exact:true})).toHaveValue('feature/demo');await expect(panel.getByLabel('Target branch (merge into)',{exact:true})).toBeVisible();await expect(panel.getByLabel('Request direction',{exact:true})).toContainText('Default branch (resolved on review)');await panel.getByLabel('Target branch (merge into)',{exact:true}).fill('release');await expect(panel.getByLabel('Request direction',{exact:true})).toContainText('feature/demo → release');await page.screenshot({path:testInfo.outputPath(`hosting-${provider}-direction-before-connect.png`)});await panel.getByRole('button',{name:'Connect account…',exact:true}).click();await expect(panel.getByLabel('Target branch (merge into)',{exact:true})).toHaveValue('release');await panel.getByLabel('Request title',{exact:true}).fill(`Ready ${provider}`);await panel.getByLabel('Request description',{exact:true}).fill('Description from GitNova');await panel.getByLabel('Draft',{exact:true}).check();await panel.getByRole('button',{name:'Review request',exact:true}).click();
 if(provider==='github'){
  await panel.getByRole('button',{name:'Review source branch push…',exact:true}).click();const push=page.locator('.ref-manager');await expect(push.getByLabel('Remote',{exact:true})).toHaveValue('origin');await expect(push.getByLabel('Destination branch',{exact:true})).toHaveValue('feature/demo');await push.getByRole('button',{name:'Back',exact:true}).click();await push.getByRole('button',{name:'Back to commit actions',exact:true}).click();await expect(panel.getByLabel('Request title',{exact:true})).toHaveValue('Ready github');await expect(panel.getByLabel('Request description',{exact:true})).toHaveValue('Description from GitNova');await expect(panel.getByLabel('Draft',{exact:true})).toBeChecked();expect(postCount).toBe(0);
  // Transport-only fixture now reports publication; the user still explicitly reviews and creates.
  needsPublication=false;await panel.getByRole('button',{name:'Review request',exact:true}).click();
 }
 await expect(panel).toContainText('team/project:feature/demo → team/project:release');await expect(panel.locator('input[type=password]')).toHaveCount(0);expect(postCount).toBe(0);await page.screenshot({path:testInfo.outputPath(`hosting-${provider}-review.png`)});await panel.getByRole('button',{name:provider==='github'?'Create pull request':'Create merge request',exact:true}).click();await expect(panel).toContainText('Request created · #17');await expect(panel).toContainText('Description from GitNova');expect(postCount).toBe(1);await panel.getByRole('button',{name:'Close',exact:true}).click();
 await open();await panel.getByLabel('Target branch (merge into)',{exact:true}).fill('release');await panel.getByRole('button',{name:'Review request',exact:true}).click();await expect(panel).toContainText('An open request already exists: #17');await panel.getByRole('button',{name:'Open existing request',exact:true}).click();await expect(panel).toContainText('Existing request opened · #17');expect(postCount).toBe(1);expect(git(repo,['rev-parse','HEAD'])).toBe(oid);expect(git(repo,['branch','--show-current'])).toBe('feature/demo');
});

test('graph branch deletion exposes separate local and remote reviews and keeps local work', async ({page},testInfo)=>{
 test.setTimeout(180000);const {repo,bare,tip,panel}=await managementFixture(page,'delete-graph');
 git(bare,['symbolic-ref','HEAD','refs/heads/main']);git(repo,['push','origin','feature/manage']);git(repo,['fetch','origin']);
 await page.getByRole('button',{name:'Refresh',exact:true}).click();
 await expect(page.getByRole('button',{name:'Refresh',exact:true})).toBeEnabled();await page.locator(`button#commit-${tip}`).click({button:'right'});
 await page.getByRole('menuitem',{name:'Delete remote branch origin/feature/manage…',exact:true}).click();
 const review=page.getByRole('region',{name:'Review graph action'});const deletion=page.getByLabel('Delete remote branch',{exact:true});await expect(deletion.locator('.recovery-ref')).toBeVisible({timeout:45000});const recovery=await deletion.locator('.recovery-ref').innerText();await expect(deletion).toContainText('Local branches, working files and staging stay in place.');expect(git(bare,['rev-parse','feature/manage'])).toBe(tip);
 await page.screenshot({path:testInfo.outputPath('delete-remote-branch-review.png')});await deletion.getByRole('button',{name:'Delete remote branch',exact:true}).click();await expect(review).toBeHidden({timeout:45000});expect(git(bare,['for-each-ref','--format=%(refname)','refs/heads/feature/manage'])).toBe('');expect(git(repo,['rev-parse','feature/manage'])).toBe(tip);expect(git(repo,['rev-parse',recovery])).toBe(tip);expect(git(repo,['diff','--','file.txt'])).toContain('uncommitted work stays');
 await page.locator(`button#commit-${tip}`).click({button:'right'});await page.getByRole('menuitem',{name:'Delete local branch spare…',exact:true}).click();await expect(panel.getByRole('button',{name:'Delete local branch',exact:true})).toBeEnabled({timeout:30000});const localRecovery=await panel.locator('.recovery-ref').innerText();await panel.getByRole('button',{name:'Delete local branch',exact:true}).click();await expect(review).toBeHidden({timeout:30000});expect(git(repo,['branch','--list','spare'])).toBe('');expect(git(repo,['rev-parse',localRecovery])).toBe(tip);expect(git(repo,['rev-parse','feature/manage'])).toBe(tip);expect(git(repo,['diff','--','file.txt'])).toContain('uncommitted work stays');
 // Checked-out local branch gets an explicit blocker instead of switching branches.
 await page.locator('[data-ref-name="refs/heads/main"]').click({button:'right'});await page.getByRole('menuitem',{name:'Delete local branch main…',exact:true}).click();await expect(panel.getByRole('button',{name:'Delete local branch',exact:true})).toBeDisabled();await expect(panel.locator('.notice').first()).toBeVisible();expect(git(repo,['branch','--show-current'])).toBe('main');
});

test('submodule background refresh stays quiet and manual refresh preserves rows on failure',async({page})=>{
 test.setTimeout(150000);const repo=join(folder,'quiet-modules-parent'),source=join(folder,'quiet-modules-source');for(const path of [repo,source]){mkdirSync(path);git(path,['init','-b','main']);git(path,['commit','--allow-empty','-m','Quiet refresh base']);}git(repo,['-c','protocol.file.allow=always','submodule','add',source,'module']);git(repo,['commit','-am','Record module']);
 await connectRealCore(page,join(folder,'quiet-modules.json'));await page.clock.install();
 await page.addInitScript(()=>{
  const w=window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args:unknown)=>Promise<unknown>};moduleReads:number;moduleCompleted:number;holdModules:boolean;modulesHeld:boolean;failModules:boolean;releaseModules?:()=>void};const original=w.__TAURI_INTERNALS__.invoke;w.moduleReads=0;w.moduleCompleted=0;w.holdModules=false;w.failModules=false;
  w.__TAURI_INTERNALS__.invoke=async(command,args)=>{if(command!=='submodule_snapshot')return original(command,args);w.moduleReads++;if(w.failModules)throw new Error('Test refresh unavailable');const value=await original(command,args);if(w.holdModules){w.modulesHeld=true;await new Promise<void>(resolve=>{w.releaseModules=resolve;});w.modulesHeld=false;}w.moduleCompleted++;return value;};
 });
 await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();const group=page.getByRole('region',{name:'Branch submodules',exact:true});await expect(group.locator('.branch-submodule-row')).toHaveCount(1);await expect(group).toContainText('Files inside are clean');await expect(group).not.toContainText('Reading branch submodules…');const height=(await group.boundingBox())!.height;
 const reads=await page.evaluate(()=>(window as unknown as {moduleReads:number}).moduleReads);
 // The former 15-second timer must not trigger recursive discovery.
 await page.clock.fastForward(16000);expect(await page.evaluate(()=>(window as unknown as {moduleReads:number}).moduleReads)).toBe(reads);
 await page.evaluate(()=>{(window as unknown as {holdModules:boolean}).holdModules=true;});await page.clock.fastForward(45000);await expect.poll(()=>page.evaluate(()=>(window as unknown as {modulesHeld:boolean}).modulesHeld)).toBe(true);await expect(group).not.toContainText('Reading branch submodules…');await expect(group.locator('.branch-submodule-row')).toHaveCount(1);expect((await group.boundingBox())!.height).toBe(height);
 await page.evaluate(()=>{const w=window as unknown as {holdModules:boolean;releaseModules?:()=>void};w.holdModules=false;w.releaseModules?.();});await expect.poll(()=>page.evaluate(()=>(window as unknown as {moduleCompleted:number}).moduleCompleted)).toBe(reads+1);
 // Focus/visibility events right after a read are coalesced.
 await page.evaluate(()=>{window.dispatchEvent(new Event('focus'));document.dispatchEvent(new Event('visibilitychange'));});expect(await page.evaluate(()=>(window as unknown as {moduleReads:number}).moduleReads)).toBe(reads+1);
 await page.evaluate(()=>{(window as unknown as {holdModules:boolean}).holdModules=true;});await group.getByRole('button',{name:'Refresh submodules',exact:true}).click();await expect.poll(()=>page.evaluate(()=>(window as unknown as {modulesHeld:boolean}).modulesHeld)).toBe(true);await expect(group.locator('.graph-submodules-heading .loading-spin')).toBeVisible();await expect(group).not.toContainText('Reading branch submodules…');expect((await group.boundingBox())!.height).toBe(height);
 await page.evaluate(()=>{const w=window as unknown as {holdModules:boolean;releaseModules?:()=>void};w.holdModules=false;w.releaseModules?.();});await expect(group.getByRole('button',{name:'Refresh submodules',exact:true})).toBeEnabled();
 await page.evaluate(()=>{(window as unknown as {failModules:boolean}).failModules=true;});await group.getByRole('button',{name:'Refresh submodules',exact:true}).click();await expect(group.getByRole('alert')).toContainText('Showing the last known state.');await expect(group.locator('.branch-submodule-row')).toHaveCount(1);
 writeFileSync(join(repo,'module','untracked.txt'),'Local child edit\n');await page.evaluate(()=>{(window as unknown as {failModules:boolean}).failModules=false;});await group.getByRole('button',{name:'Refresh submodules',exact:true}).click();await expect(group).toContainText('Uncommitted changes inside');await expect(group.getByRole('alert')).toHaveCount(0);
});

test('message-only Amend uses the existing composer and preserves the ordinary draft and unstaged work',async({page},testInfo)=>{
 test.setTimeout(120000);const repo=join(folder,'amend-message');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Original root']);const old=git(repo,['rev-parse','HEAD']),tree=git(repo,['rev-parse','HEAD^{tree}']);writeFileSync(join(repo,'file.txt'),'unstaged stays\n');
 await connectRealCore(page,join(folder,'amend-message.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();const panel=page.getByRole('complementary',{name:'Working changes',exact:true});await panel.getByLabel('Summary',{exact:true}).fill('My ordinary draft');await panel.getByRole('checkbox',{name:'Amend last commit',exact:true}).check();await expect(panel.getByLabel('Summary',{exact:true})).toHaveValue('Original root');await panel.getByRole('checkbox',{name:'Amend last commit',exact:true}).uncheck();await expect(panel.getByLabel('Summary',{exact:true})).toHaveValue('My ordinary draft');await panel.getByRole('checkbox',{name:'Amend last commit',exact:true}).check();await expect(panel.getByLabel('Summary',{exact:true})).toHaveValue('Original root');await panel.getByLabel('Summary',{exact:true}).fill('Clarify original root');await panel.getByRole('button',{name:'Review amend',exact:true}).click();await expect(panel).toContainText('Message-only amend; the file tree stays unchanged.');await expect(panel).toContainText(old.slice(0,8));const recovery=await panel.locator('.amend-review .recovery-ref').innerText();await expect(panel.getByRole('button',{name:'Amend reviewed commit',exact:true})).toBeInViewport({ratio:1});await page.screenshot({path:testInfo.outputPath('amend-message-review.png')});await panel.getByRole('button',{name:'Amend reviewed commit',exact:true}).click();await expect(panel).toContainText('Last commit amended; original retained for recovery');await expect(panel.getByRole('checkbox',{name:'Amend last commit',exact:true})).not.toBeChecked();await expect(panel.getByLabel('Summary',{exact:true})).toHaveValue('My ordinary draft');expect(git(repo,['rev-parse','HEAD'])).not.toBe(old);expect(git(repo,['rev-parse','HEAD^{tree}'])).toBe(tree);expect(git(repo,['rev-parse',recovery])).toBe(old);expect(git(repo,['log','-1','--format=%B'])).toBe('Clarify original root');expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('unstaged stays\n');
});

test('Amend of fetched history requires acknowledgment and publishes no unstaged work or force push',async({page},testInfo)=>{
 test.setTimeout(120000);const repo=join(folder,'amend-referenced'),bare=join(folder,'amend-referenced.git');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Original shared']);const old=git(repo,['rev-parse','HEAD']);git(folder,['init','--bare',bare]);git(repo,['remote','add','origin',bare]);git(repo,['push','-u','origin','main']);writeFileSync(join(repo,'file.txt'),'staged amendment\n');git(repo,['add','file.txt']);writeFileSync(join(repo,'file.txt'),'unstaged private edit\n');
 await connectRealCore(page,join(folder,'amend-referenced.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();const panel=page.getByRole('complementary',{name:'Working changes',exact:true});await panel.getByRole('checkbox',{name:'Amend last commit',exact:true}).check();await expect(panel.getByLabel('Summary',{exact:true})).toHaveValue('Original shared');await panel.getByRole('button',{name:'Review amend',exact:true}).click();await expect(panel).toContainText('refs/remotes/origin/main');await expect(panel.getByRole('button',{name:'Amend reviewed commit',exact:true})).toBeDisabled();await panel.getByRole('checkbox',{name:'I understand this rewrites referenced history.',exact:true}).check();await expect(panel.getByRole('button',{name:'Amend reviewed commit',exact:true})).toBeEnabled();await expect(panel.getByRole('button',{name:'Amend reviewed commit',exact:true})).toBeInViewport({ratio:1});await page.screenshot({path:testInfo.outputPath('amend-referenced-review.png')});await page.setViewportSize({width:780,height:560});await expect(panel.getByRole('button',{name:'Amend reviewed commit',exact:true})).toBeInViewport({ratio:1});await page.screenshot({path:testInfo.outputPath('amend-referenced-compact.png')});await panel.getByRole('button',{name:'Amend reviewed commit',exact:true}).click();await expect(panel).toContainText('Last commit amended; original retained for recovery');expect(git(repo,['show','HEAD:file.txt'])).toBe('staged amendment');expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('unstaged private edit\n');expect(git(bare,['rev-parse','main'])).toBe(old);expect(git(repo,['rev-parse','refs/remotes/origin/main'])).toBe(old);
});



test('Copy commit link directly uses global host settings and the exact historical SHA',async({page,context})=>{
 await context.grantPermissions(['clipboard-read','clipboard-write']);const repo=join(folder,'commit-links');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['commit','--allow-empty','-m','Link original']);const old=git(repo,['rev-parse','HEAD']);git(repo,['commit','--allow-empty','-m','Link latest']);const head=git(repo,['rev-parse','HEAD']);git(repo,['remote','add','origin','git@gitlab.com:team/sub/project.git']);
 await connectRealCore(page,join(folder,'commit-links.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator(`#commit-${old}`).click();await page.getByRole('button',{name:'Copy commit link',exact:true}).click();await expect(page.getByRole('button',{name:'Commit link copied',exact:true})).toBeVisible();expect(await page.evaluate(()=>navigator.clipboard.readText())).toBe(`https://gitlab.com/team/sub/project/-/commit/${old}`);
 git(repo,['remote','add','upstream','git@github.com:upstream/project.git']);git(repo,['update-ref','refs/remotes/upstream/main',head]);git(repo,['config','branch.main.remote','upstream']);git(repo,['config','branch.main.merge','refs/heads/main']);
 await page.locator(`#commit-${head}`).click();await page.getByRole('button',{name:'Copy commit link',exact:true}).click();await expect(page.getByRole('button',{name:'Commit link copied',exact:true})).toBeVisible();expect(await page.evaluate(()=>navigator.clipboard.readText())).toBe(`https://github.com/upstream/project/commit/${head}`);await expect(page.getByLabel('Link remote',{exact:true})).toHaveCount(0);
 git(repo,['remote','set-url','upstream','git@git.example.com:group/nested/project.git']);
 await page.evaluate(()=>{localStorage.setItem('gitnova.connections.v1',JSON.stringify([{id:'host',label:'Team',provider:'gitlab',origin:'https://git.example.com',enabled:false}]));localStorage.setItem('gitnova.commitLink.legacy.upstream',JSON.stringify({provider:'github',origin:'https://wrong.example.com'}));});
 await page.locator(`#commit-${old}`).click();await page.getByRole('button',{name:'Copy commit link',exact:true}).click();await expect(page.getByRole('button',{name:'Commit link copied',exact:true})).toBeVisible();expect(await page.evaluate(()=>navigator.clipboard.readText())).toBe(`https://git.example.com/group/nested/project/-/commit/${old}`);
 expect(git(repo,['rev-parse','HEAD'])).toBe(head);expect(git(repo,['status','--porcelain'])).toBe('');
});

test('FAQ is searchable in existing Settings in English and Thai',async({page},testInfo)=>{
 await connectRealCore(page,join(folder,'faq.json'));await page.goto('/');await page.getByRole('button',{name:'Settings',exact:true}).click();const dialog=page.locator('dialog.settings-dialog');
 await expect(dialog.getByRole('tab',{name:'General',exact:true})).toHaveAttribute('aria-selected','true');await expect(dialog.getByRole('searchbox')).toHaveCount(0);
 await dialog.getByRole('tab',{name:'General',exact:true}).focus();await page.keyboard.press('End');await expect(dialog.getByRole('tab',{name:'FAQ',exact:true})).toBeFocused();await expect(dialog.getByRole('tab',{name:'FAQ',exact:true})).toHaveAttribute('aria-selected','true');
 await dialog.getByRole('searchbox',{name:'Search the guide…'}).fill('submodule');await dialog.getByText('What is a submodule?',{exact:true}).click();await expect(dialog.getByRole('tabpanel',{name:'FAQ',exact:true})).toContainText('Each parent branch can record a different commit.');
 await dialog.getByRole('tab',{name:'General',exact:true}).click();await dialog.getByLabel('Language',{exact:true}).selectOption('th');await dialog.getByRole('tab',{name:'FAQ',exact:true}).click();await expect(dialog.getByRole('searchbox')).toHaveValue('submodule');await dialog.getByRole('searchbox').fill('เชอร์รี่ไม่มี');await expect(dialog.getByRole('status')).toContainText('ไม่พบหัวข้อ');await dialog.getByRole('searchbox').fill('cherry-pick');await expect(dialog.getByText('Cherry-pick คืออะไร ทำไมต้องเลือก parent?',{exact:true})).toBeVisible();
 await page.screenshot({path:testInfo.outputPath('faq-tab-thai.png')});await page.setViewportSize({width:390,height:640});await expect(dialog.getByRole('tab',{name:'FAQ',exact:true})).toBeInViewport();await page.screenshot({path:testInfo.outputPath('faq-tab-compact.png')});await dialog.getByRole('tab',{name:'FAQ',exact:true}).focus();await page.keyboard.press('Escape');await expect(dialog).not.toBeVisible();await expect(page.locator('.settings-trigger')).toBeFocused();
 await page.keyboard.press('Control+,');await expect(dialog.getByRole('tab',{name:'ทั่วไป',exact:true})).toHaveAttribute('aria-selected','true');
});



function mergeSubmoduleFixture(name:string,incomingPointer=false){
 const repo=join(folder,name),childSource=join(folder,`${name}-child`);mkdirSync(repo);mkdirSync(childSource);git(childSource,['init','-b','main']);const pointers:string[]=[];
 for(const value of ['base','next','current']){writeFileSync(join(childSource,'child.txt'),`${value}\n`);git(childSource,['add','.']);git(childSource,['commit','-m',value]);pointers.push(git(childSource,['rev-parse','HEAD']));}
 git(repo,['init','-b','release']);git(repo,['-c','protocol.file.allow=always','submodule','add',childSource,'_dags']);const child=join(repo,'_dags');git(child,['checkout','--detach',pointers[0]]);writeFileSync(join(repo,'base.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Base child pointer']);
 git(repo,['switch','-c','feature']);if(incomingPointer){git(child,['checkout','--detach',pointers[1]]);git(repo,['add','_dags']);}writeFileSync(join(repo,'feature.txt'),'incoming feature\n');git(repo,['add','feature.txt']);git(repo,['commit','-m','Feature for release']);const source=git(repo,['rev-parse','HEAD']);git(repo,['-c','submodule.recurse=false','switch','release']);git(child,['checkout','--detach',incomingPointer?pointers[0]:pointers[2]]);if(!incomingPointer){git(repo,['add','_dags']);git(repo,['commit','-m','Keep current child']);}
 return {repo,child,source,pointers,original:git(repo,['rev-parse','HEAD'])};
}

test('merge preview keeps the current submodule pointer and merges feature files without touching the child',async({page},testInfo)=>{
 test.setTimeout(150000);const f=mergeSubmoduleFixture('merge-keep-child');const indexPath=git(f.child,['rev-parse','--path-format=absolute','--git-path','index']);const index=readFileSync(indexPath);const childRefs=git(f.child,['show-ref']);await connectRealCore(page,join(folder,'merge-keep-child.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(f.repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator(`#commit-${f.source}`).click({button:'right'});await page.getByRole('menuitem',{name:'Merge into release…',exact:true}).click();const review=page.locator('.merge-start-review');await expect(review).toContainText('Keep current pointer');await expect(review.locator('.merge-submodule-evidence')).toContainText('_dags');await expect(review.locator('.merge-submodule-evidence')).toContainText(f.pointers[2].slice(0,12));await expect(review.locator('.merge-submodule-evidence')).toContainText(f.pointers[0].slice(0,12));await expect(review.getByRole('button',{name:'Start merge',exact:true})).toBeEnabled();await page.screenshot({path:testInfo.outputPath('merge-keeps-current-submodule.png')});expect(git(f.repo,['rev-parse','HEAD'])).toBe(f.original);expect(git(f.child,['rev-parse','HEAD'])).toBe(f.pointers[2]);
 await review.getByRole('button',{name:'Start merge',exact:true}).click();const inspector=page.getByLabel('Merge working changes',{exact:true});await expect(inspector).toContainText('Review merge result');await page.getByLabel('Merge message',{exact:true}).fill('Merge feature, keep current submodule');await inspector.getByRole('button',{name:'Review merge commit',exact:true}).click();await inspector.getByRole('button',{name:'Finish merge',exact:true}).click();await expect(page.locator('.working-row')).toContainText('Your working tree is clean',{timeout:45000});expect(git(f.repo,['show','-s','--format=%P','HEAD'])).toBe(`${f.original} ${f.source}`);expect(git(f.repo,['rev-parse','HEAD:_dags'])).toBe(f.pointers[2]);expect(git(f.child,['rev-parse','HEAD'])).toBe(f.pointers[2]);expect(readFileSync(indexPath)).toEqual(index);expect(git(f.child,['show-ref'])).toBe(childRefs);expect(readFileSync(join(f.repo,'feature.txt'),'utf8')).toBe('incoming feature\n');
});

test('merge preview names actual submodule changes and does not replace already-included history with a pointer warning',async({page},testInfo)=>{
 test.setTimeout(120000);const f=mergeSubmoduleFixture('merge-pointer-block',true);await connectRealCore(page,join(folder,'merge-pointer-block.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(f.repo);await page.getByRole('button',{name:'Open',exact:true}).click();const open=async()=>{await page.locator(`#commit-${f.source}`).click({button:'right'});await page.getByRole('menuitem',{name:'Merge into release…',exact:true}).click();};await open();const review=page.locator('.merge-start-review');await expect(review).toContainText('The predicted merge changes submodule pointers');await expect(review.locator('.merge-submodule-evidence')).toContainText('_dags');await expect(review.locator('.merge-submodule-evidence')).toContainText(f.pointers[0].slice(0,12));await expect(review.locator('.merge-submodule-evidence')).toContainText(f.pointers[1].slice(0,12));await expect(review.getByRole('button',{name:'Start merge',exact:true})).toBeDisabled();await page.screenshot({path:testInfo.outputPath('merge-submodule-result-blocked.png')});expect(git(f.child,['rev-parse','HEAD'])).toBe(f.pointers[0]);
 await page.keyboard.press('Escape');git(f.repo,['-c','submodule.recurse=false','merge','--no-ff','feature','-m','Externally included feature']);git(f.child,['checkout','--detach',f.pointers[2]]);git(f.repo,['add','_dags']);git(f.repo,['commit','-m','Keep current child after merge']);const included=git(f.repo,['rev-parse','HEAD']);await page.getByRole('button',{name:'Refresh',exact:true}).click();await expect(page.locator(`#commit-${included}`)).toBeVisible();await open();await expect(review).toContainText('The selected history is already included in this branch.');await expect(review.getByRole('button',{name:'Already included',exact:true})).toBeDisabled();await expect(review.locator('.merge-submodule-evidence')).toHaveCount(0);await expect(review).not.toContainText('pointers/layout');expect(git(f.repo,['rev-parse','HEAD'])).toBe(included);expect(git(f.child,['rev-parse','HEAD'])).toBe(f.pointers[2]);
});


test('Settings keeps global language and connections independent of repository selection and drafts',async({page},testInfo)=>{
 await connectRealCore(page,join(folder,'settings-ui.json'));await page.goto('/');
 const trigger=page.getByRole('button',{name:'Settings',exact:true});await trigger.click();const settings=page.locator('.settings-dialog');await expect(settings).not.toContainText('Connect once per company or Git host.');await settings.getByRole('tab',{name:'Connections',exact:true}).click();await expect(settings).toContainText('Connect once per company or Git host.');await settings.getByRole('tab',{name:'General',exact:true}).click();
 await page.keyboard.press('Escape');await expect(settings).toBeHidden();await expect(trigger).toBeFocused();
 await page.getByLabel('Add an existing repository').fill(repository);await page.getByRole('button',{name:'Open',exact:true}).click();const head=git(repository,['rev-parse','HEAD']);
 await page.locator(`#commit-${head}`).click();await expect(page.locator('.commit-inspector .pipeline-status')).toHaveCount(0);await expect(page.locator('.commit-inspector .jenkins-status')).toHaveCount(0);
 await page.locator('.working-row').click();await page.getByLabel('Summary',{exact:true}).fill('Keep settings draft');
 await page.keyboard.press('Control+,');await expect(settings).toBeVisible();await expect(settings).toContainText('Connections');await expect(settings.locator('.settings-scope')).toHaveCount(0);await expect(settings).not.toContainText(repository);await expect(settings).not.toContainText(head.slice(0,8));
 await settings.getByLabel('Language',{exact:true}).selectOption('th');await expect(page.locator('html')).toHaveAttribute('lang','th');await expect(settings).toBeVisible();await page.screenshot({path:testInfo.outputPath('central-settings-thai.png')});
 await page.keyboard.press('Escape');await expect(page.locator('.settings-dialog')).toBeHidden();await expect(page.getByLabel('สรุป',{exact:true})).toHaveValue('Keep settings draft');
 await page.keyboard.press('Control+,');await page.getByLabel('ภาษา',{exact:true}).selectOption('en');await page.keyboard.press('Escape');await expect(trigger).toBeFocused();await page.locator(`#commit-${head}`).click();await expect(page.locator(`#commit-${head}`)).toHaveAttribute('aria-pressed','true');
 await page.keyboard.press('Control+o');await expect(page.locator('#workspace-picker')).toBeVisible();await expect(page.locator('#workspace-picker').getByLabel('Language',{exact:true})).toHaveCount(0);await page.keyboard.press('Escape');
 await page.locator(`#commit-${head}`).click({button:'right'});await page.getByRole('menuitem',{name:'Create branch here…',exact:true}).click();await page.getByLabel('New branch name',{exact:true}).fill('feature/settings-draft');await page.keyboard.press('Control+,');await page.getByLabel('Language',{exact:true}).selectOption('th');await page.keyboard.press('Escape');await expect(page.locator('#graph-ref-name')).toHaveValue('feature/settings-draft');await page.keyboard.press('Escape');
 expect(git(repository,['rev-parse','HEAD'])).toBe(head);expect(git(repository,['status','--porcelain'])).toContain('pending.txt');
});

test('global company connections pair GitLab or GitHub with Jenkins without opening a repository',async({page},testInfo)=>{
 await connectRealCore(page,join(folder,'global-settings.json'));const hosts=new Map<string,string>(),jenkins=new Map<string,string>();const commands:unknown[]=[];
 await page.exposeBinding('globalAccounts',async(_source,command:string,args:{profile?:{provider:string;origin:string};controller?:string;username?:string})=>{
  commands.push({command,args});if(command==='host_account'||command==='connect_host'){
   const p=args.profile!;if(command==='connect_host')hosts.set(p.origin,p.provider==='gitlab'?'alice-a':'bob-b');
   return {selection:{remote:'',branch:'',oid:'',provider:p.provider,origin:p.origin},provider:p.provider,origin:new URL(p.origin).href.replace(/\/+$/,''),project:'',account:hosts.get(p.origin)??null};
  }
  if(command==='jenkins_account')return jenkins.get(args.controller!)??null;
  if(command==='connect_jenkins_account'){jenkins.set(args.controller!,args.username!);return args.username;}
  throw new Error(`Unexpected account command ${command}`);
 });
 await page.addInitScript(()=>{const w=window as unknown as {__TAURI_INTERNALS__:{invoke:(c:string,a:unknown)=>Promise<unknown>};globalAccounts:(c:string,a:unknown)=>Promise<unknown>};const original=w.__TAURI_INTERNALS__.invoke;w.__TAURI_INTERNALS__.invoke=(c,a)=>['host_account','connect_host','jenkins_account','connect_jenkins_account'].includes(c)?w.globalAccounts(c,a):original(c,a);});
 await page.goto('/');await page.getByRole('button',{name:'Settings',exact:true}).click();const settings=page.locator('.settings-dialog');await settings.getByRole('tab',{name:'Connections',exact:true}).click();
 for(const [label,provider,host,controller,user] of [['Company A','gitlab','https://git.a.example','https://ci.a.example','jenkins-a'],['Company B','github','https://git.b.example','https://ci.b.example','jenkins-b']]){
  await settings.getByRole('button',{name:'Add connection',exact:true}).click();await settings.getByLabel('Company or connection name').fill(label);await settings.getByLabel('Git provider',{exact:true}).selectOption(provider);await settings.getByLabel('Git host HTTPS address').fill(host);await settings.getByRole('button',{name:'Add connection',exact:true}).click();
  const card=settings.getByRole('article',{name:label,exact:true});await expect(card).toBeVisible();await card.getByRole('button',{name:'Connect',exact:true}).click();await expect(card).toContainText(provider==='gitlab'?'alice-a':'bob-b');
  await card.getByRole('button',{name:'Connect Jenkins…',exact:true}).click();await card.getByLabel('Jenkins controller URL').fill(controller);await card.getByLabel('Jenkins username').fill(user);await card.getByRole('button',{name:'Connect Jenkins account…',exact:true}).click();await expect(card).toContainText(user);
 }
 await expect(settings.locator('.settings-scope')).toHaveCount(0);expect(JSON.stringify(commands)).not.toContain('membershipId');expect(JSON.stringify(commands)).not.toContain('jobUrl');expect(JSON.stringify(commands)).not.toContain('oid');
 const stored=await page.evaluate(()=>JSON.parse(localStorage.getItem('gitnova.connections.v1')!));expect(stored).toHaveLength(2);expect(stored[0].jenkins.controller).toBe('https://ci.a.example');expect(stored[1].jenkins.controller).toBe('https://ci.b.example');
 await page.screenshot({path:testInfo.outputPath('global-company-connections.png')});await page.keyboard.press('Escape');await page.reload();await page.getByRole('button',{name:'Settings',exact:true}).click();await settings.getByRole('tab',{name:'Connections',exact:true}).click();await expect(settings.getByRole('article',{name:'Company A',exact:true})).toContainText('alice-a');await expect(settings.getByRole('article',{name:'Company B',exact:true})).toContainText('jenkins-b');
 await settings.getByRole('button',{name:'Remove connection Company A',exact:true}).click();await settings.getByRole('button',{name:'Remove connection',exact:true}).click();await settings.getByRole('tab',{name:'Connections',exact:true}).click();await expect(settings.getByRole('article',{name:'Company A',exact:true})).toHaveCount(0);await expect(settings.getByRole('article',{name:'Company B',exact:true})).toBeVisible();expect(hosts.has('https://git.a.example')).toBe(true);
});

test('global CI follows real fetch hosts across repositories, ignores late results and pauses while hidden',async({page},testInfo)=>{
 const repos:string[]=[],oids:string[]=[];for(const [name,url] of [['global-a-one','git@git.a.example:group/one.git'],['global-a-two','ssh://git@git.a.example/group/two.git'],['global-b','https://git.b.example/org/project.git']]){const repo=join(folder,name);mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['commit','--allow-empty','-m',name]);git(repo,['remote','add','origin',url]);repos.push(repo);oids.push(git(repo,['rev-parse','HEAD']));}
 await connectRealCore(page,join(folder,'global-ci-graph.json'));const queries:{origin:string;provider:string;oid:string}[]=[];let running=true,release:(()=>void)|undefined;
 await page.exposeBinding('globalCi',async(_source,args:{selection:{origin:string;provider:string;oid:string}})=>{const s=args.selection;queries.push(s);if(s.oid===oids[1])await new Promise<void>(r=>{release=r;});return {oid:s.oid,origin:s.origin,project:s.provider==='gitlab'?'group/project':'org/project',checkedAt:Math.floor(Date.now()/1000),partial:false,runs:[{id:s.provider==='gitlab'?'40778':s.oid,sha:s.oid,reference:'main',source:s.provider==='gitlab'?'gitlab_pipeline':'provider',status:s.provider==='github'?'failed':running?'running':'success',url:s.provider==='gitlab'?`${s.origin}/group/project/-/pipelines/40778`:`${s.origin}/project/commit/${s.oid}`,jobs:[],jobsPartial:false},...(s.provider==='github'?[{id:'99',displayNumber:1,name:'CI',sha:s.oid,reference:'main',source:'github_actions',status:'failed',url:`${s.origin}/org/project/actions/runs/99`,jobs:[],jobsPartial:false}]:[])]};});
 await page.addInitScript(()=>{localStorage.setItem('gitnova.connections.v1',JSON.stringify([{id:'a',label:'Company A',provider:'gitlab',origin:'https://git.a.example',enabled:true},{id:'b',label:'Company B',provider:'github',origin:'https://git.b.example',enabled:true}]));const w=window as unknown as {__TAURI_INTERNALS__:{invoke:(c:string,a:unknown)=>Promise<unknown>};globalCi:(a:unknown)=>Promise<unknown>};const original=w.__TAURI_INTERNALS__.invoke;w.__TAURI_INTERNALS__.invoke=(c,a)=>c==='ci_summary'?w.globalCi(a):original(c,a);});
 await page.clock.install();await page.goto('/');const open=async(repo:string,first=false)=>{if(!first)await page.keyboard.press('Control+o');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();};await open(repos[0],true);await page.clock.fastForward(1000);await expect(page.locator(`#commit-${oids[0]} .commit-ci`)).toContainText('Running · #40778');expect(queries[0].origin).toBe('https://git.a.example');
 await page.evaluate(()=>{Object.defineProperty(document,'hidden',{configurable:true,get:()=>true});document.dispatchEvent(new Event('visibilitychange'));});const count=queries.length;await page.clock.fastForward(65000);expect(queries.length).toBe(count);running=false;await page.evaluate(()=>{Object.defineProperty(document,'hidden',{configurable:true,get:()=>false});document.dispatchEvent(new Event('visibilitychange'));});await page.clock.fastForward(1500);await expect(page.locator(`#commit-${oids[0]} .commit-ci`)).toContainText('Passed · #40778');
 await expect(page.locator(`#commit-${oids[0]} .commit-ci`)).toHaveAttribute('title',/Pipeline #40778 · Passed · main/);
 await page.evaluate(()=>{const w=window as unknown as {__TAURI_INTERNALS__:{invoke:(c:string,a:unknown)=>Promise<unknown>};openedPipeline:unknown};const invoke=w.__TAURI_INTERNALS__.invoke;w.__TAURI_INTERNALS__.invoke=(c,a)=>{if(c==='open_ci_url'){w.openedPipeline=a;return Promise.resolve();}return invoke(c,a);};});
 await page.locator(`#commit-${oids[0]}`).focus();await page.keyboard.press('Shift+F10');await page.getByRole('menuitem',{name:'Open pipeline #40778',exact:true}).click();
 expect(await page.evaluate(()=>(window as unknown as {openedPipeline:unknown}).openedPipeline)).toEqual({url:'https://git.a.example/group/project/-/pipelines/40778'});
 await page.screenshot({path:testInfo.outputPath('gitlab-passed-pipeline-column.png')});
 await open(repos[1]);await page.clock.fastForward(1000);await expect.poll(()=>!!release).toBe(true);await open(repos[2]);await page.clock.fastForward(1000);release!();await expect(page.locator(`#commit-${oids[2]} .commit-ci`)).toContainText('Failed · #1');expect(queries.at(-1)?.provider).toBe('github');expect(queries.at(-1)?.origin).toBe('https://git.b.example');await expect(page.locator(`#commit-${oids[1]}`)).toHaveCount(0);await expect(page.locator(`#commit-${oids[2]} .commit-ci`)).toHaveAttribute('title',/CI #1 · Failed · main/);await page.locator(`#commit-${oids[2]}`).focus();await page.keyboard.press('Shift+F10');await page.getByRole('menuitem',{name:'Open workflow #1 · CI',exact:true}).click();expect(await page.evaluate(()=>(window as unknown as {openedPipeline:unknown}).openedPipeline)).toEqual({url:'https://git.b.example/org/project/actions/runs/99'});await page.screenshot({path:testInfo.outputPath('global-ci-github-column.png')});
 for(let i=0;i<repos.length;i++){expect(git(repos[i],['rev-parse','HEAD'])).toBe(oids[i]);expect(git(repos[i],['status','--porcelain'])).toBe('');}
});

test('a branch switch refreshes working state and graph once after the write',async({page})=>{
 const repo=join(folder,'switch-refresh-count');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'main\n');git(repo,['add','.']);git(repo,['commit','-m','main']);git(repo,['switch','-c','feature']);writeFileSync(join(repo,'file.txt'),'feature\n');git(repo,['commit','-am','feature']);git(repo,['switch','main']);
 await connectRealCore(page,join(folder,'switch-refresh-count.json'));
 await page.addInitScript(()=>{const w=window as unknown as {__TAURI_INTERNALS__:{invoke:(c:string,a:unknown)=>Promise<unknown>};refreshCalls:string[];pendingSnapshot:number;snapshotCompleted:number};w.refreshCalls=[];w.pendingSnapshot=0;w.snapshotCompleted=0;const original=w.__TAURI_INTERNALS__.invoke;w.__TAURI_INTERNALS__.invoke=async(c,a)=>{w.refreshCalls.push(c);if(c==='repository_snapshot')w.pendingSnapshot++;try{return await original(c,a);}finally{if(c==='repository_snapshot'){w.pendingSnapshot--;w.snapshotCompleted++;}}};});
 await page.clock.install();await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();const switcher=page.getByRole('button',{name:'Switch branch',exact:true});await expect(switcher).toContainText('main');await expect.poll(()=>page.evaluate(()=>(window as unknown as {snapshotCompleted:number}).snapshotCompleted)).toBeGreaterThan(0);
 await switcher.click();await page.getByRole('button',{name:'Select branch feature',exact:true}).click();await expect(page.getByRole('button',{name:'Switch to feature',exact:true})).toBeEnabled();await page.evaluate(()=>(window as unknown as {refreshCalls:string[]}).refreshCalls=[]);await page.getByRole('button',{name:'Switch to feature',exact:true}).click();await expect(page.locator('.branch-preview')).toHaveCount(0);await expect(switcher).toContainText('feature');const calls=await page.evaluate(()=>(window as unknown as {refreshCalls:string[]}).refreshCalls);
 expect(calls.filter(c=>c==='repository_snapshot')).toHaveLength(1);expect(calls.filter(c=>c==='graph_page')).toHaveLength(1);expect(git(repo,['branch','--show-current'])).toBe('feature');expect(git(repo,['status','--porcelain'])).toBe('');
});

test('merge cherry-pick keeps unrelated clean submodule checkout and makes the parent baseline explicit', async ({page}, testInfo) => {
  test.setTimeout(180000);
  const f = mergeSubmoduleFixture('pick-preserved-child', true);
  git(f.repo, ['switch', '-c', 'main']);
  git(f.repo, ['switch', 'feature']); git(f.repo, ['switch', '-c', 'source-base']);
  git(f.repo, ['merge', '--no-ff', 'release', '-m', 'Include release base']);
  git(f.repo, ['switch', '-c', 'sql-feature']);
  writeFileSync(join(f.repo, 'query.sql'), 'select 1;\n'); git(f.repo, ['add', 'query.sql']); git(f.repo, ['commit', '-m', 'SQL-only addition']);
  git(f.repo, ['switch', 'source-base']); git(f.repo, ['merge', '--no-ff', 'sql-feature', '-m', 'Selected SQL merge']);
  const source = git(f.repo, ['rev-parse', 'HEAD']), baseline = git(f.repo, ['rev-parse', 'HEAD^1']);
  git(f.repo, ['-c', 'submodule.recurse=false', 'switch', 'main']);
  git(f.child, ['checkout', '--detach', f.pointers[1]]);
  const indexPath = git(f.child, ['rev-parse', '--path-format=absolute', '--git-path', 'index']);
  const bytes = readFileSync(indexPath), refs = git(f.child, ['show-ref']);
  await connectRealCore(page, join(folder, 'pick-preserved-child.json')); await page.goto('/');
  await page.getByLabel('Add an existing repository').fill(f.repo); await page.getByRole('button', {name:'Open',exact:true}).click();
  await page.locator(`#commit-${source}`).click({button:'right'});
  await page.getByRole('menuitem', {name:'Cherry-pick onto main…',exact:true}).click();
  const review = page.locator('.merge-start-review'), start = review.getByRole('button',{name:'Start cherry-pick',exact:true});
  await expect(review).toContainText('Select the mainline parent'); await expect(start).toBeDisabled();
  await review.getByLabel('Mainline parent · change baseline',{exact:true}).selectOption('1');
  await expect(start).toBeEnabled({timeout:45000});
  await expect(review).toContainText(`${baseline.slice(0,12)} → ${source.slice(0,12)}`);
  await expect(review).toContainText('Keep existing submodule checkouts');
  await expect(review).toContainText('query.sql');
  await review.getByLabel('Mainline parent · change baseline',{exact:true}).selectOption('');
  await expect(start).toBeDisabled();
  await review.getByLabel('Mainline parent · change baseline',{exact:true}).selectOption('1');
  await expect(start).toBeEnabled({timeout:45000});
  await page.screenshot({path:testInfo.outputPath('cherry-pick-preserved-submodule.png')});
  await start.click(); const inspector = page.getByLabel('Cherry-pick working changes',{exact:true}); await expect(inspector).toBeVisible({timeout:45000});
  await inspector.getByRole('button',{name:'Review cherry-pick commit',exact:true}).click();
  await inspector.getByRole('button',{name:'Finish cherry-pick',exact:true}).click();
  await expect(inspector).toHaveCount(0,{timeout:45000});
  expect(git(f.repo,['show','-s','--format=%P','HEAD'])).toBe(f.original);
  expect(git(f.repo,['rev-parse','HEAD:_dags'])).toBe(f.pointers[0]);
  expect(git(f.child,['rev-parse','HEAD'])).toBe(f.pointers[1]);
  expect(readFileSync(indexPath)).toEqual(bytes); expect(git(f.child,['show-ref'])).toBe(refs);
  expect(readFileSync(join(f.repo,'query.sql'),'utf8')).toBe('select 1;\n');
  expect(existsSync(join(f.repo,'feature.txt'))).toBe(false);
});

function mergeCleanupFixture(name: string) {
  const repo=join(folder,name);mkdirSync(repo);git(repo,['init','-b','main']);
  writeFileSync(join(repo,'base.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Cleanup base']);
  git(repo,['switch','-c','source']);writeFileSync(join(repo,'source.txt'),'source\n');git(repo,['add','.']);git(repo,['commit','-m','Cleanup source']);const source=git(repo,['rev-parse','HEAD']);
  git(repo,['update-ref','refs/remotes/origin/source',source]);git(repo,['switch','main']);
  writeFileSync(join(repo,'main.txt'),'main\n');git(repo,['add','.']);git(repo,['commit','-m','Destination work']);
  return {repo,source,original:git(repo,['rev-parse','HEAD'])};
}
test('merge checkbox opts into local source deletion only after reviewed finish',async({page},testInfo)=>{
  test.setTimeout(150000);const f=mergeCleanupFixture('cleanup-ui');await connectRealCore(page,join(folder,'cleanup-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(f.repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  await page.locator(`#commit-${f.source}`).click({button:'right'});await page.getByRole('menuitem',{name:'Merge into main…',exact:true}).click();
  const review=page.locator('.merge-start-review'),choice=review.getByRole('checkbox',{name:'Delete local source branch after merge: source',exact:true});await expect(choice).not.toBeChecked();await choice.check();await expect(review.getByRole('button',{name:'Start merge',exact:true})).toBeEnabled({timeout:45000});
  await page.screenshot({path:testInfo.outputPath('merge-delete-source-choice.png')});await review.getByRole('button',{name:'Start merge',exact:true}).click();const inspector=page.getByLabel('Merge working changes',{exact:true});await expect(inspector).toBeVisible({timeout:45000});expect(git(f.repo,['rev-parse','source'])).toBe(f.source);
  await inspector.getByRole('button',{name:'Review merge commit',exact:true}).click();await expect(inspector).toContainText('After this merge succeeds, delete local branch source. Remote branches stay unchanged.');await inspector.getByRole('button',{name:'Finish merge',exact:true}).click();await expect(inspector).toHaveCount(0,{timeout:45000});
  expect(git(f.repo,['for-each-ref','--format=%(refname)','refs/heads/source'])).toBe('');expect(git(f.repo,['rev-parse','refs/remotes/origin/source'])).toBe(f.source);expect(git(f.repo,['show','-s','--format=%P','HEAD'])).toBe(`${f.original} ${f.source}`);
});
test('merge cleanup preserves a moved source branch and shows a separate warning after merge success',async({page},testInfo)=>{
  test.setTimeout(150000);const f=mergeCleanupFixture('cleanup-moved-ui');await connectRealCore(page,join(folder,'cleanup-moved-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(f.repo);await page.getByRole('button',{name:'Open',exact:true}).click();
  await page.locator(`#commit-${f.source}`).click({button:'right'});await page.getByRole('menuitem',{name:'Merge into main…',exact:true}).click();const review=page.locator('.merge-start-review');await review.getByRole('checkbox',{name:'Delete local source branch after merge: source',exact:true}).check();await expect(review.getByRole('button',{name:'Start merge',exact:true})).toBeEnabled({timeout:45000});await review.getByRole('button',{name:'Start merge',exact:true}).click();
  const inspector=page.getByLabel('Merge working changes',{exact:true});await expect(inspector).toBeVisible({timeout:45000});const newer=git(f.repo,['commit-tree',`${f.source}^{tree}`,'-p',f.source,'-m','Unmerged new source commit']);git(f.repo,['update-ref','refs/heads/source',newer,f.source]);
  await inspector.getByRole('button',{name:'Review merge commit',exact:true}).click();await inspector.getByRole('button',{name:'Finish merge',exact:true}).click();await expect(inspector).toHaveCount(0,{timeout:45000});await expect(page.getByText('Merge succeeded. Inspect cleanup of source:',{exact:false})).toBeVisible();await page.screenshot({path:testInfo.outputPath('merge-success-source-kept.png')});expect(git(f.repo,['rev-parse','source'])).toBe(newer);expect(git(f.repo,['show','-s','--format=%P','HEAD'])).toBe(`${f.original} ${f.source}`);
});

test('first project publishes 383 files through Stage all, bounded commit review and Push remote setup',async({page},testInfo)=>{
 test.setTimeout(180000);const repo=join(folder,'first-publication'),bare=join(folder,'first-publication.git');mkdirSync(repo);mkdirSync(bare);git(repo,['init','-b','main']);git(bare,['init','--bare']);
 for(const [key,value] of [['user.name','GitNova Fixture'],['user.email','fixture@example.invalid'],['commit.gpgsign','false'],['core.hooksPath','.git/hooks'],['core.attributesFile','.git/no-attributes']])git(repo,['config',key,value]);
 mkdirSync(join(repo,'src'));writeFileSync(join(repo,'.gitignore'),'ignored.txt\n');writeFileSync(join(repo,'ignored.txt'),'not published\n');
 for(let i=0;i<382;i++)writeFileSync(join(repo,'src',`module-${String(i).padStart(3,'0')}.txt`),`module ${i}\n${'fixture line\n'.repeat(600)}`);
 await connectRealCore(page,join(folder,'first-publication-state.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
 await page.locator('.working-row').click();const working=page.getByLabel('Working changes',{exact:true});await expect(working).toContainText('383 changed files');await expect(working.getByRole('button',{name:'Stage all changes…',exact:true})).toBeVisible();await page.screenshot({path:testInfo.outputPath('stage-all-visible-in-heading.png')});await working.getByRole('button',{name:'Stage all changes…',exact:true}).click();const staging=working.getByLabel('Review Stage all',{exact:true});await expect(staging).toContainText('383 files will be staged.');expect(existsSync(join(repo,'.git/index'))).toBe(false);await page.screenshot({path:testInfo.outputPath('first-project-stage-all.png')});await staging.getByRole('button',{name:'Stage 383 files',exact:true}).click();await expect(staging).toHaveCount(0,{timeout:45000});
 await working.getByLabel('Summary',{exact:true}).fill('Initial public project');await working.getByRole('button',{name:'Review commit',exact:true}).click();await expect(working).toContainText('The complete staged file list is selected.');const visibleFiles=working.locator('.staged-group .changed-file');expect(await visibleFiles.count()).toBeLessThan(40);await visibleFiles.first().focus();await page.keyboard.press('End');await expect(visibleFiles.filter({hasText:'src/module-381.txt'})).toBeFocused();await page.keyboard.press('Home');await expect(visibleFiles.first()).toBeFocused();await page.keyboard.press('End');await visibleFiles.filter({hasText:'src/module-381.txt'}).click();await expect(page.getByLabel('Staged commit preview',{exact:true})).toContainText('module 381');await page.screenshot({path:testInfo.outputPath('first-project-complete-manifest.png')});await working.getByRole('button',{name:'Commit staged changes',exact:true}).click();await expect(page.locator('.push-current')).toBeEnabled({timeout:45000});expect(git(repo,['ls-tree','-r','--name-only','HEAD']).split('\n')).toHaveLength(383);
 await page.locator('.push-current').click();await page.getByRole('button',{name:'Add remote…',exact:true}).click();const setup=page.getByLabel('Add remote',{exact:true});await setup.getByLabel('Repository URL or local path',{exact:true}).fill(bare);await setup.getByRole('button',{name:'Review remote',exact:true}).click();await expect(setup.getByLabel('Review remote setup',{exact:true})).toContainText('origin');expect(git(repo,['remote'])).toBe('');await page.screenshot({path:testInfo.outputPath('first-project-add-remote.png')});await setup.getByRole('button',{name:'Add remote & continue',exact:true}).click();await expect(setup).toHaveCount(0);
 await page.getByLabel('Remote',{exact:true}).selectOption('origin');await page.getByLabel('Set this destination as upstream after push',{exact:true}).check();await page.getByRole('button',{name:'Check destination & review',exact:true}).click();await expect(page.getByText('First publication to this branch',{exact:false})).toBeVisible();await page.getByRole('button',{name:'Push reviewed branch',exact:true}).click();await expect(page.locator('.graph-actions')).toHaveCount(0,{timeout:45000});expect(git(bare,['rev-parse','main'])).toBe(git(repo,['rev-parse','HEAD']));expect(git(repo,['rev-parse','--abbrev-ref','main@{upstream}'])).toBe('origin/main');expect(git(repo,['show','-s','--format=%ae','HEAD'])).toBe('fixture@example.invalid');
});
test('Stage all refuses changed file contents and leaves an unborn index untouched',async({page})=>{
 test.setTimeout(90000);const repo=join(folder,'stage-all-stale');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'a.txt'),'before\n');writeFileSync(join(repo,'b.txt'),'other\n');await connectRealCore(page,join(folder,'stage-all-stale.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();const working=page.getByLabel('Working changes',{exact:true});await working.getByRole('button',{name:'Stage all changes…',exact:true}).click();const review=working.getByLabel('Review Stage all',{exact:true});await expect(review.getByRole('button',{name:'Stage 2 files',exact:true})).toBeEnabled();writeFileSync(join(repo,'a.txt'),'after\n');await review.getByRole('button',{name:'Stage 2 files',exact:true}).click();await expect(review.getByRole('alert')).toContainText('Stage all selection changed. Review again.');expect(existsSync(join(repo,'.git/index'))).toBe(false);expect(readFileSync(join(repo,'a.txt'),'utf8')).toBe('after\n');
});

async function openBlockConflict(page:Page,name:string) {
 const repo=join(folder,name);mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['config','user.name','Conflict Fixture']);git(repo,['config','user.email','conflict@example.invalid']);git(repo,['config','commit.gpgsign','false']);
 const base=Array.from({length:30},(_,i)=>`unchanged line ${i}`);writeFileSync(join(repo,'plan.txt'),base.join('\n')+'\n');git(repo,['add','.']);git(repo,['commit','-m','Plan base']);git(repo,['switch','-c','incoming']);
 const incoming=[...base];incoming[2]='incoming first';incoming[24]='incoming second';writeFileSync(join(repo,'plan.txt'),incoming.join('\n')+'\n');git(repo,['commit','-am','Incoming plan blocks']);const source=git(repo,['rev-parse','HEAD']);git(repo,['switch','main']);
 const current=[...base];current[2]='current first';current[24]='current second';writeFileSync(join(repo,'plan.txt'),current.join('\n')+'\n');git(repo,['commit','-am','Destination plan']);git(repo,['config','merge.conflictStyle','diff3']);const original=git(repo,['rev-parse','HEAD']);
 await connectRealCore(page,join(folder,`${name}.json`));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator(`#commit-${source}`).click({button:'right'});await page.getByRole('menuitem',{name:'Cherry-pick onto main…',exact:true}).click();await expect(page.getByRole('button',{name:'Start cherry-pick',exact:true})).toBeEnabled({timeout:45000});await page.getByRole('button',{name:'Start cherry-pick',exact:true}).click();
 const inspector=page.getByLabel('Cherry-pick working changes',{exact:true});await expect(inspector).toContainText('1 unresolved conflicts',{timeout:45000});await expect(page.getByLabel('Incoming conflict version',{exact:true})).toContainText('incoming first');return {repo,source,original,inspector};
}
test('cherry-pick diff3 conflict blocks resolve current and both without writing until review and apply',async({page},testInfo)=>{
 test.setTimeout(150000);const f=await openBlockConflict(page,'block-resolution');const before=readFileSync(join(f.repo,'plan.txt'),'utf8'),index=readFileSync(join(f.repo,'.git/index'));await page.getByRole('button',{name:'Edit result',exact:true}).click();const resolver=page.getByLabel('Conflict resolution editor',{exact:true});await expect(resolver).toContainText('Conflict 1 of 2');
 await expect(page.getByRole('button',{name:'Review edited result',exact:true})).toBeDisabled();const draft=page.getByRole('textbox',{name:'Edit merge result',exact:true});await draft.fill('<<<<<<<<< HEAD\nincomplete draft\n');await expect(resolver).toContainText('Incomplete conflict markers remain.');await expect(page.getByRole('button',{name:'Review edited result',exact:true})).toBeDisabled();await draft.fill(before);
 // Choosing directly from read-only inspection starts a draft. Revisit and undo
 // choices before review; no checkbox writes working content or staging.
 await resolver.getByRole('checkbox',{name:'Include current block',exact:true}).check();
 await resolver.getByRole('checkbox',{name:'Include incoming block',exact:true}).check();
 await resolver.getByRole('checkbox',{name:'Include incoming block',exact:true}).uncheck();
 await resolver.getByRole('button',{name:'Next conflict',exact:true}).click();await expect(resolver).toContainText('Conflict 2 of 2');await expect(page.getByLabel('Incoming conflict version',{exact:true})).toContainText('incoming second');
 await resolver.getByRole('checkbox',{name:'Include current block',exact:true}).check();await resolver.getByRole('checkbox',{name:'Include incoming block',exact:true}).check();await expect(resolver).toContainText('0 unresolved blocks');
 await resolver.getByRole('button',{name:'Previous conflict',exact:true}).click();await expect(resolver.getByRole('checkbox',{name:'Include current block',exact:true})).toBeChecked();await expect(resolver.getByRole('checkbox',{name:'Include incoming block',exact:true})).not.toBeChecked();
 await resolver.getByRole('checkbox',{name:'Include current block',exact:true}).uncheck();await expect(resolver).toContainText('1 unresolved blocks');await expect(resolver.getByRole('button',{name:'Review edited result',exact:true})).toBeDisabled();await resolver.getByRole('checkbox',{name:'Include current block',exact:true}).check();
 const editor=page.getByRole('textbox',{name:'Edit merge result',exact:true});await expect(editor).toHaveValue(/current first/);await expect(editor).toHaveValue(/current second\nincoming second/);await expect(editor).not.toHaveValue(/incoming first|<<<<<<<|=======|\|\|\|\|\|\|\|/);expect(readFileSync(join(f.repo,'plan.txt'),'utf8')).toBe(before);expect(readFileSync(join(f.repo,'.git/index'))).toEqual(index);
 await page.screenshot({path:testInfo.outputPath('three-way-conflict-blocks.png')});await page.getByRole('button',{name:'Review edited result',exact:true}).click();await expect(f.inspector.getByRole('button',{name:'Apply resolution',exact:true})).toBeEnabled();expect(readFileSync(join(f.repo,'plan.txt'),'utf8')).toBe(before);await f.inspector.getByRole('button',{name:'Apply resolution',exact:true}).click();await expect(f.inspector).toContainText('All conflicts resolved.',{timeout:45000});expect(git(f.repo,['show',':plan.txt'])).toContain('current second\nincoming second');await f.inspector.getByRole('button',{name:'Review cherry-pick commit',exact:true}).click();await f.inspector.getByRole('button',{name:'Finish cherry-pick',exact:true}).click();await expect(f.inspector).toHaveCount(0,{timeout:45000});expect(git(f.repo,['show','-s','--format=%P','HEAD'])).toBe(f.original);expect(git(f.repo,['ls-files','--unmerged'])).toBe('');
});
test('conflict choices open a draft directly and survive language changes without touching Git',async({page},testInfo)=>{
 test.setTimeout(120000);const f=await openBlockConflict(page,'block-direct');const before=readFileSync(join(f.repo,'plan.txt'),'utf8'),index=readFileSync(join(f.repo,'.git/index'));const resolver=page.getByLabel('Conflict resolution editor',{exact:true});
 await expect(page.getByRole('textbox',{name:'Edit merge result',exact:true})).toHaveAttribute('readonly','');await resolver.getByRole('checkbox',{name:'Include incoming block',exact:true}).check();await expect(page.getByRole('textbox',{name:'Edit merge result',exact:true})).not.toHaveAttribute('readonly','');await expect(resolver.getByRole('checkbox',{name:'Include incoming block',exact:true})).toBeChecked();
 await page.evaluate(()=>window.dispatchEvent(new StorageEvent('storage',{key:'gitnova.locale',newValue:'th'})));await expect(page.getByRole('checkbox',{name:'ใช้ข้อความฝั่งที่นำเข้าในจุดนี้',exact:true})).toBeChecked();await expect(page.locator('html')).toHaveAttribute('lang','th');await page.evaluate(()=>window.dispatchEvent(new StorageEvent('storage',{key:'gitnova.locale',newValue:'en'})));
 // Assert the original bytes after direct selection. A later manual edit becomes
 // the new draft; applying an old first-block choice must not overwrite it.
 const draft=page.getByRole('textbox',{name:'Edit merge result',exact:true});const value=await draft.inputValue();await f.inspector.getByRole('button',{name:'Back to diff',exact:true}).click();await expect(draft).toHaveValue(value);await page.getByRole('button',{name:'Edit result',exact:true}).click();await expect(draft).toHaveValue(value);await draft.fill('manual prefix\n'+value);await resolver.getByRole('checkbox',{name:'Include current block',exact:true}).check();await expect(draft).toHaveValue(/manual prefix/);await expect(draft).toHaveValue(/incoming first/);
 expect(readFileSync(join(f.repo,'plan.txt'),'utf8')).toBe(before);expect(readFileSync(join(f.repo,'.git/index'))).toEqual(index);expect(git(f.repo,['rev-parse','HEAD'])).toBe(f.original);await page.setViewportSize({width:900,height:760});const separation=await f.inspector.evaluate(el=>({files:el.querySelector('.working-files')!.getBoundingClientRect().bottom,resolution:el.querySelector('.merge-resolution')!.getBoundingClientRect().top}));expect(separation.resolution).toBeGreaterThanOrEqual(separation.files-1);await page.getByRole('separator',{name:'Resize diff area',exact:true}).focus();await page.keyboard.press('End');await page.screenshot({path:testInfo.outputPath('conflict-choice-draft-narrow.png')});
});
test('revert conflict block choices show the pre-commit side and stage only the reviewed draft',async({page})=>{
 test.setTimeout(150000);const repo=join(folder,'revert-block-choices');mkdirSync(repo);git(repo,['init','-b','main']);const base=Array.from({length:30},(_,i)=>`original line ${i}`);writeFileSync(join(repo,'plan.txt'),base.join('\n')+'\n');git(repo,['add','.']);git(repo,['commit','-m','Original plan']);const change=[...base];change[2]='change first';change[24]='change second';writeFileSync(join(repo,'plan.txt'),change.join('\n')+'\n');git(repo,['commit','-am','Change to revert']);const source=git(repo,['rev-parse','HEAD']);change[2]='later first';change[24]='later second';writeFileSync(join(repo,'plan.txt'),change.join('\n')+'\n');git(repo,['commit','-am','Later plan']);const tip=git(repo,['rev-parse','HEAD']);
 await connectRealCore(page,join(folder,'revert-block-choices.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator(`#commit-${source}`).click({button:'right'});await page.getByRole('menuitem',{name:'Revert on main…',exact:true}).click();await page.getByRole('button',{name:'Start revert',exact:true}).click();const panel=page.getByLabel('Revert working changes',{exact:true});await expect(panel).toContainText('1 unresolved conflicts',{timeout:45000});const resolver=page.getByLabel('Conflict resolution editor',{exact:true});await expect(resolver).toContainText('Pre-commit version');const before=readFileSync(join(repo,'plan.txt'),'utf8'),index=readFileSync(join(repo,'.git/index'));await resolver.getByRole('checkbox',{name:'Include incoming block',exact:true}).check();await resolver.getByRole('button',{name:'Next conflict',exact:true}).click();await resolver.getByRole('checkbox',{name:'Include incoming block',exact:true}).check();await expect(page.getByRole('textbox',{name:'Edit merge result',exact:true})).toHaveValue(base.join('\n')+'\n');expect(readFileSync(join(repo,'plan.txt'),'utf8')).toBe(before);expect(readFileSync(join(repo,'.git/index'))).toEqual(index);await resolver.getByRole('button',{name:'Review edited result',exact:true}).click();await panel.getByRole('button',{name:'Apply resolution',exact:true}).click();await expect(panel).toContainText('All conflicts resolved.');await panel.getByRole('button',{name:'Review revert commit',exact:true}).click();await panel.getByRole('button',{name:'Finish revert',exact:true}).click();await expect(panel).toHaveCount(0,{timeout:45000});expect(git(repo,['rev-parse','HEAD^'])).toBe(tip);expect(git(repo,['status','--porcelain'])).toBe('');expect(readFileSync(join(repo,'plan.txt'),'utf8')).toBe(base.join('\n')+'\n');
});
test('conflict editor refuses external file changes while keeping the unsaved draft visible',async({page})=>{
 test.setTimeout(120000);const f=await openBlockConflict(page,'block-external');await page.getByRole('button',{name:'Edit result',exact:true}).click();const editor=page.getByRole('textbox',{name:'Edit merge result',exact:true});await editor.fill('my unsaved resolution\n');const index=readFileSync(join(f.repo,'.git/index'));writeFileSync(join(f.repo,'plan.txt'),'external resolution\n');await page.getByRole('button',{name:'Review edited result',exact:true}).click();await expect(f.inspector.getByRole('alert')).toContainText('Conflict content changed outside this editor.');await expect(editor).toHaveValue('my unsaved resolution\n');expect(readFileSync(join(f.repo,'plan.txt'),'utf8')).toBe('external resolution\n');expect(readFileSync(join(f.repo,'.git/index'))).toEqual(index);expect(git(f.repo,['rev-parse','HEAD'])).toBe(f.original);
});


test('First Push uses connected account creation, requires public confirmation, then adds only the verified remote',async({page},testInfo)=>{
 test.setTimeout(90000);const repo=join(folder,'publish-from-push');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['commit','--allow-empty','-m','Publish initial']);const head=git(repo,['rev-parse','HEAD']);await connectRealCore(page,join(folder,'publish-from-push.json'));let creates=0;
 await page.exposeBinding('publicationProvider',async(_source,c:string,a:any)=>{
  if(c==='repository_create_options')return {account:'alice',namespaces:[{id:'user:alice',path:'alice'},{id:'org:team',path:'team'}],partial:false};
  if(c==='preview_repository_create')return {request:a.request,account:'alice',project:`alice/${a.request.name}`,cloneUrl:`https://github.example/alice/${a.request.name}.git`,token:'review-provider',created:null};
  if(c==='create_repository'){creates++;return {receipt:{id:'created-fixture',kind:'create_hosted_repository',status:'succeeded',summary:'Empty hosted repository created and verified; no code pushed',headBefore:head,headAfter:head},repository:{project:`alice/${a.request.name}`,cloneUrl:`https://github.example/alice/${a.request.name}.git`,webUrl:`https://github.example/alice/${a.request.name}`,visibility:a.request.visibility}};}
  return null;
 });
 await page.addInitScript(()=>{localStorage.setItem('gitnova.connections.v1',JSON.stringify([{id:'personal',label:'Personal GitHub',provider:'github',origin:'https://github.example',enabled:false}]));const w=window as unknown as {publicationProvider:(c:string,a:unknown)=>Promise<unknown>;__TAURI_INTERNALS__:{invoke:(c:string,a:any)=>Promise<unknown>}};const original=w.__TAURI_INTERNALS__.invoke;w.__TAURI_INTERNALS__.invoke=(c,a)=>c==='ci_connection'?Promise.resolve(null):['repository_create_options','preview_repository_create','create_repository'].includes(c)?w.publicationProvider(c,a):c==='host_account'?Promise.resolve({account:'alice',provider:a.profile.provider,origin:a.profile.origin}):original(c,a);});
 await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.getByRole('button',{name:'Settings',exact:true}).click();const settings=page.getByRole('dialog',{name:'Settings',exact:true});await expect(settings.getByRole('heading',{name:'Repository remotes',exact:true})).toHaveCount(0);await settings.getByRole('button',{name:'Close settings',exact:true}).click();
 await page.locator('.push-current').click();await expect(page.getByText('No remote is configured for this repository. Add a destination before pushing?',{exact:true})).toBeVisible();await page.getByRole('button',{name:'Add remote…',exact:true}).click();const setup=page.getByLabel('Add remote',{exact:true});await setup.getByRole('button',{name:'Manage connections in Settings…',exact:true}).click();await expect(settings).toBeVisible();await expect(settings.getByRole('heading',{name:'Repository remotes',exact:true})).toHaveCount(0);await settings.getByRole('button',{name:'Close settings',exact:true}).click();await expect(setup.getByRole('button',{name:'Manage connections in Settings…',exact:true})).toBeFocused();await setup.getByRole('button',{name:'Create new repository',exact:true}).click();await setup.getByLabel('Connection / account',{exact:true}).selectOption('personal');await expect(setup.getByLabel('Owner / namespace',{exact:true})).toHaveValue('user:alice');await expect(setup.getByLabel('Access',{exact:true})).toHaveValue('private');await setup.getByLabel('Repository name',{exact:true}).fill('project-alpha');await setup.getByLabel('Description',{exact:true}).fill('Original project description');await setup.getByLabel('Access',{exact:true}).selectOption('public');await setup.getByRole('button',{name:'Review repository creation',exact:true}).click();await expect(setup).toContainText('https://github.example/alice/project-alpha.git');expect(creates).toBe(0);expect(git(repo,['remote'])).toBe('');await expect(setup.getByRole('button',{name:'Create empty repository',exact:true})).toBeDisabled();await setup.getByRole('checkbox',{name:'I understand this repository will be public.',exact:true}).check();await page.screenshot({path:testInfo.outputPath('first-push-create-review.png')});await setup.getByRole('button',{name:'Create empty repository',exact:true}).click();await expect(setup).toContainText('Empty repository created.');expect(creates).toBe(1);expect(git(repo,['remote'])).toBe('');
 await setup.getByRole('button',{name:'Review remote',exact:true}).click();await expect(setup.getByLabel('Review remote setup',{exact:true})).toBeVisible();git(repo,['config','fixture.external','changed']);await setup.getByRole('button',{name:'Add remote & continue',exact:true}).click();await expect(setup.getByRole('alert')).toBeVisible();await expect(setup).toContainText('https://github.example/alice/project-alpha.git');expect(creates).toBe(1);expect(git(repo,['remote'])).toBe('');await setup.getByRole('button',{name:'Review remote',exact:true}).click();await setup.getByRole('button',{name:'Add remote & continue',exact:true}).click();await expect(setup).toHaveCount(0);await expect(page.getByLabel('Remote',{exact:true})).toHaveValue('origin');await expect(page.getByRole('checkbox',{name:'Set this destination as upstream after push',exact:true})).toBeChecked();expect(git(repo,['config','remote.origin.url'])).toBe('https://github.example/alice/project-alpha.git');expect(git(repo,['rev-parse','HEAD'])).toBe(head);expect(existsSync(join(repo,'.git/FETCH_HEAD'))).toBe(false);expect(creates).toBe(1);
});

test('Dark and Light themes persist without losing graph selection or commit drafts',async({page},testInfo)=>{
 await page.emulateMedia({reducedMotion:'reduce'});await connectRealCore(page,join(folder,'theme.json'));await page.goto('/');await expect(page.locator('html')).toHaveAttribute('data-theme','dark');await page.getByLabel('Add an existing repository').fill(repository);await page.getByRole('button',{name:'Open',exact:true}).click();const head=git(repository,['rev-parse','HEAD']);await page.locator(`#commit-${head}`).click();await page.locator('.working-row').click();await page.getByLabel('Summary',{exact:true}).fill('Keep theme draft');
 await page.getByRole('button',{name:'Settings',exact:true}).click();const settings=page.locator('.settings-dialog');await expect(settings.getByLabel('Theme',{exact:true})).toHaveValue('dark');await settings.getByLabel('Theme',{exact:true}).selectOption('light');await expect(page.locator('html')).toHaveAttribute('data-theme','light');await page.screenshot({path:testInfo.outputPath('light-general.png')});await settings.getByRole('button',{name:'Close settings',exact:true}).click();await expect(page.getByLabel('Summary',{exact:true})).toHaveValue('Keep theme draft');
 await page.locator(`#commit-${head}`).click();await expect(page.locator(`#commit-${head}`)).toHaveAttribute('aria-pressed','true');await expect(page.locator('.commit-inspector h2')).toHaveText('Merge session cache fix');await page.locator('.dock-files button').filter({hasText:'src/auth/token.ts'}).click();await expect(page.locator('.split-cell.add code').first()).toBeVisible();await expect(page.locator('.split-cell.remove code').first()).toBeVisible();await page.screenshot({path:testInfo.outputPath('light-graph-diff.png')});
 const contrasts=await page.evaluate(()=>{
  function rgb(value:string){return (value.match(/[\d.]+/g)??[]).slice(0,3).map(Number);}
  function luminance(values:number[]){const [r,g,b]=values.map(n=>{n/=255;return n<=.04045?n/12.92:((n+.055)/1.055)**2.4;});return .2126*r+.7152*g+.0722*b;}
  return ['.subject','.commit-date','.detail-heading','.split-cell.add code','.split-cell.remove code'].flatMap(selector=>{const el=document.querySelector(selector);if(!el)return [];let node:Element|null=el;let bg='';while(node){bg=getComputedStyle(node).backgroundColor;if(bg!=='rgba(0, 0, 0, 0)')break;node=node.parentElement;}const fg=luminance(rgb(getComputedStyle(el).color)),back=luminance(rgb(bg));return [{selector,ratio:(Math.max(fg,back)+.05)/(Math.min(fg,back)+.05)}];});
 });expect(contrasts).toHaveLength(5);for(const value of contrasts)expect(value.ratio,value.selector).toBeGreaterThanOrEqual(4.5);
 await page.getByRole('button',{name:'Settings',exact:true}).click();await settings.getByRole('tab',{name:'FAQ',exact:true}).click();await page.screenshot({path:testInfo.outputPath('light-faq.png')});await settings.getByRole('button',{name:'Close settings',exact:true}).click();await page.reload();await expect(page.locator('html')).toHaveAttribute('data-theme','light');await page.getByRole('button',{name:'Settings',exact:true}).click();await expect(settings.getByLabel('Theme',{exact:true})).toHaveValue('light');await settings.getByLabel('Theme',{exact:true}).selectOption('dark');await expect(page.locator('html')).toHaveAttribute('data-theme','dark');await settings.getByRole('button',{name:'Close settings',exact:true}).click();expect(git(repository,['rev-parse','HEAD'])).toBe(head);expect(git(repository,['status','--porcelain'])).toContain('pending.txt');
});

test('Stage and Unstage hunk preserve the other block and working file',async({page},testInfo)=>{
 const repo=join(folder,'hunk-ui');mkdirSync(repo);git(repo,['init','-b','main']);const body=(a:string,b:string)=>Array.from({length:45},(_,i)=>`line ${i}: ${i===2?a:i===36?b:'context'}\n`).join('');writeFileSync(join(repo,'file.txt'),body('base','base'));git(repo,['add','.']);git(repo,['commit','-m','Hunk base']);writeFileSync(join(repo,'file.txt'),body('first','second'));
 await connectRealCore(page,join(folder,'hunk-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();await expect(page.getByRole('button',{name:'Stage hunk 1',exact:true})).toBeVisible();await page.screenshot({path:testInfo.outputPath('hunk-stage-review.png')});await page.getByRole('button',{name:'Stage hunk 1',exact:true}).click();await expect(page.getByRole('button',{name:'Stage hunk 1',exact:true})).toBeVisible();await expect.poll(()=>git(repo,['show',':file.txt'])).toBe(body('first','base').trim());expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe(body('first','second'));
 await page.getByRole('region',{name:'Staged files',exact:true}).locator('.changed-file').click();await expect(page.getByRole('button',{name:'Unstage hunk 1',exact:true})).toBeVisible();await page.getByRole('button',{name:'Unified',exact:true}).click();await page.getByRole('button',{name:'Unstage hunk 1',exact:true}).click();await expect.poll(()=>git(repo,['show',':file.txt'])).toBe(body('base','base').trim());expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe(body('first','second'));
});

test('Graph-owned rebase resolves a conflict and continues without changing its target branch',async({page},testInfo)=>{
 test.setTimeout(120000);const repo=join(folder,'rebase-ui');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Base']);git(repo,['checkout','-b','feature']);writeFileSync(join(repo,'file.txt'),'feature\n');git(repo,['commit','-am','Feature change']);const original=git(repo,['rev-parse','HEAD']);git(repo,['checkout','main']);writeFileSync(join(repo,'file.txt'),'target\n');git(repo,['commit','-am','Target change']);git(repo,['checkout','-b','target-side']);writeFileSync(join(repo,'extra.txt'),'target side\n');git(repo,['add','.']);git(repo,['commit','-m','Target side']);git(repo,['checkout','main']);git(repo,['merge','--no-ff','target-side','-m','Merge target side']);const target=git(repo,['rev-parse','HEAD']);git(repo,['checkout','feature']);
 await connectRealCore(page,join(folder,'rebase-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator(`#commit-${target}`).click({button:'right'});await page.getByRole('menuitem',{name:'Rebase feature onto this commit…',exact:true}).click();await expect(page.getByText('Replay 1 commits onto the selected target.',{exact:false})).toBeVisible();await expect(page.getByLabel('Mainline parent · change baseline',{exact:true})).toHaveCount(0);await expect(page.getByRole('button',{name:'Start rebase',exact:true})).toBeDisabled();expect(git(repo,['rev-parse','HEAD'])).toBe(original);await page.getByRole('checkbox',{name:'I understand rebase changes commit IDs; shared history needs coordination.',exact:true}).check();await page.getByRole('button',{name:'Start rebase',exact:true}).click();const panel=page.getByRole('complementary',{name:'Rebase working changes',exact:true});await expect(panel).toBeVisible();await panel.getByText('Whole-file choices',{exact:true}).click();await panel.getByRole('button',{name:'Use commit being replayed',exact:true}).click();await page.getByRole('button',{name:'Apply resolution',exact:true}).click();await expect(panel).toContainText('All conflicts resolved.');await expect(panel.getByLabel('Rebase message',{exact:true})).toHaveCount(0);await panel.getByRole('button',{name:'Review continue rebase',exact:true}).click();await expect(panel.getByRole('button',{name:'Continue rebase',exact:true})).toBeEnabled();await expect(page.locator('.commit-diff-dock .dock-patch')).toContainText('feature');await page.screenshot({path:testInfo.outputPath('rebase-continue-review.png')});await panel.getByRole('button',{name:'Continue rebase',exact:true}).click();await expect.poll(()=>git(repo,['rev-parse','HEAD^'])).toBe(target);expect(git(repo,['rev-parse','main'])).toBe(target);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('feature\n');expect(git(repo,['status','--porcelain'])).toBe('');
});

test('Worktree metadata review locks and unlocks without losing dirty work',async({page})=>{
 const repo=join(folder,'worktree-maintenance'),linked=join(folder,'metadata-linked');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Metadata base']);git(repo,['worktree','add','-b','side',linked]);writeFileSync(join(linked,'file.txt'),'keep local work\n');await connectRealCore(page,join(folder,'worktree-maintenance.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.commit-row')).toHaveCount(1);await page.keyboard.press('Control+o');const entry=page.locator('.worktree-entry').filter({has:page.locator('small',{hasText:linked})});await entry.getByText('Worktree metadata',{exact:true}).click();await entry.getByRole('button',{name:'Lock worktree…',exact:true}).click();await entry.getByRole('button',{name:'Apply metadata change',exact:true}).click();await expect.poll(()=>git(repo,['worktree','list','--porcelain'])).toContain('locked');expect(readFileSync(join(linked,'file.txt'),'utf8')).toBe('keep local work\n');await entry.getByRole('button',{name:'Unlock worktree…',exact:true}).click();await entry.getByRole('button',{name:'Apply metadata change',exact:true}).click();await expect.poll(()=>git(repo,['worktree','list','--porcelain'])).not.toContain('locked');expect(existsSync(linked)).toBe(true);
});

test('Selected submodule URL synchronization preserves its dirty checkout and recorded pointer',async({page},testInfo)=>{
 const parent=join(folder,'sync-parent'),old=join(folder,'sync-old'),next=join(folder,'sync-next');for(const root of [parent,old,next]){mkdirSync(root);git(root,['init','-b','main']);writeFileSync(join(root,'file.txt'),'base\n');git(root,['add','.']);git(root,['commit','-m','Sync fixture']);}git(parent,['-c','protocol.file.allow=always','submodule','add',old,'child']);git(parent,['commit','-am','Record child']);git(parent,['config','--file','.gitmodules','submodule.child.url',next]);writeFileSync(join(parent,'child/file.txt'),'keep child edits\n');const childHead=git(join(parent,'child'),['rev-parse','HEAD']);const recorded=git(parent,['ls-files','--stage','child']);
 await connectRealCore(page,join(folder,'sync-parent.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(parent);await page.getByRole('button',{name:'Open',exact:true}).click();const group=page.getByRole('region',{name:'Branch submodules',exact:true});await group.getByRole('button',{name:/Manage submodule/}).click();await group.getByRole('button',{name:'Sync URLs…',exact:true}).click();const review=group.getByRole('region',{name:'Review submodule URL synchronization',exact:true});await expect(review).toContainText(next);await page.screenshot({path:testInfo.outputPath('submodule-sync-review.png')});await review.getByRole('button',{name:'Sync URLs',exact:true}).click();await expect.poll(()=>git(join(parent,'child'),['config','remote.origin.url'])).toBe(next);expect(git(join(parent,'child'),['rev-parse','HEAD'])).toBe(childHead);expect(git(parent,['ls-files','--stage','child'])).toBe(recorded);expect(readFileSync(join(parent,'child/file.txt'),'utf8')).toBe('keep child edits\n');
});

test('large Amend reviews all 107 staged files with per-file patches and preserves unstaged work',async({page},testInfo)=>{
 test.setTimeout(120000);
 const repo=join(folder,'amend-large');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Original root']);const old=git(repo,['rev-parse','HEAD']);
 for(let i=0;i<107;i++)writeFileSync(join(repo,`change-${String(i).padStart(3,'0')}.txt`),`file ${String(i).padStart(3,'0')}\n${'reviewable content\n'.repeat(600)}\n`);
 git(repo,['add','.']);const tree=git(repo,['write-tree']);writeFileSync(join(repo,'file.txt'),'private unstaged work\n');
 await connectRealCore(page,join(folder,'amend-large.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();
 const panel=page.getByRole('complementary',{name:'Working changes',exact:true});await panel.getByRole('checkbox',{name:'Amend last commit',exact:true}).check();await panel.getByRole('button',{name:'Review amend',exact:true}).click();await expect(panel).toContainText('The complete staged file list is selected.');await expect(panel.locator('.staged-group .changed-file')).toHaveCount(107);
 await panel.locator('.staged-group .changed-file').filter({hasText:'change-106.txt'}).click();await expect(page.getByLabel('Amend delta preview',{exact:true})).toContainText('file 106');await expect(panel.getByRole('button',{name:'Amend reviewed commit',exact:true})).toBeEnabled();await page.screenshot({path:testInfo.outputPath('large-amend-per-file.png')});await panel.getByRole('button',{name:'Amend reviewed commit',exact:true}).click();await expect(panel).toContainText('Last commit amended; original retained for recovery');
 expect(git(repo,['rev-parse','HEAD'])).not.toBe(old);expect(git(repo,['rev-parse','HEAD^{tree}'])).toBe(tree);expect(git(repo,['ls-tree','-r','--name-only','HEAD']).split('\n')).toHaveLength(108);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('private unstaged work\n');
});

test('new remote branch with long shared history uses bounded review and pushes the complete tip',async({page},testInfo)=>{
 const repo=join(folder,'long-push'),bare=join(folder,'long-push.git');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['commit','--allow-empty','-m','Shared base']);const base=git(repo,['rev-parse','HEAD']);git(folder,['init','--bare',bare]);git(repo,['remote','add','origin',bare]);git(repo,['push','origin','main']);
 let stream='';for(let n=1;n<=520;n++){const subject=`Feature history ${n}`,parent=n===1?base:`:${n-1}`;stream+=`commit refs/heads/feature\nmark :${n}\ncommitter GitNova Test <test@example.invalid> ${1700000000+n} +0000\ndata ${subject.length}\n${subject}\nfrom ${parent}\n\n`;}
 execFileSync('git',['-C',repo,'fast-import','--quiet'],{input:stream});git(repo,['switch','feature']);const tip=git(repo,['rev-parse','HEAD']);
 await connectRealCore(page,join(folder,'long-push.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.push-current')).toBeEnabled();await page.locator('.push-current').click();await page.getByRole('button',{name:'Check destination & review',exact:true}).click();
 const review=page.locator('.push-review');await expect(review).toContainText('521 commits in this branch history');await expect(review).toContainText('Showing the latest 500 of 521 commits.');await expect(review).toContainText('These are not necessarily new commits or objects to upload.');await expect(review.locator('.outgoing-commits > div')).toHaveCount(500);await expect(page.getByRole('button',{name:'Push reviewed branch',exact:true})).toBeEnabled();expect(git(bare,['for-each-ref','--format=%(refname)','refs/heads/feature'])).toBe('');
 await page.screenshot({path:testInfo.outputPath('long-branch-push-review.png')});await page.getByRole('button',{name:'Push reviewed branch',exact:true}).click();await expect(page.locator('.graph-actions')).toHaveCount(0,{timeout:45000});expect(git(bare,['rev-parse','feature'])).toBe(tip);expect(git(bare,['rev-list','--count','feature'])).toBe('521');expect(git(bare,['rev-parse','main'])).toBe(base);expect(git(repo,['rev-parse','HEAD'])).toBe(tip);expect(git(repo,['status','--porcelain'])).toBe('');
});


test('auto fetch defaults to one minute, preserves checkout and selection, pauses hidden and can be disabled',async({page},testInfo)=>{
 const writer=join(folder,'auto-writer'),repo=join(folder,'auto-local');mkdirSync(writer);git(writer,['init','-b','main']);writeFileSync(join(writer,'file.txt'),'base\n');git(writer,['add','.']);git(writer,['commit','-m','Auto base']);git(folder,['clone',writer,repo]);const head=git(repo,['rev-parse','HEAD']);
 writeFileSync(join(repo,'file.txt'),'staged draft\n');git(repo,['add','file.txt']);writeFileSync(join(repo,'file.txt'),'unstaged draft\n');const index=readFileSync(join(repo,'.git/index'));
 let fetches=0;await connectRealCore(page,join(folder,'auto-fetch.json'),command=>{if(command==='auto_fetch')fetches++;},null);
 const clock=new Date('2026-01-01T00:00:00Z');await page.clock.install({time:clock});await page.clock.pauseAt(new Date(clock.getTime()+1000));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator(`#commit-${head}`).click();
 writeFileSync(join(writer,'remote.txt'),'incoming\n');git(writer,['add','.']);git(writer,['commit','-m','Incoming auto fetched commit']);const incoming=git(writer,['rev-parse','HEAD']);
 await page.clock.fastForward(59000);expect(fetches).toBe(0);await page.clock.fastForward(1000);await expect.poll(()=>fetches).toBe(1);await expect(page.locator('.auto-fetch-status')).toHaveText('Auto fetched',{timeout:45000});await expect(page.locator(`#commit-${incoming}`)).toBeVisible();await expect(page.locator(`#commit-${head}`)).toHaveAttribute('aria-pressed','true');expect(git(repo,['rev-parse','HEAD'])).toBe(head);expect(git(repo,['rev-parse','origin/main'])).toBe(incoming);expect(readFileSync(join(repo,'.git/index'))).toEqual(index);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('unstaged draft\n');
 await page.evaluate(()=>{Object.defineProperty(document,'hidden',{configurable:true,get:()=>true});document.dispatchEvent(new Event('visibilitychange'));});await page.clock.fastForward(180000);expect(fetches).toBe(1);await page.evaluate(()=>{Object.defineProperty(document,'hidden',{configurable:true,get:()=>false});document.dispatchEvent(new Event('visibilitychange'));});await page.clock.fastForward(1000);await expect.poll(()=>fetches).toBe(2);await expect(page.locator('.auto-fetch-status')).toHaveText('Auto fetched');
 await page.keyboard.press('Control+,');const settings=page.getByRole('dialog',{name:'Settings',exact:true});await expect(settings.getByLabel('Auto fetch',{exact:true})).toHaveValue('60');await page.clock.fastForward(120000);expect(fetches).toBe(2);await settings.getByLabel('Auto fetch',{exact:true}).selectOption('0');await settings.getByRole('button',{name:'Close settings',exact:true}).click();await page.clock.fastForward(180000);expect(fetches).toBe(2);expect(await page.evaluate(()=>localStorage.getItem('gitnova.auto-fetch-seconds'))).toBe('0');await expect(page.locator('.auto-fetch-status')).toHaveCount(0);await page.screenshot({path:testInfo.outputPath('auto-fetch-preserved-graph.png')});
});

test('auto fetch failures back off without dialogs or repeated immediate retries',async({page})=>{
 const repo=join(folder,'auto-failure');mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['commit','--allow-empty','-m','Failure fixture']);git(repo,['remote','add','origin',join(folder,'missing-remote.git')]);
 let fetches=0;await connectRealCore(page,join(folder,'auto-failure.json'),command=>{if(command==='auto_fetch')fetches++;},30);const clock=new Date('2026-01-01T00:00:00Z');await page.clock.install({time:clock});await page.clock.pauseAt(new Date(clock.getTime()+1000));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.commit-row')).toHaveCount(1);
 await page.clock.fastForward(30000);await expect(page.locator('.auto-fetch-status')).toHaveText('Auto fetch paused',{timeout:45000});expect(fetches).toBe(1);await expect(page.getByRole('dialog')).toHaveCount(0);await expect(page.getByRole('dialog',{name:'Settings',exact:true})).not.toBeVisible();await page.clock.fastForward(59000);expect(fetches).toBe(1);await page.clock.fastForward(1000);await expect.poll(()=>fetches).toBe(2);await expect(page.locator('.auto-fetch-status')).toHaveText('Auto fetch paused');expect(git(repo,['status','--porcelain'])).toBe('');
});


test('initialize repository reviews an existing project then opens an unborn graph', async ({page}, testInfo) => {
  test.setTimeout(120000);
  const destination=join(folder,'init-existing-project');mkdirSync(destination);writeFileSync(join(destination,'README.md'),'Existing project\n');
  await connectRealCore(page,join(folder,'init-catalog.json'));await page.goto('/');
  await page.locator('.init-controls > summary').click();const controls=page.locator('.init-controls');
  await controls.getByLabel('Repository folder',{exact:true}).fill(destination);
  await controls.getByLabel('Initial branch',{exact:true}).fill('develop');
  await controls.getByRole('button',{name:'Review initialization',exact:true}).click();
  await expect(controls.locator('.clone-preview')).toContainText(destination);expect(existsSync(join(destination,'.git'))).toBe(false);
  await controls.getByRole('button',{name:'Initialize & open graph',exact:true}).click();
  await expect(page.getByText('No commits yet',{exact:true})).toBeVisible();await expect(page.locator('.branch-current')).toContainText('develop');
  await page.locator('.working-row').click();
  await expect(page.locator('.working-detail')).toContainText('README.md');
  expect(readFileSync(join(destination,'README.md'),'utf8')).toBe('Existing project\n');expect(git(destination,['ls-files'])).toBe('');
  await page.screenshot({path:testInfo.outputPath('initialized-project.png')});
});


test('graph columns resize together, persist, reset and keep selection and scroll alignment',async({page},testInfo)=>{
  test.setTimeout(120000);
  await page.setViewportSize({width:1440,height:900});
  await connectRealCore(page,join(folder,'column-widths.json'));await page.goto('/');
  await page.evaluate(()=>localStorage.removeItem('gitnova.history-column-widths.v1'));await page.reload();
  await page.getByLabel('Add an existing repository').fill(repository);await page.getByRole('button',{name:'Open',exact:true}).click();
  const selected=await page.locator('.commit-row.selected').getAttribute('id');
  const widths=()=>page.locator('.history-heading > span').evaluateAll(nodes=>nodes.map(n=>Math.round(n.getBoundingClientRect().width)));
  for(const [index,label] of ['Branch / Tag','Graph','Commit message','CI','Author','Date'].entries()){
    const before=(await widths())[index];
    const separator=page.getByRole('separator',{name:`Resize ${label} column`,exact:true});
    await separator.scrollIntoViewIfNeeded();const box=(await separator.boundingBox())!;
    await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width/2+40,box.y+box.height/2,{steps:5});await page.mouse.up();
    await expect.poll(async()=>(await widths())[index]).toBeGreaterThanOrEqual(before+35);
  }
  const resized=await widths();
  await expect(page.locator('.commit-row.selected')).toHaveAttribute('id',selected!);
  const aligned=()=>page.evaluate(()=>{
    const header=[...document.querySelectorAll('.history-heading > span')].map(n=>n.getBoundingClientRect());
    const row=[...document.querySelector('.commit-row')!.children].map(n=>n.getBoundingClientRect());
    return header.every((box,i)=>Math.abs(box.left-row[i].left)<1.1&&Math.abs(box.width-row[i].width)<1.1);
  });
  await expect.poll(aligned).toBe(true);
  await page.reload();await expect(page.locator('.commit-row')).not.toHaveCount(0);await expect.poll(widths).toEqual(resized);
  await page.locator('.history-scroll').evaluate(n=>{n.scrollLeft=250;});await expect.poll(aligned).toBe(true);
  const divider=page.getByRole('separator',{name:'Resize CI column',exact:true});await divider.scrollIntoViewIfNeeded();await divider.focus();await page.keyboard.press('ArrowRight');await expect.poll(async()=>(await widths())[3]).toBe(resized[3]+10);
  await divider.dblclick();await expect.poll(async()=>(await widths())[3]).toBe(140);
  await page.setViewportSize({width:780,height:700});await expect.poll(aligned).toBe(true);
  await page.locator('.history-scroll').evaluate(n=>{n.scrollLeft=0;});await expect.poll(aligned).toBe(true);
  await page.screenshot({path:testInfo.outputPath('resizable-graph-columns.png')});
});

test('file and code investigation opens the matching diff and compares branches across repositories without checkout',async({page},testInfo)=>{
  test.setTimeout(120000);const repos=[join(folder,'investigate-api'),join(folder,'investigate-web')];
  for(const repo of repos){mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'a-first.txt'),'base\n');writeFileSync(join(repo,'z-code.txt'),'before\n');git(repo,['add','.']);git(repo,['commit','-m','Investigation base']);git(repo,['branch','release']);git(repo,['checkout','-b','develop']);writeFileSync(join(repo,'a-first.txt'),'another change\n');writeFileSync(join(repo,'z-code.txt'),'after unique archaeology phrase\n');git(repo,['commit','-am','Introduce investigated behavior']);writeFileSync(join(repo,'draft.txt'),'preserve draft\n');}
  const before=repos.map(repo=>({head:git(repo,['rev-parse','HEAD']),index:readFileSync(join(repo,'.git/index')),refs:git(repo,['show-ref'])}));
  await connectRealCore(page,join(folder,'investigation-state.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repos[0]);await page.getByRole('button',{name:'Open',exact:true}).click();await addSearchRepo(page,repos[1]);
  await expect(page.getByRole('button',{name:'Search commits…',exact:true})).toBeEnabled();await page.keyboard.press('ControlOrMeta+k');const search=page.getByRole('region',{name:'Commit search',exact:true});await search.getByLabel('Search mode',{exact:true}).selectOption('history');await search.getByLabel('Literal repository path (optional)',{exact:true}).fill('z-code.txt');await search.getByLabel('Literal code phrase (optional)',{exact:true}).fill('unique archaeology phrase');await search.getByLabel('Commit search scope',{exact:true}).selectOption('workspace');await search.getByRole('button',{name:'Search',exact:true}).click();await expect(search.locator('.commit-search-result')).toHaveCount(2);await expect(search.locator('.investigation-evidence')).toContainText('investigate-api');await page.screenshot({path:testInfo.outputPath('file-code-investigation.png')});
  await search.locator('.commit-search-result').filter({hasText:'investigate-api'}).click();await expect(search).toBeHidden();await expect(page.locator('.dock-file-heading')).toContainText('z-code.txt');
  await expect(page.getByRole('button',{name:'Search commits…',exact:true})).toBeEnabled();await page.keyboard.press('ControlOrMeta+k');await search.getByLabel('Search mode',{exact:true}).selectOption('compare');await search.getByRole('button',{name:'Search',exact:true}).click();await expect(search.locator('.investigation-evidence')).toContainText('1 source-only · 0 target-only · 2 files differ');await expect(search.locator('.commit-search-result')).toHaveCount(2);await expect(search).toContainText('does not certify release readiness');await page.screenshot({path:testInfo.outputPath('cross-repository-branch-evidence.png')});
  await search.getByLabel('Target branch',{exact:true}).fill('missing-release');await search.getByRole('button',{name:'Search',exact:true}).click();await expect(search.locator('.investigation-evidence > div')).toHaveCount(2);await expect(search.locator('.commit-search-result')).toHaveCount(0);await expect(search).toContainText('Results are incomplete.');
  for(let i=0;i<repos.length;i++){expect(git(repos[i],['rev-parse','HEAD'])).toBe(before[i].head);expect(git(repos[i],['show-ref'])).toBe(before[i].refs);expect(readFileSync(join(repos[i],'.git/index'))).toEqual(before[i].index);expect(readFileSync(join(repos[i],'draft.txt'),'utf8')).toBe('preserve draft\n');}
});

test('connection commit identities separate hosts in Settings and real commits without changing Git config', async ({page},testInfo) => {
  const rows=[{id:'personal',label:'Personal',provider:'github',origin:'https://public.example',enabled:false},{id:'work',label:'Work',provider:'gitlab',origin:'https://work.example',enabled:false}];
  const repos=rows.map(row=>{const repo=join(folder,`identity-${row.id}`);mkdirSync(repo);git(repo,['init','-b','main']);git(repo,['config','user.name','Fallback']);git(repo,['config','user.email','fallback@example.invalid']);git(repo,['config','commit.gpgsign','false']);git(repo,['config','core.hooksPath','.git/no-hooks']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','base']);git(repo,['remote','add','origin',`${row.origin}/owner/repository.git`]);writeFileSync(join(repo,'file.txt'),'change\n');git(repo,['add','file.txt']);return repo;});
  const before=repos.map(repo=>readFileSync(join(repo,'.git/config'),'utf8'));
  await connectRealCore(page,join(folder,'identity-ui.json'));
  await page.addInitScript(values=>{
    localStorage.setItem('gitnova.connections.v1',JSON.stringify(values));
    // Authentication is unrelated to author settings. Never query real Keychain
    // accounts or contact providers; all identity commands use the real core.
    const host=window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args:Record<string,unknown>)=>Promise<unknown>}};
    const original=host.__TAURI_INTERNALS__.invoke;
    host.__TAURI_INTERNALS__.invoke=(command,args)=>command==='host_account'?Promise.resolve({...args.profile as object,account:null}):original(command,args);
  },rows);
  await page.goto('/');await page.getByLabel('Add an existing repository').fill(repos[0]);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();
  await expect(page.locator('.commit-identity-notice')).toContainText('Fallback');
  await page.keyboard.press('Control+,');const settings=page.locator('dialog.settings-dialog');await settings.getByRole('tab',{name:'Connections',exact:true}).click();
  for(const row of rows){const form=settings.getByRole('form',{name:`Commit identity for ${row.origin}`,exact:true});await form.getByLabel('Commit name',{exact:true}).fill(row.label);await form.getByLabel('Commit email',{exact:true}).fill(`${row.id}@example.invalid`);await form.getByRole('button',{name:'Save commit identity',exact:true}).click();await expect(form).toContainText(`Saved commit identity: ${row.label}`);}
  await page.screenshot({path:testInfo.outputPath('connection-commit-identities.png')});await page.keyboard.press('Escape');await expect(settings).not.toBeVisible();
  for(let i=0;i<repos.length;i++){
    if(i){await page.locator('.repo-picker').click();await page.getByLabel('Add an existing repository').fill(repos[i]);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();}
    const working=page.getByLabel('Working changes',{exact:true});await expect(working.locator('.commit-identity-notice')).toContainText(`${rows[i].id}@example.invalid`);
    await working.getByLabel('Summary',{exact:true}).fill(`Scoped ${rows[i].label} commit`);await working.getByRole('button',{name:'Review commit',exact:true}).click();await working.getByRole('button',{name:'Commit staged changes',exact:true}).click();
    await expect.poll(()=>git(repos[i],['show','-s','--format=%an|%ae|%cn|%ce','HEAD'])).toBe(`${rows[i].label}|${rows[i].id}@example.invalid|${rows[i].label}|${rows[i].id}@example.invalid`);
    expect(readFileSync(join(repos[i],'.git/config'),'utf8')).toBe(before[i]);
  }
});

test('graph Terminal action opens the selected repository dock without losing graph context',async({page})=>{
 await connectRealCore(page,join(folder,'terminal-action.json'));await page.goto('/');
 await page.getByLabel('Add an existing repository').fill(repository);await page.getByRole('button',{name:'Open',exact:true}).click();
 await expect(page.locator('.terminal-toggle')).toBeEnabled();
 const selected=await page.locator('.commit-row.selected').getAttribute('id');
 // Only the PTY transport is emulated here; workspace and graph read real Git.
 await page.evaluate(async path=>{
  const {api}=await import('/src/api.ts');let opened=0;
  api.terminalOpen=async()=>{opened++;(window as any).terminalOpenCount=opened;return{id:'test-terminal',path,shell:'/bin/sh'};};
  api.terminalRead=async()=>({data:[],dropped:false,exited:false});api.terminalWrite=async()=>{};api.terminalResize=async()=>{};api.terminalClose=async()=>{};
 },repository);
 await page.locator('.terminal-toggle').click();await expect(page.getByRole('region',{name:'Repository terminal',exact:true})).toBeVisible();
 await expect(page.locator('.terminal-context')).toContainText(repository);
 await page.getByRole('button',{name:'Hide terminal',exact:true}).click();await expect(page.locator('.repository-terminal')).toBeHidden();
 await page.locator('.terminal-toggle').click();await expect(page.locator('.repository-terminal')).toBeVisible();
 expect(await page.evaluate(()=>(window as any).terminalOpenCount)).toBe(1);
 expect(await page.locator('.commit-row.selected').getAttribute('id')).toBe(selected);
 await page.locator('.repository-terminal').getByRole('button',{name:'End session',exact:true}).click();
 await page.locator('.terminal-end-review').getByRole('button',{name:'End session',exact:true}).click();await expect(page.locator('.repository-terminal')).toHaveCount(0);
});

test('submodule newer version review is separate from recorded restore and parent staging',async({page},testInfo)=>{
 test.setTimeout(120000);
 const source=join(folder,'newer-source'),repo=join(folder,'newer-parent');mkdirSync(source);mkdirSync(repo);
 git(source,['init','-b','main']);writeFileSync(join(source,'file.txt'),'first\n');git(source,['add','.']);git(source,['commit','-m','child first']);const first=git(source,['rev-parse','HEAD']);
 writeFileSync(join(source,'file.txt'),'newer\n');git(source,['commit','-am','child newer']);const newer=git(source,['rev-parse','HEAD']);
 git(repo,['init','-b','main']);git(repo,['-c','protocol.file.allow=always','submodule','add',source,'module']);const child=join(repo,'module');git(child,['switch','--detach',first]);git(repo,['add','.']);git(repo,['commit','-m','record child first']);
 writeFileSync(join(repo,'prepared.txt'),'keep staged\n');git(repo,['add','prepared.txt']);const index=readFileSync(join(repo,'.git/index'));
 await connectRealCore(page,join(folder,'newer-state.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
 await page.getByRole('button',{name:'Manage submodule module',exact:true}).click();
 await page.getByRole('button',{name:'Review newer version…',exact:true}).click();
 const review=page.getByRole('region',{name:'Review submodule module',exact:true});
 await expect(review).toContainText('Update from cached remote branch');await expect(review).toContainText('refs/remotes/origin/main');await expect(review).toContainText(newer.slice(0,12));await expect(review).toContainText('stage the new pointer separately');
 await review.getByText('1 changed path',{exact:true}).click();await expect(review).toContainText('file.txt');
 expect(git(child,['rev-parse','HEAD'])).toBe(first);expect(readFileSync(join(repo,'.git/index'))).toEqual(index);
 await expect(review.getByRole('button',{name:'Update to reviewed commit',exact:true})).toBeEnabled();await expect(review.getByRole('button',{name:'Update to reviewed commit',exact:true})).toBeInViewport();await page.screenshot({path:testInfo.outputPath('submodule-newer-review.png')});
 await review.getByRole('button',{name:'Update to reviewed commit',exact:true}).click();
 await expect(page.getByRole('button',{name:'Manage submodule module',exact:true})).toBeEnabled();
 await expect(page.locator('.branch-submodule-state')).toContainText('Commit changed');
 expect(git(child,['rev-parse','HEAD'])).toBe(newer);expect(git(repo,['rev-parse',':module'])).toBe(first);expect(readFileSync(join(repo,'.git/index'))).toEqual(index);expect(git(repo,['show',':prepared.txt'])).toBe('keep staged');
});

test('submodule registration lifecycle adds and commits then removes registration while retaining child files',async({page},testInfo)=>{
 test.setTimeout(180000);const repo=join(folder,'registration-parent'),source=join(folder,'registration-source');
 for(const path of [repo,source]){mkdirSync(path);git(path,['init','-b','main']);writeFileSync(join(path,'file.txt'),'base\n');git(path,['add','.']);git(path,['commit','-m','Registration fixture']);}
 await connectRealCore(page,join(folder,'registration-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();
 const group=page.getByRole('region',{name:'Branch submodules',exact:true});await group.getByRole('button',{name:'Add submodule…',exact:true}).click();let review=group.getByRole('region',{name:'Review submodule registration',exact:true});await review.getByLabel('Submodule folder',{exact:true}).fill('child');await review.getByLabel('Repository URL or local source',{exact:true}).fill(source);await review.getByRole('button',{name:'Review registration change',exact:true}).click();await expect(review).toContainText('Downloads the source default branch');expect(existsSync(join(repo,'child'))).toBe(false);await page.screenshot({path:testInfo.outputPath('submodule-add-review.png')});await review.getByRole('button',{name:'Add reviewed submodule',exact:true}).click();await expect(group.getByRole('button',{name:'Manage submodule child',exact:true})).toBeVisible();expect(git(repo,['ls-files','--stage','child'])).toMatch(/^160000 /);
 await page.locator('.working-row').click();const working=page.getByRole('complementary',{name:'Working changes',exact:true});await working.getByLabel('Summary',{exact:true}).fill('Add child registration');await working.getByRole('button',{name:'Review commit',exact:true}).click();await expect(working.getByRole('button',{name:'Commit staged changes',exact:true})).toBeEnabled();await working.getByRole('button',{name:'Commit staged changes',exact:true}).click();await expect.poll(()=>git(repo,['log','-1','--format=%s'])).toBe('Add child registration');
 writeFileSync(join(repo,'child/file.txt'),'keep child work\n');await group.getByRole('button',{name:'Manage submodule child',exact:true}).click();await group.getByRole('button',{name:'Remove registration…',exact:true}).click();review=group.getByRole('region',{name:'Review submodule registration',exact:true});await review.getByRole('button',{name:'Review registration change',exact:true}).click();await expect(review).toContainText('Child files, local commits and retained Git metadata stay on disk');await page.screenshot({path:testInfo.outputPath('submodule-remove-review.png')});await review.getByRole('button',{name:'Remove registration and keep files',exact:true}).click();await expect.poll(()=>git(repo,['ls-files','--','child'])).toBe('');expect(readFileSync(join(repo,'child/file.txt'),'utf8')).toBe('keep child work\n');
});

test('moved worktree repair keeps local edits and reconnects the registered folder',async({page},testInfo)=>{
 const repo=join(folder,'reattach-parent'),old=join(folder,'reattach-old'),moved=join(folder,'reattach-moved');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Reattach fixture']);git(repo,['worktree','add','-b','side',old]);writeFileSync(join(old,'file.txt'),'keep moved work\n');renameSync(old,moved);
 await connectRealCore(page,join(folder,'reattach-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.commit-row')).toHaveCount(1);await expect(page.locator('.repo-picker')).toBeEnabled();await page.keyboard.press('Control+o');const entry=page.locator('.worktree-entry').filter({has:page.locator('small',{hasText:old})});await entry.getByText('Worktree metadata',{exact:true}).click();await entry.getByLabel('Moved checkout folder',{exact:true}).fill(realpathSync(moved));await entry.getByRole('button',{name:'Reattach moved worktree…',exact:true}).click();await expect(entry).toContainText('Reattaches the original moved checkout');await page.screenshot({path:testInfo.outputPath('worktree-reattach-review.png')});await entry.getByRole('button',{name:'Apply metadata change',exact:true}).click();await expect.poll(()=>git(repo,['worktree','list','--porcelain'])).toContain(moved);expect(readFileSync(join(moved,'file.txt'),'utf8')).toBe('keep moved work\n');expect(git(moved,['symbolic-ref','--short','HEAD'])).toBe('side');expect(existsSync(old)).toBe(false);
});

test('follow file renames opens the exact historical filename without checkout',async({page},testInfo)=>{
 const repo=join(folder,'follow-history');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'old.txt'),'original\n');git(repo,['add','.']);git(repo,['commit','-m','Historical original']);const old=git(repo,['rev-parse','HEAD']);git(repo,['mv','old.txt','new.txt']);git(repo,['commit','-m','Rename document']);writeFileSync(join(repo,'new.txt'),'latest\n');git(repo,['commit','-am','Edit renamed document']);const tip=git(repo,['rev-parse','HEAD']);
 await connectRealCore(page,join(folder,'follow-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.commit-row')).toHaveCount(3);
 await page.getByRole('button',{name:'Search commits…',exact:true}).click();await page.getByLabel('Search mode',{exact:true}).selectOption('follow');await page.getByLabel('File path at revision',{exact:true}).fill('new.txt');await page.getByRole('button',{name:'Search',exact:true}).click();const hit=page.locator('.commit-search-result').filter({hasText:'Historical original'});await expect(hit).toContainText('old.txt');await hit.click();await expect(page.locator(`#commit-${old}`)).toHaveClass(/selected/);await expect(page.locator('.commit-diff-dock')).toContainText('old.txt');await page.screenshot({path:testInfo.outputPath('follow-renames.png')});expect(git(repo,['rev-parse','HEAD'])).toBe(tip);expect(readFileSync(join(repo,'new.txt'),'utf8')).toBe('latest\n');
});

test('registration staging pairs the manifest and gitlink and preserves unrelated staging',async({page},testInfo)=>{
 const source=join(folder,'index-child'),repo=join(folder,'index-parent');for(const p of [source,repo]){mkdirSync(p);git(p,['init','-b','main']);writeFileSync(join(p,'file.txt'),'base\n');git(p,['add','.']);git(p,['commit','-m','Registration fixture']);}
 git(repo,['-c','protocol.file.allow=always','submodule','add',source,'child']);writeFileSync(join(repo,'other.txt'),'keep staged\n');git(repo,['add','other.txt']);
 await connectRealCore(page,join(folder,'registration-index-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.commit-row')).toHaveCount(1);await page.locator('.working-row').click();await page.getByRole('button',{name:'Unstage submodule pointer',exact:true}).click();let review=page.getByRole('region',{name:'Review submodule registration',exact:true});await review.getByRole('button',{name:'Review registration change',exact:true}).click();await expect(review).toContainText('entire .gitmodules');await review.getByRole('button',{name:'Unstage registration',exact:true}).click();await expect.poll(()=>git(repo,['ls-files','.gitmodules','child'])).toBe('');expect(git(repo,['show',':other.txt'])).toBe('keep staged');
 await page.getByRole('region',{name:'Unstaged files',exact:true}).getByRole('button').filter({hasText:'.gitmodules'}).click();await page.getByRole('button',{name:'Stage registration…',exact:true}).click();review=page.getByRole('region',{name:'Review submodule registration',exact:true});await review.getByLabel('Submodule folder',{exact:true}).fill('child');await review.getByRole('button',{name:'Review registration change',exact:true}).click();await review.getByText('Manifest staging change',{exact:true}).click();await expect(review).toContainText('submodule');await page.screenshot({path:testInfo.outputPath('registration-staging.png')});await review.getByRole('button',{name:'Stage registration',exact:true}).click();await expect.poll(()=>git(repo,['ls-files','child'])).toBe('child');expect(git(repo,['show',':other.txt'])).toBe('keep staged');
});

test('missing worktree prune archives metadata and keeps its local branch',async({page},testInfo)=>{
 const repo=join(folder,'prune-ui-parent'),missing=join(folder,'prune-ui-missing');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Prune fixture']);git(repo,['worktree','add','-b','side',missing]);const tip=git(repo,['rev-parse','side']);rmSync(missing,{recursive:true});
 await connectRealCore(page,join(folder,'prune-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.commit-row')).toHaveCount(1);await expect(page.locator('.repo-picker')).toBeEnabled();await page.keyboard.press('Control+o');const entry=page.locator('.worktree-entry').filter({has:page.locator('small',{hasText:'prune-ui-missing'})});await entry.getByText('Worktree metadata',{exact:true}).click();await entry.getByRole('button',{name:'Prune missing worktree metadata…',exact:true}).click();await expect(entry).toContainText('Git metadata is archived first');await page.screenshot({path:testInfo.outputPath('prune-missing-worktree.png')});await entry.getByRole('button',{name:'Apply metadata change',exact:true}).click();await expect.poll(()=>git(repo,['worktree','list','--porcelain'])).not.toContain(missing);expect(git(repo,['rev-parse','side'])).toBe(tip);
});

test('graph rebase recreates two-parent merges and retains the target and other local branches',async({page},testInfo)=>{
 const repo=join(folder,'merge-rebase-ui');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'base.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Base']);const base=git(repo,['rev-parse','HEAD']);git(repo,['checkout','-b','feature']);writeFileSync(join(repo,'feature.txt'),'feature\n');git(repo,['add','.']);git(repo,['commit','-m','Feature']);git(repo,['checkout','-b','side',base]);writeFileSync(join(repo,'side.txt'),'side\n');git(repo,['add','.']);git(repo,['commit','-m','Side']);const side=git(repo,['rev-parse','HEAD']);git(repo,['checkout','feature']);git(repo,['merge','--no-ff','side','-m','Merge side']);git(repo,['checkout','main']);writeFileSync(join(repo,'target.txt'),'target\n');git(repo,['add','.']);git(repo,['commit','-m','Target']);const target=git(repo,['rev-parse','HEAD']);git(repo,['checkout','feature']);
 await connectRealCore(page,join(folder,'topology-rebase-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator(`#commit-${target}`).click({button:'right'});await page.getByRole('menuitem',{name:'Rebase feature onto this commit…',exact:true}).click();await expect(page.locator('.merge-start-review')).toContainText('Preserves reviewed merge topology');await page.getByRole('checkbox',{name:'I understand rebase changes commit IDs; shared history needs coordination.',exact:true}).check();await expect(page.getByRole('button',{name:'Start rebase',exact:true})).toBeEnabled();await page.screenshot({path:testInfo.outputPath('topology-rebase-review.png')});await page.getByRole('button',{name:'Start rebase',exact:true}).click();await expect.poll(()=>git(repo,['rev-parse','HEAD^2'])).not.toBe(side);expect(git(repo,['rev-list','--parents','-n','1','HEAD']).split(/\s+/)).toHaveLength(3);expect(git(repo,['rev-parse','main'])).toBe(target);expect(git(repo,['rev-parse','side'])).toBe(side);expect(git(repo,['status','--porcelain'])).toBe('');
});

test('branch comparison displays registered gitlink ancestry and opens the matching child commit',async({page},testInfo)=>{
 const source=join(folder,'dependency-source'),repo=join(folder,'dependency-parent');for(const p of [source,repo]){mkdirSync(p);git(p,['init','-b','main']);writeFileSync(join(p,'base.txt'),'base\n');git(p,['add','.']);git(p,['commit','-m','Dependency base']);git(p,['checkout','-b','develop']);writeFileSync(join(p,'feature.txt'),'feature\n');git(p,['add','.']);git(p,['commit','-m','Dependency feature']);}
 git(repo,['-c','protocol.file.allow=always','submodule','add',source,'library']);git(repo,['commit','-m','Pin child']);git(repo,['branch','release','HEAD']);const child=join(repo,'library');git(child,['checkout','main']);const wanted=git(child,['rev-parse','HEAD']);git(repo,['add','library']);git(repo,['commit','-m','Select older child']);const tip=git(repo,['rev-parse','HEAD']);
 await connectRealCore(page,join(folder,'dependencies-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(child);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.commit-row')).toHaveCount(2);await expect(page.locator('.repo-picker')).toBeEnabled();await page.keyboard.press('Control+o');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.commit-row')).toHaveCount(4);
 await page.getByRole('button',{name:'Search commits…',exact:true}).click();await page.getByLabel('Search mode',{exact:true}).selectOption('compare');await page.getByRole('button',{name:'Search',exact:true}).click();await page.getByText('Gitlink dependency evidence',{exact:false}).click();const dep=page.locator('.dependency-evidence');await expect(dep).toContainText('library');await expect(dep).toContainText('Dependency rollback');await expect(dep).toContainText(wanted.slice(0,12));await page.screenshot({path:testInfo.outputPath('gitlink-dependency-evidence.png')});await dep.getByRole('button',{name:'Inspect dependency commit · library',exact:true}).click();await expect(page.locator(`#commit-${wanted}`)).toHaveClass(/selected/);expect(git(repo,['rev-parse','HEAD'])).toBe(tip);expect(git(repo,['status','--porcelain'])).toBe('');
});


test('stale lock recovery reviews ownership archives only the old lock and refuses a live owner',async({page})=>{
 const repo=join(folder,'stale-lock-ui');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file.txt'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','Recovery fixture']);writeFileSync(join(repo,'file.txt'),'staged\n');git(repo,['add','file.txt']);writeFileSync(join(repo,'file.txt'),'working\n');
 const lock=join(repo,'.git/gitnova-operation.lock');const child=spawnSync(process.execPath,['-e','process.exit(0)']);expect(child.status).toBe(0);const contents=`GitNova pid=${child.pid}\n`;writeFileSync(lock,contents);const index=readFileSync(join(repo,'.git/index')),head=git(repo,['rev-parse','HEAD']);
 await connectRealCore(page,join(folder,'stale-lock-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator('.working-row').click();await page.getByText('Recent activity & recovery',{exact:true}).click();await page.getByRole('button',{name:'Recover stale lock…',exact:true}).click();const review=page.getByRole('region',{name:'Review stale lock recovery',exact:true});await expect(review).toContainText('Previous owner PID');await expect(review.getByRole('button',{name:'Archive stale lock',exact:true})).toBeEnabled();expect(readFileSync(lock,'utf8')).toBe(contents);await review.getByRole('button',{name:'Archive stale lock',exact:true}).click();await expect(page.locator('.stale-lock-recovery')).toContainText('Stale lock archived.');expect(existsSync(lock)).toBe(false);expect(readFileSync(join(repo,'.git/index'))).toEqual(index);expect(readFileSync(join(repo,'file.txt'),'utf8')).toBe('working\n');expect(git(repo,['rev-parse','HEAD'])).toBe(head);
 writeFileSync(lock,`GitNova pid=${process.pid}\n`);await page.getByRole('button',{name:'Recover stale lock…',exact:true}).click();await expect(review).toContainText('still running');await expect(review.getByRole('button',{name:'Archive stale lock',exact:true})).toBeDisabled();expect(existsSync(lock)).toBe(true);
});

test('interactive graph replay can fix an invalid plan and combine commits without changing the target',async({page})=>{
 const repo=join(folder,'interactive-ui');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'base'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','base']);git(repo,['checkout','-b','feature']);const commits:string[]=[];for(const name of ['first','second']){writeFileSync(join(repo,name),`${name}\n`);git(repo,['add',name]);git(repo,['commit','-m',name]);commits.push(git(repo,['rev-parse','HEAD']));}git(repo,['checkout','main']);writeFileSync(join(repo,'target'),'target\n');git(repo,['add','target']);git(repo,['commit','-m','target']);const target=git(repo,['rev-parse','HEAD']);git(repo,['checkout','feature']);
 await connectRealCore(page,join(folder,'interactive-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await page.locator(`#commit-${target}`).click({button:'right'});await page.getByRole('menuitem',{name:'Rebase feature onto this commit…',exact:true}).click();await page.getByRole('button',{name:'Customize linear replay…',exact:true}).click();await expect(page.locator('.replay-step')).toHaveCount(2);const first=page.getByLabel(`Replay action for ${commits[0].slice(0,8)}`,{exact:true});await first.selectOption('fixup');await expect(page.getByRole('alert')).toContainText('Fixup needs an earlier kept commit.');await expect(first).toBeVisible();await first.selectOption('pick');await expect(first).toHaveValue('pick');await expect(page.getByRole('alert')).toHaveCount(0);await page.getByLabel(`Replay action for ${commits[1].slice(0,8)}`,{exact:true}).selectOption('fixup');await page.getByRole('checkbox',{name:'I understand rebase changes commit IDs; shared history needs coordination.',exact:true}).check();const start=page.getByRole('button',{name:'Start rebase',exact:true});await expect(start).toBeEnabled();await start.click();await expect.poll(()=>git(repo,['rev-list','--count',`${target}..HEAD`])).toBe('1');expect(git(repo,['rev-parse','main'])).toBe(target);expect(git(repo,['show','-s','--format=%s','HEAD'])).toBe('first');expect(readFileSync(join(repo,'second'),'utf8')).toBe('second\n');
});

test('reviewed worktree move updates the existing workspace location without moving its branch',async({page})=>{
 const repo=join(folder,'move-ui-parent'),linked=join(folder,'move-ui-source'),destination=join(folder,'move-ui-target');mkdirSync(repo);git(repo,['init','-b','main']);writeFileSync(join(repo,'file'),'base\n');git(repo,['add','.']);git(repo,['commit','-m','move fixture']);git(repo,['worktree','add','-b','side',linked]);const tip=git(repo,['rev-parse','side']);
 await connectRealCore(page,join(folder,'move-ui.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(repo);await page.getByRole('button',{name:'Open',exact:true}).click();await expect(page.locator('.repo-picker')).toBeEnabled();await page.keyboard.press('Control+o');const entry=page.locator('.worktree-entry').filter({has:page.locator('small',{hasText:'move-ui-source'})});await entry.getByText('Worktree metadata',{exact:true}).click();await entry.getByLabel('New worktree folder',{exact:true}).fill(destination);await entry.getByRole('button',{name:'Move linked worktree…',exact:true}).click();await expect(entry.getByRole('region',{name:'Review worktree metadata',exact:true})).toContainText(destination);await entry.getByRole('button',{name:'Move reviewed worktree',exact:true}).click();await expect.poll(()=>existsSync(destination)).toBe(true);expect(existsSync(linked)).toBe(false);expect(git(repo,['rev-parse','side'])).toBe(tip);expect(readFileSync(join(destination,'file'),'utf8')).toBe('base\n');
});

test('committed line attribution and root package evidence navigate historical context without checkout',async({page})=>{
 const root=join(folder,'line-app'),lib=join(folder,'registered-library');for(const repo of [root,lib]){mkdirSync(repo);git(repo,['init','-b','main']);}
 writeFileSync(join(lib,'package.json'),JSON.stringify({name:'@fixture/lib',version:'2.1.0'}));git(lib,['add','.']);git(lib,['commit','-m','library package']);
 writeFileSync(join(root,'code.txt'),'first line\n');writeFileSync(join(root,'package.json'),JSON.stringify({name:'@fixture/app',version:'1.0.0',dependencies:{'@fixture/lib':'^1'}}));git(root,['add','.']);git(root,['commit','-m','attribution base']);const base=git(root,['rev-parse','HEAD']);git(root,['branch','release']);git(root,['checkout','-b','develop']);git(root,['mv','code.txt','renamed file.txt']);writeFileSync(join(root,'renamed file.txt'),'first line\nnew\n');writeFileSync(join(root,'package.json'),JSON.stringify({name:'@fixture/app',version:'2.0.0',dependencies:{'@fixture/lib':'^2'}}));git(root,['add','.']);git(root,['commit','-m','package and renamed line']);const tip=git(root,['rev-parse','HEAD']);writeFileSync(join(root,'renamed file.txt'),'private working draft\n');const index=readFileSync(join(root,'.git/index'));
 await connectRealCore(page,join(folder,'line-package.json'));await page.goto('/');await page.getByLabel('Add an existing repository').fill(lib);await page.getByRole('button',{name:'Open',exact:true}).click();await addSearchRepo(page,root);
 await expect(page.getByRole('button',{name:'Search commits…',exact:true})).toBeEnabled();await page.keyboard.press('ControlOrMeta+k');const search=page.getByRole('region',{name:'Commit search',exact:true});await search.getByLabel('Search mode',{exact:true}).selectOption('blame');await expect(search.getByLabel('Commit search scope',{exact:true})).toBeDisabled();await search.getByLabel('File path at revision',{exact:true}).fill('renamed file.txt');await search.getByLabel('End line',{exact:true}).fill('2');await search.getByRole('button',{name:'Search',exact:true}).click();await expect(search.locator('.blame-lines button')).toHaveCount(2);await expect(search.locator('.blame-lines')).toContainText('first line');await expect(search.locator('.blame-lines')).not.toContainText('private working draft');await search.locator('.blame-lines button').last().click();await expect(search).toBeHidden();await expect(page.locator('.dock-file-heading')).toContainText('renamed file.txt');await expect(page.locator('.commit-inspector')).toContainText(tip.slice(0,8));await page.keyboard.press('ControlOrMeta+k');await search.getByRole('button',{name:'Search',exact:true}).click();await expect(search.locator('.blame-lines button')).toHaveCount(2);await search.locator('.blame-lines button').first().click();await expect(search).toBeHidden();await expect(page.locator('.dock-file-heading')).toContainText('code.txt');await expect(page.locator('.commit-inspector')).toContainText(base.slice(0,8));
 await page.keyboard.press('ControlOrMeta+k');await search.getByLabel('Search mode',{exact:true}).selectOption('compare');await search.getByRole('button',{name:'Search',exact:true}).click();const packageEvidence=search.locator('details').filter({has:page.getByText('Root package manifest evidence',{exact:true})});await expect(packageEvidence).toBeVisible();await packageEvidence.locator('summary').click();await expect(packageEvidence).toContainText('2.0.0');await expect(packageEvidence).toContainText('^2 → ^1');await expect(packageEvidence).toContainText('2.1.0');await packageEvidence.getByRole('button').click();await expect(search).toBeHidden();await expect(page.locator('.repo-picker')).toContainText('registered-library');expect(git(root,['rev-parse','HEAD'])).toBe(tip);expect(readFileSync(join(root,'.git/index'))).toEqual(index);expect(readFileSync(join(root,'renamed file.txt'),'utf8')).toBe('private working draft\n');
});
