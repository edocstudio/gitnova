// Explicit acceptance mode only; the native host validates the disposable catalog.
(async()=>{
 if(window.__gitnovaWorkflowsRunning)return;window.__gitnovaWorkflowsRunning=true;
 const fetchKey='gitnova.auto-fetch-seconds';localStorage.setItem(fetchKey,'0');window.dispatchEvent(new StorageEvent('storage',{key:fetchKey,newValue:'0'}));
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
 const wait=async(predicate,label)=>{const end=Date.now()+45000;while(Date.now()<end){const value=predicate();if(value)return value;await pause(50);}throw Error(`Timeout: ${label}`);};
 const button=async(label,root=document)=>wait(()=>[...root.querySelectorAll('button')].find(b=>b.textContent.trim()===label&&!b.disabled),label);
 const click=async(label,root)=>{(await button(label,root)).click();};
 const input=(node,value)=>{Object.getOwnPropertyDescriptor(node.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype,'value').set.call(node,value);node.dispatchEvent(new Event('input',{bubbles:true}));};
 const report=sample=>invoke('native_soak_report',{sample});const checks=[];
 try{
  await wait(()=>document.querySelector('.commit-row')&&!document.querySelector('.sync-state'),'initial graph');
  await wait(()=>document.visibilityState==='visible','visible native WebView');
  const catalog=await invoke('catalog'),workspace=catalog.workspaces.find(w=>w.id===catalog.activeWorkspaceId),parent=workspace.memberships.find(m=>m.name==='workflow-parent');
  const head=(await invoke('graph_page',{membershipId:parent.id,cursor:null})).rows[0].commit.oid;
  await wait(()=>{const b=document.querySelector('.push-current');return b&&!b.disabled?b:null;},'parent Push').then(b=>b.click());
  input(await wait(()=>document.querySelector('#manage-destination'),'parent destination'),'review/native');
  await click('Check destination & review');
  const childButton=await wait(()=>[...document.querySelectorAll('.push-submodule-help button')].find(b=>b.textContent.includes('Review branch at required commit')&&!b.disabled),'required child commit');
  childButton.click();await wait(()=>document.querySelector('.branch-current')?.textContent.includes('Detached'),'child context');
  input(await wait(()=>document.querySelector('#graph-ref-name'),'branch name'),'review/native-child');await click('Review branch');await click('Create & open review/native-child');
  await wait(()=>!document.querySelector('.graph-actions'),'child branch created');
  (await wait(()=>{const b=document.querySelector('.push-current');return b&&!b.disabled?b:null;},'child Push')).click();await click('Check destination & review');await click('Push reviewed branch');await wait(()=>!document.querySelector('.ref-manager'),'child published');
  await click('Return to workflow-parent Push');await wait(()=>document.querySelector('#manage-destination')?.value==='review/native','restored parent destination');
  await click('Check destination & review');await click('Push reviewed branch');await wait(()=>!document.querySelector('.ref-manager'),'parent published');checks.push('reviewed-child-publication-and-parent-return');
  // Endpoint migration uses the visible dialog and confirmation, not direct IPC writes.
  (await wait(()=>document.querySelector('.footer-remote'),'remote footer')).click();
  const fetch=await wait(()=>document.querySelector('#endpoint-fetch')&&!document.querySelector('#endpoint-fetch').disabled&&document.querySelector('#endpoint-fetch'),'remote URLs');
  const newUrl=fetch.value.replace('parent.git','moved-parent.git');if(newUrl===fetch.value)throw Error('Unexpected fixture endpoint');input(fetch,newUrl);await click('Review remote change');await click('Save remote URLs');await wait(()=>!document.querySelector('.remote-endpoint-dialog'),'endpoint saved');checks.push('reviewed-endpoint-migration');
  const row=await wait(()=>[...document.querySelectorAll('.commit-row')].find(r=>r.textContent.includes('Native distant change')),'historical commit');row.click();
  await wait(()=>document.querySelector('.patch-viewport'),'historical diff');await click('Unified');
  const scroll=await wait(()=>document.querySelector('.dock-code-scroll'),'diff viewport');scroll.scrollTop=scroll.scrollHeight;scroll.dispatchEvent(new Event('scroll'));await pause(100);
  await click('Edit file');const editor=await wait(()=>document.querySelector('.working-file-editor textarea'),'editable file');
  await wait(()=>document.activeElement===editor&&editor.value.slice(editor.selectionStart).startsWith('setting_5102: "native change"'),'diff caret reveal');
  input(editor,editor.value.replace('native change','native corrected'));await click('Save working file');await wait(()=>!document.querySelector('.working-file-editor'),'file saved');checks.push('distant-diff-editor-reveal-and-save');
  await report({kind:'workflows-complete',checks,visibility:document.visibilityState,head,scope:'Visible macOS WebView DOM actions through native IPC and real disposable Git repositories; no physical-screen or credential acceptance claim.'});
 }catch(error){await report({kind:'workflows-failed',checks,error:String(error).slice(0,300),visibility:document.visibilityState});}
})();
