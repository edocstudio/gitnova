import {expect,it} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import {DiffView} from './DiffView';
it('mounts code only for initially open sections while keeping the complete file manifest',()=>{
 const diff=Array.from({length:200},(_,i)=>`diff --git a/file-${i}.txt b/file-${i}.txt\n--- a/file-${i}.txt\n+++ b/file-${i}.txt\n@@ -1 +1 @@\n-old-${i}\n+new-${i}\n`).join('');
 const html=renderToStaticMarkup(<DiffView diff={diff}/>);
 expect(html.match(/class="patch-file"/g)).toHaveLength(200);
 expect(html.match(/class="code-scroll"/g)).toHaveLength(3);
 expect(html).toContain('file-199.txt');expect(html).not.toContain('new-199');
 expect(html).toContain('new-2');
});
