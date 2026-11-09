import {version} from '../package.json';

const wordmark=[
  '  ____ _ _   _   _                 ',
  ' / ___(_) |_| \\ | | _____   ____ _ ',
  '| |  _| | __|  \\| |/ _ \\ \\ / / _` |',
  '| |_| | | |_| |\\  | (_) \\ V / (_| |',
  ' \\____|_|\\__|_| \\_|\\___/ \\_/ \\__,_|',
];
export function terminalWelcome(columns:number,welcome:string):string{
  const blue='\x1b[38;2;76;154;255m',cyan='\x1b[38;2;80;200;210m',reset='\x1b[0m';
  // These strings are display-only. Remove controls even from translation input.
  const clean=welcome.replace(/[\x00-\x1f\x7f-\x9f]/g,'');
  const heading=columns>=50?wordmark.join('\r\n'):'GitNova';
  return `\r\n${blue}${heading}${reset}\r\n${cyan}  GitNova ${version} · edocstudio${reset}\r\n  ${clean}\r\n  nova --help\r\n\r\n`;
}
