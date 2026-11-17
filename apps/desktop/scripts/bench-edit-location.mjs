// Isolated pure-function measurements, not native UI latency or a RAM guarantee.
import {readFileSync,writeFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {stripTypeScriptTypes} from 'node:module';
import {fileURLToPath} from 'node:url';
import {resolve,dirname,join} from 'node:path';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../../..');
const args=process.argv.slice(2);const baselineRef=args[0];const output=args[1];
if(!baselineRef||!output)throw new Error('Pass baseline commit/ref and JSON output path.');
const baselineCommit=execFileSync('git',['rev-parse','--verify','--end-of-options',`${baselineRef}^{commit}`],{cwd:root,encoding:'utf8'}).trim();
const file='apps/desktop/src/edit-location.ts';
const before=execFileSync('git',['show',`${baselineCommit}:${file}`],{cwd:root,encoding:'utf8'});
const after=readFileSync(join(root,file),'utf8');
const load=async(source)=>import('data:text/javascript;base64,'+Buffer.from(stripTypeScriptTypes(source)).toString('base64'));
const old=await load(before),current=await load(after);
const cases=[['unique-20k',Array.from({length:20000},(_,i)=>`setting_${i}: configuration ${i}`).join('\n'),{line:15000,text:'setting_14999: configuration 14999',before:['setting_14998: configuration 14998'],after:['setting_15000: configuration 15000']}],
 ...[128*1024,1024*1024,4*1024*1024].map(bytes=>[`repeated-${bytes}`,'value\n'.repeat(Math.floor(bytes/6)),{line:Math.floor(bytes/12),text:'value',before:['value','value','value'],after:['value','value','value']}]),
 ['repeated-4MiB-no-match','value\n'.repeat(Math.floor(4*1024*1024/6)),{line:300000,text:'missing',before:['missing'],after:['missing']}]];
const measure=(fn,text,anchor)=>{const samples=[];let offset;for(let n=0;n<3;n++){const start=performance.now();offset=fn(text,anchor);samples.push(performance.now()-start);}const sorted=[...samples].sort((a,b)=>a-b);return {samplesMs:samples,medianMs:sorted[1],offset};};
const results=cases.map(([name,text,anchor])=>{const before=measure(old.editorOffset,text,anchor),after=measure(current.editorOffset,text,anchor);if(before.offset!==after.offset)throw new Error(`Location changed in ${name}`);return {name,utf16Units:text.length,before,after};});
const digest=value=>createHash('sha256').update(value).digest('hex');
const report={baselineCommit,candidateSourceSha256:digest(after),baselineSourceSha256:digest(before),node:process.version,platform:process.platform,arch:process.arch,samples:3,limitations:['Synthetic pure-function timings in Node, not native WebView or full UI timings.','Sequential blocks; OS scheduling/thermal state are uncontrolled.','No process memory or physical presentation latency claim.'],results};
writeFileSync(resolve(output),JSON.stringify(report,null,2)+'\n');
for(const row of results)console.log(`${row.name}: ${row.before.medianMs.toFixed(1)} -> ${row.after.medianMs.toFixed(1)} ms`);
