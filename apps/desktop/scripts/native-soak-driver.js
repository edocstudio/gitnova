// Injected only into the isolated debug native-soak profile. Reviewed branch writes target only its clean large-b fixture.
(async()=>{
 if(window.__gitnovaSoakRunning)return;window.__gitnovaSoakRunning=true;
 // Foreground mode must not silently add fetch writes to a read/branch-switch
 // protocol. This storage belongs only to the isolated incognito fixture.
 const fetchKey='gitnova.auto-fetch-seconds';localStorage.setItem(fetchKey,'0');
 window.dispatchEvent(new StorageEvent('storage',{key:fetchKey,newValue:'0'}));
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
 const wait=async(predicate,label,ms=90000)=>{const end=Date.now()+ms;while(Date.now()<end){const value=predicate();if(value)return value;await pause(100);}throw Error(`Timeout: ${label}`);};
 const input=(node,value)=>{Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(node,value);node.dispatchEvent(new Event('input',{bubbles:true}));};
 const report=sample=>invoke('native_soak_report',{sample});
 const requireVisible=()=>{if(window.__gitnovaSoakRequireVisible&&document.visibilityState!=='visible')throw Error('Foreground soak lost visible WebView; no foreground acceptance is claimed.');};
 const timings={};const measure=async(name,action)=>{await report({kind:'phase',cycle,name,visibility:document.visibilityState});const start=performance.now();const value=await action();(timings[name]??=[]).push(performance.now()-start);return value;};
 const start=performance.now();let cycle=0;
 try{
  await wait(()=>document.querySelector('.repo-picker .picker-name'),'initial graph');
  if(window.__gitnovaSoakRequireVisible)await wait(()=>document.visibilityState==='visible','visible foreground WebView',30000);

  const catalog=await invoke('catalog');const workspace=catalog.workspaces.find(w=>w.id===catalog.activeWorkspaceId);const memberships=workspace.memberships;
  await report({kind:'ready',locations:memberships.length,auto_fetch_seconds:Number(localStorage.getItem(fetchKey))});
  while(performance.now()-start<window.__gitnovaSoakSeconds*1000){
   requireVisible();
   let target=cycle%3===0 ? memberships.find(m=>m.name==='large-a')??memberships[0] : memberships[cycle%memberships.length];
   if(document.querySelector('.repo-picker .picker-name')?.textContent===target.name)target=memberships[(memberships.indexOf(target)+1)%memberships.length];
   await measure('switch_repository_ui',async()=>{
    const picker=document.querySelector('.repo-picker');if(picker.disabled)await wait(()=>!picker.disabled,'workspace enabled');picker.click();
    const query=await wait(()=>document.querySelector('.repo-filter input'),'repository search');input(query,target.name);
    const option=await wait(()=>Array.from(document.querySelectorAll('.repo-option-main')).find(b=>b.querySelector('strong')?.textContent===target.name&&!b.disabled),'matching repository');option.click();
    await wait(()=>document.querySelector('.repo-picker .picker-name')?.textContent===target.name&&document.querySelector('.commit-row')&&!document.querySelector('.sync-state'),'repository graph');
   });
   // The console may be locked overnight. Exercise the existing manual Refresh
   // action instead of pretending the hidden WebView is foreground/visible.
   await measure('manual_refresh_ui',async()=>{const refresh=await wait(()=>{const b=document.querySelector('.topbar .refresh');return b&&!b.disabled?b:null;},'refresh enabled');refresh.click();await pause(200);await wait(()=>!document.querySelector('.topbar .refresh')?.disabled&&!document.querySelector('.sync-state'),'refresh complete');});
   const snap=await measure('snapshot_ipc',()=>invoke('repository_snapshot',{membershipId:target.id}));
   await measure('detail_ui',async()=>{
    const rows=document.querySelectorAll('.commit-row:not(.stash-graph-row)');const selected=rows[Math.min(rows.length-1,cycle%5)];const oid=selected?.id.slice(7);selected?.click();
    await wait(()=>document.querySelector('.commit-inspector .commit-identity code')?.textContent===oid?.slice(0,8),'commit details');
    const files=document.querySelectorAll('.dock-files button');if(files.length>1){const name=files[1].title;files[1].click();await wait(()=>document.querySelector('.dock-file-heading code')?.textContent===name&&!document.querySelector('.dock-patch [role="status"]'),'selected historical patch');}
   });
   if(cycle%3===0){await measure('older_graph_ui',async()=>{const more=document.querySelector('.load-more');if(more&&!more.disabled){more.click();await pause(100);await wait(()=>!document.querySelector('.load-more')?.disabled&&!document.querySelector('.sync-state'),'older graph page');}});}
   const history=document.querySelector('.history-scroll');if(history){history.scrollTop=cycle%2?1400:0;history.dispatchEvent(new Event('scroll'));await pause(100);}
   if(cycle%4===0){document.querySelector('.working-row')?.click();await pause(200);if(snap.changes.files.length)await wait(()=>document.querySelector('.dock-file-heading'),'working diff');}
   if(cycle%6===5){
    await measure('workspace_search_ipc',()=>invoke('search_commits',{request:{workspaceId:workspace.id,currentMembershipId:target.id,scope:'workspace',selectedIds:[],query:'needle',cursor:null}}));
    await measure('search_ui_current',async()=>{document.querySelector('.commit-search-trigger').click();const query=await wait(()=>document.querySelector('#commit-search input'),'commit search');input(query,'needle');await wait(()=>document.querySelector('.commit-search-result')&&document.querySelector('.commit-search-results')?.getAttribute('aria-busy')==='false','search results',120000);document.querySelector('.commit-search-trigger').click();});
   }
   if(cycle===0||cycle%12===11){
    const clean=memberships.find(m=>m.name==='large-b');
    await measure('branch_roundtrip_review_apply_ipc',async()=>{
     for(const branch of ['soak-switch','main']){
      const preview=await invoke('preview_branch_switch',{membershipId:clean.id,branch});if(preview.blockedReason)throw Error(preview.blockedReason);
      const receipt=await invoke('switch_branch',{membershipId:clean.id,branch,token:preview.token});if(receipt.status!=='succeeded')throw Error(`Branch roundtrip: ${receipt.status}`);
     }
    });
   }
   requireVisible();await report({kind:'cycle',cycle:++cycle,elapsed_ms:performance.now()-start,metrics:timings,visibility:document.visibilityState,dom_nodes:document.getElementsByTagName('*').length,visible_rows:document.querySelectorAll('.commit-row').length});
   for(const key of Object.keys(timings))timings[key]=[];
   await pause(8000);
  }
  await report({kind:'complete',cycle,elapsed_ms:performance.now()-start});
 }catch(e){await report({kind:'failed',cycle,visibility:document.visibilityState,error:String(e).slice(0,250),elapsed_ms:performance.now()-start});}
})();
