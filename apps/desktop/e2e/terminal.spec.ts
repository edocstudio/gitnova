import {test,expect} from '@playwright/test';
// Renderer/lifecycle checks use a bounded emulated PTY transport. Real shell cwd,
// input, EOF and membership checks live in the native terminal tests.
test('terminal dock keeps sessions separate across hide and repository switching',async({page},testInfo)=>{
 await page.goto('/');
 await page.evaluate(async()=>{
  const reactModule=await import('/node_modules/.vite/deps/react.js');const React=reactModule.default??reactModule;
  const domModule=await import('/node_modules/.vite/deps/react-dom_client.js');const {createRoot}=domModule.default??domModule;
  const {RepositoryTerminal}=await import('/src/RepositoryTerminal.tsx');
  const {api}=await import('/src/api.ts');
  const records:{opens:string[];writes:string[];closes:string[];resizes:number}= {opens:[],writes:[],closes:[],resizes:0};
  (window as unknown as {terminalRecords:typeof records}).terminalRecords=records;
  const buffers=new Map<string,number[]>();
  api.terminalOpen=async(id:string)=>{records.opens.push(id);buffers.set(id,Array.from(new TextEncoder().encode(`Ready ${id}\r\n`)));return{id,path:`/fixture/${id}`,shell:'/bin/sh'};};
  api.terminalRead=async(id:string)=>{const data=buffers.get(id)??[];buffers.set(id,[]);return{data,dropped:false,exited:false};};
  api.terminalWrite=async(id:string,_session:string,data:string)=>{records.writes.push(`${id}:${data}`);buffers.set(id,[...(buffers.get(id)??[]),...new TextEncoder().encode(data)]);};
  api.terminalResize=async()=>{records.resizes++;};
  api.terminalClose=async(id:string)=>{records.closes.push(id);};
  const host=document.createElement('div');host.id='terminal-fixture';host.style.cssText='position:fixed;bottom:0;left:0;right:0;z-index:100';document.body.append(host);
  function Fixture(){
   const [id,setId]=React.useState('alpha');const [entries,setEntries]=React.useState([{membershipId:'alpha',name:'Alpha'}]);const [visible,setVisible]=React.useState(true);
   return React.createElement('div',{},React.createElement('button',{onClick:()=>setVisible(true)},'Show fixture terminal'),React.createElement('button',{onClick:()=>{setId('beta');setEntries(old=>old.some(e=>e.membershipId==='beta')?old:[...old,{membershipId:'beta',name:'Beta'}]);setVisible(true);}},'Switch fixture beta'),React.createElement('button',{onClick:()=>{setId('alpha');setVisible(true);}},'Switch fixture alpha'),React.createElement(RepositoryTerminal,{entries,selectedId:id,visible,onHide:()=>setVisible(false),onEnd:(ended:string)=>setEntries(old=>old.filter(e=>e.membershipId!==ended)),onChanged:()=>{}}));
  }
  createRoot(host).render(React.createElement(Fixture));
 });
 const dock=page.locator('#terminal-fixture .repository-terminal');
 await expect(dock).toBeVisible();await expect(dock).toContainText('/fixture/alpha');
 const input=()=>dock.locator('.repository-shell:visible .xterm-helper-textarea');
 await input().pressSequentially('echo alpha');await input().press('Enter');
 await expect.poll(()=>page.evaluate(()=>(window as any).terminalRecords.writes.join(''))).toContain('alpha:');
 await page.getByRole('button',{name:'Hide terminal',exact:true}).click();await expect(dock).toBeHidden();
 await page.getByRole('button',{name:'Show fixture terminal'}).click();await expect(dock).toBeVisible();
 await expect.poll(()=>page.evaluate(()=>(window as any).terminalRecords.opens)).toEqual(['alpha']);
 await page.getByRole('button',{name:'Switch fixture beta'}).click();await expect(dock).toContainText('/fixture/beta');await input().pressSequentially('beta');
 await expect.poll(()=>page.evaluate(()=>(window as any).terminalRecords.writes.join(''))).toContain('beta:');
 await page.getByRole('button',{name:'Switch fixture alpha'}).click();await expect(dock).toContainText('/fixture/alpha');
 await dock.getByRole('separator',{name:'Resize terminal'}).focus();await page.keyboard.press('ArrowUp');await expect(dock.getByRole('separator')).toHaveAttribute('aria-valuenow','280');
 await page.screenshot({path:testInfo.outputPath('repository-terminal.png')});
 await dock.getByRole('button',{name:'End session',exact:true}).click();await expect(dock).toContainText('End this shell');
 await dock.locator('.terminal-end-review').getByRole('button',{name:'End session',exact:true}).click();await expect(dock).toBeHidden();
 await expect.poll(()=>page.evaluate(()=>(window as any).terminalRecords.closes)).toEqual(['alpha']);
});
