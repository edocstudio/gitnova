import {expect,it} from 'vitest';
import {terminalWelcome} from './terminal-welcome';
import {version} from '../package.json';
it('renders the original GitNova greeting and actual version, with a narrow layout',()=>{
  const wide=terminalWelcome(80,'Git made simple.');const narrow=terminalWelcome(30,'Git made simple.');
  expect(wide).toContain(`GitNova ${version} · edocstudio`);expect(wide).toContain('Git made simple.');
  expect(wide.split('\r\n').length).toBeGreaterThan(narrow.split('\r\n').length);
  expect(narrow).toContain('GitNova');
  expect(wide).toContain('nova --help');
});
it('does not accept control sequences from the localized greeting',()=>{
  expect(terminalWelcome(80,'hello\x1b]52;c;payload\x07\r\nworld')).not.toContain('\x1b]52');
  expect(terminalWelcome(80,'hello\nworld')).toContain('helloworld');
});
