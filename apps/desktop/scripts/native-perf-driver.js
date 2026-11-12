// Injected only by the explicit, isolated native acceptance performance profile.
// Timings/counts only; no payload contents, source, paths or credentials reported.
(async()=>{
 if(window.__gitnovaPerfRunning)return;window.__gitnovaPerfRunning=true;
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
 const wait=async(predicate)=>{const end=performance.now()+30000;while(performance.now()<end){if(predicate())return;await pause(20);}throw Error('Timed out waiting for fixture UI');};
 const rows=[];
 const sample=async(name,action)=>{const start=performance.now();const value=await action();rows.push({name,milliseconds:performance.now()-start});return value;};
 try{
  await wait(()=>document.querySelector('.commit-row')&&!document.querySelector('.sync-state'));
  const catalog=await invoke('catalog');const workspace=catalog.workspaces.find(w=>w.id===catalog.activeWorkspaceId);const membershipId=workspace.selectedId;
  for(let i=0;i<20;i++){
   const graph=await sample('graph_first_ipc',()=>invoke('graph_page',{membershipId,cursor:null}));
   if(graph.next)await sample('graph_next_ipc',()=>invoke('graph_page',{membershipId,cursor:graph.next}));
   const snapshot=await sample('snapshot_ipc',()=>invoke('repository_snapshot',{membershipId}));
   await sample('file_preview_ipc',()=>invoke('file_preview',{membershipId,path:snapshot.changes.files[0].path,staged:false}));
   await sample('detail_ipc',()=>invoke('commit_detail',{membershipId,oid:graph.rows[i%graph.rows.length].commit.oid}));
   // Actual UI processing/render readiness, after a genuine user selection.
   await sample('detail_ui_dom_ready',async()=>{
    const visible=[...document.querySelectorAll('.commit-row:not(.stash-graph-row)')];const row=visible[(i+1)%Math.min(5,visible.length)];const oid=row.id.slice(7);row.click();
    await wait(()=>document.querySelector('.commit-inspector .commit-identity code')?.textContent===oid.slice(0,8));
   });
   await sample('working_diff_ui_dom_ready',async()=>{
    document.querySelector('.working-row').click();
    await wait(()=>{
     const path=document.querySelector('.working-detail .changed-file.active')?.getAttribute('title');
     return path&&document.querySelector('.dock-body.single-file .dock-file-heading code')?.textContent===path&&document.querySelector('.patch-viewport');
    });
   });
  }
  await invoke('native_soak_report',{sample:{kind:'performance',samples:rows,visibility:document.visibilityState,scope:'Native IPC and selected-commit UI, warm application. DOM readiness does not establish physical screen presentation.'}});
  console.warn('GITNOVA_PERF_COMPLETE');
 }catch(error){await invoke('native_soak_report',{sample:{kind:'performance-failed',message:'Fixture performance driver failed',samples:rows}});console.error('GITNOVA_PERF_FAILED');}
})();
