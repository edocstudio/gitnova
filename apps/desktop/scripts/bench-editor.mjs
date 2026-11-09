// Isolated Chromium editor-layout comparison. No Git operations or real files.
import {chromium} from '@playwright/test';
import {writeFileSync,mkdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {platform,arch,cpus} from 'node:os';
import {execFileSync} from 'node:child_process';
import {splitEditorText} from '../src/working-file-text.ts';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'../../..');
const output=resolve(process.argv[2]??resolve(root,'target/editor-benchmark.json'));
const median=values=>{const sorted=[...values].sort((a,b)=>a-b);return sorted[Math.floor(sorted.length/2)];};
const report={schemaVersion:1,kind:'gitnova-editor-layout-comparison',measuredAt:new Date().toISOString(),sourceCommit:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),candidateWorkingTree:true,environment:{platform:platform(),arch:arch(),cpu:cpus()[0].model,node:process.version},samples:[],limits:['Synthetic editor layout and typing only, not full application or Git-operation latency.','Chromium differs from native macOS WebKit; results do not certify native RAM stability.','JS heap excludes DOM, renderer native allocations and GPU memory. No long-session RAM test.','Baseline reconstructs the previous whole-document editor; candidate uses the current section splitter.','No real repository data, tokens, Git credentials or network services used.']};
const browser=await chromium.launch({headless:true});report.environment.browser=browser.version();
try{
  for(const lines of [15000,40000]){
    const full='setting: ภาษาไทย 😀 value\n'.repeat(lines);
    const sections=splitEditorText(full);
    for(let run=0;run<7;run++)for(const mode of (run%2?['sections','whole-document']:['whole-document','sections'])){
      const context=await browser.newContext({viewport:{width:1360,height:900}});
      const page=await context.newPage();page.setDefaultTimeout(10000);
      const row={mode,run,utf8Bytes:Buffer.byteLength(full),sections:sections.length};
      try{
        await page.setContent('<style>body{margin:0;background:#101c26;color:#ccd}main{display:flex;height:500px}pre,textarea{box-sizing:border-box;width:50%;min-width:0;overflow:auto;white-space:pre;font:14px monospace;flex:1 1 0;contain:strict}textarea{resize:none}</style><main><pre></pre><textarea aria-label="Editor"></textarea></main>');
        row.renderMs=await page.evaluate(async text=>{
          const start=performance.now();document.querySelector('pre').textContent=text;document.querySelector('textarea').value=text;
          await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));return performance.now()-start;
        },mode==='sections'?sections[0]:full);
        const area=page.locator('textarea');await area.focus();
        await area.evaluate(element=>element.setSelectionRange(element.value.length,element.value.length));
        const start=performance.now();await page.keyboard.insertText('x');
        await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
        row.typeAndPaintMs=performance.now()-start;
        const session=await context.newCDPSession(page);await session.send('Performance.enable');
        const {metrics}=await session.send('Performance.getMetrics');row.jsHeapUsedBytes=metrics.find(metric=>metric.name==='JSHeapUsedSize')?.value??null;
        row.renderedCodeUnits=mode==='sections'?sections[0].length:full.length;
        if(await area.inputValue()!==(mode==='sections'?sections[0]:full)+'x')throw new Error('Typing result differs');
      }catch(error){row.error=error.message;}finally{await context.close();}
      report.samples.push(row);console.log(JSON.stringify(row));
    }
  }
}finally{await browser.close();}
report.comparison=[...new Set(report.samples.map(row=>row.utf8Bytes))].map(bytes=>{
  const result={utf8Bytes:bytes};
  for(const mode of ['whole-document','sections']){
    const rows=report.samples.filter(row=>row.utf8Bytes===bytes&&row.mode===mode&&!row.error);
    result[mode]={passed:rows.length,failed:7-rows.length,renderMedianMs:rows.length?median(rows.map(row=>row.renderMs)):null,typeAndPaintMedianMs:rows.length?median(rows.map(row=>row.typeAndPaintMs)):null,jsHeapUsedMedianBytes:rows.length?median(rows.map(row=>row.jsHeapUsedBytes)):null};
  }
  return result;
});
mkdirSync(dirname(output),{recursive:true});writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(`Report: ${output}`);
if(report.samples.some(row=>row.error))process.exitCode=1;
