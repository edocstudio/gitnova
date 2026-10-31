import {spawnSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {resolve,dirname} from 'node:path';
const here=dirname(fileURLToPath(import.meta.url));
const manifest=readFileSync(resolve(here,'../../../Cargo.toml'),'utf8');
const nodeRange=JSON.parse(readFileSync(resolve(here,'../package.json'),'utf8')).engines.node;
const nodeBounds=nodeRange.match(/^\^(\d+\.\d+\.\d+) \|\| >=(\d+\.\d+\.\d+)$/);
if(!nodeBounds)throw new Error('Unsupported Node engine range in package.json; update the prerequisite checker.');
const minimumRust=manifest.match(/^rust-version\s*=\s*"([\d.]+)"/m)?.[1];
if(!minimumRust)throw new Error('Cannot read workspace Rust requirement.');
const atLeast=(version,minimum)=>{
 const a=version.split('.').map(Number),b=minimum.split('.').map(Number);
 for(let i=0;i<3;i++){if((a[i]??0)!==(b[i]??0))return (a[i]??0)>(b[i]??0);}
 return true;
};
export function checkPrerequisites({nodeVersion=process.versions.node,platform=process.platform,probe=(command,args)=>spawnSync(command,args,{encoding:'utf8',timeout:8000,maxBuffer:64*1024,env:{...process.env,RUSTUP_AUTO_INSTALL:'0'}})}={}){
 const results=[];
 const nodeOK=(nodeVersion.split('.')[0]===nodeBounds[1].split('.')[0]&&atLeast(nodeVersion,nodeBounds[1]))||atLeast(nodeVersion,nodeBounds[2]);
 results.push({ok:nodeOK,label:`Node.js ${nodeVersion}`,help:`Requires Node ${nodeRange}. From the repo root, nvm install && nvm use (if using nvm).`});
 for(const command of ['rustc','cargo','git']){
  const result=probe(command,['--version']);
  const output=String(result.stdout??'').trim();
  const version=output.match(/\b(\d+\.\d+\.\d+)\b/)?.[1];
  const available=!result.error&&result.status===0&&!!version;
  const rust=command!=='git';
  const ok=available&&(rust?(atLeast(version,minimumRust)&&!/-nightly|-beta/.test(output)):atLeast(version,'2.38.0'));
  results.push({ok,label:output||`${command}: ${result.error?.code==='ETIMEDOUT'?'version check timed out':'not available in PATH'}`,help:rust?`Install stable Rust ${minimumRust}+ with Cargo: https://rust-lang.org/tools/install/ . Reopen Terminal after installation. If installed via rustup, check that ~/.cargo/bin (Windows: %USERPROFILE%\\.cargo\\bin) is in PATH; on macOS/Linux you can source "$HOME/.cargo/env". Update an older rustup toolchain with rustup update stable.`:'Install Git 2.38+ (merge previews require merge-tree --write-tree) and confirm git --version works in this Terminal: https://git-scm.com/downloads/'});
 }
 if(platform==='darwin'){
  const sdk=probe('xcrun',['--show-sdk-path']);
  results.push({ok:!sdk.error&&sdk.status===0&&!!String(sdk.stdout??'').trim(),label:'macOS SDK / Command Line Tools',help:'For desktop development, install the Apple Command Line Tools with xcode-select --install, then finish the installer. See https://v2.tauri.app/start/prerequisites/#macos'});
 }
 if(platform==='linux'){
  for(const library of ['webkit2gtk-4.1','gtk+-3.0','dbus-1','openssl']){
   const result=probe('pkg-config',['--exists',library]);
   results.push({ok:!result.error&&result.status===0,label:`Linux ${library} development library`,help:'Install the Linux packages in docs/platforms.md, including libdbus-1-dev and libwebkit2gtk-4.1-dev.'});
  }
 }
 if(platform==='win32'){
  const result=probe('powershell.exe',['-NoLogo','-NoProfile','-NonInteractive','-Command',"[Environment]::OSVersion.Version.ToString()"]);
  results.push({ok:!result.error&&result.status===0,label:'Windows PowerShell',help:'Windows desktop requires PowerShell, Microsoft C++ Build Tools (MSVC) and WebView2. See docs/platforms.md.'});
 }
 return results;
}
export function reportPrerequisites(options){
 const results=checkPrerequisites(options);
 for(const item of results){console.log(`${item.ok?'OK':'MISSING'}  ${item.label}`);if(!item.ok)console.error(`  ${item.help}`);}
 const ok=results.every(item=>item.ok);
 console.log(ok?'Basic source-build tools are available.':'GitNova source build cannot start until the checks above pass.');
 console.log('Platform libraries/build tools must also be installed: https://v2.tauri.app/start/prerequisites/');
 console.log('This check does not install tools, compile code or verify every platform dependency. An installed GitNova desktop app does not require Rust/Cargo/Node; running from source does.');
 return ok;
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url))process.exitCode=reportPrerequisites()?0:1;
