import {useEffect,useRef,useState} from 'react';
import {api,errorMessage} from './api';
import {connections,normalizeHost,saveConnections} from './connection-store';
import type {Connection} from './connection-store';
import {t} from './i18n';
import {Icon} from './Icons';
export function Connections(){
 const [items,setItems]=useState(connections),[adding,setAdding]=useState(false),[label,setLabel]=useState(''),[provider,setProvider]=useState<'gitlab'|'github'>('gitlab'),[origin,setOrigin]=useState('');
 const itemsRef=useRef(items);itemsRef.current=items;
 const [busy,setBusy]=useState(false),[error,setError]=useState('');const active=useRef(true),flight=useRef(false);
 useEffect(()=>{active.current=true;return()=>{active.current=false;};},[]);
 function save(next:Connection[]){saveConnections(next);itemsRef.current=next;setItems(next);}
 async function add(){if(flight.current)return;flight.current=true;setBusy(true);setError('');try{const context=await api.hostAccount({provider,origin:normalizeHost(origin.trim())});if(!active.current)return;save([...itemsRef.current,{id:crypto.randomUUID(),label:label.trim()||new URL(context.origin).hostname,provider,origin:context.origin,enabled:!!context.account}]);setAdding(false);setLabel('');setOrigin('');}catch(e){if(active.current)setError(errorMessage(e));}finally{flight.current=false;if(active.current)setBusy(false);}}
 return <section className="global-connections" aria-labelledby="connections-title"><h3 id="connections-title">{t('Connections')}</h3><p className="hint">{t('Connect once per company or Git host. Matching repositories share these accounts across all workspaces.')}</p>
  {items.map(item=><ConnectionCard key={item.id} item={item} onSave={value=>save(itemsRef.current.map(v=>v.id===item.id?value:v))} onRemove={()=>save(itemsRef.current.filter(v=>v.id!==item.id))}/>)}
  {error&&<p role="alert" className="notice">{t(error)}</p>}
  {adding?<form className="connection-add" onSubmit={e=>{e.preventDefault();void add();}}>
   <label htmlFor="connection-label">{t('Company or connection name')}</label><input id="connection-label" value={label} maxLength={80} disabled={busy} placeholder={t('Company A')} onChange={e=>setLabel(e.target.value)}/>
   <label htmlFor="connection-provider">{t('Git provider')}</label><select id="connection-provider" disabled={busy} value={provider} onChange={e=>setProvider(e.target.value as 'gitlab'|'github')}><option value="gitlab">GitLab</option><option value="github">GitHub</option></select>
   <label htmlFor="connection-origin">{t('Git host HTTPS address')}</label><input id="connection-origin" value={origin} maxLength={4096} disabled={busy} placeholder="https://git.company.example" onChange={e=>setOrigin(e.target.value)}/>
   <div className="connection-actions"><button type="button" disabled={busy} onClick={()=>setAdding(false)}>{t('Cancel')}</button><button className="primary" disabled={busy||!origin.trim()}>{busy?t('Checking connection…'):t('Add connection')}</button></div>
  </form>:<button className="connection-add-button" disabled={items.length>=20} onClick={()=>setAdding(true)}><Icon name="plus" size={14}/>{t('Add connection')}</button>}
  <p className="hint">{t('Tokens stay in Keychain. Connecting does not fetch, push or trigger builds.')}</p>
  <p className="hint connection-scope">{t('The graph displays CI statuses and checks reported to this Git host, including Jenkins when it reports there. Connecting Jenkins alone does not discover jobs or publish statuses.')}</p>

 </section>;
}
function ConnectionCard({item,onSave,onRemove}:{item:Connection;onSave:(v:Connection)=>void;onRemove:()=>void}){
 const [account,setAccount]=useState<string|null>(null),[jenkinsAccount,setJenkinsAccount]=useState<string|null>(null);
 const [busy,setBusy]=useState(false),[checking,setChecking]=useState(true),[error,setError]=useState('');
 const [editing,setEditing]=useState(false),[controller,setController]=useState(item.jenkins?.controller??''),[username,setUsername]=useState(item.jenkins?.username??'');
 const [confirm,setConfirm]=useState<'host'|'jenkins'|'remove'|null>(null);const live=useRef(true),flight=useRef(false);
 useEffect(()=>{live.current=true;return()=>{live.current=false;};},[]);
 useEffect(()=>{let valid=true;void(async()=>{try{const context=await api.hostAccount(item);if(valid){setAccount(context.account);if(context.account&&!item.enabled)onSave({...item,enabled:true});}if(item.jenkins){const value=await api.jenkinsAccount(item.jenkins.controller,item.jenkins.username);if(valid)setJenkinsAccount(value);}}catch(e){if(valid)setError(errorMessage(e));}finally{if(valid)setChecking(false);}})();return()=>{valid=false;};},[item.provider,item.origin,item.jenkins?.controller,item.jenkins?.username]);
 async function run(task:()=>Promise<void>){if(flight.current)return;flight.current=true;setBusy(true);setError('');try{await task();}catch(e){if(live.current)setError(errorMessage(e));}finally{flight.current=false;if(live.current)setBusy(false);}}
 return <article className="connection-card" aria-label={item.label}>
  <div className="connection-title"><div><strong>{item.label}</strong><span>{item.provider==='gitlab'?'GitLab':'GitHub'} · {item.origin}</span></div><button className="icon-button" aria-label={t('Remove connection {0}',[item.label])} disabled={busy} onClick={()=>setConfirm('remove')}><Icon name="trash" size={15}/></button></div>
  <div className="connection-service"><div><strong>{t('Git authentication · PR/MR · CI')}</strong><p className="hint">{checking?t('Checking saved accounts…'):account?t('Account saved: {0}',[account]):t('Not connected')}</p></div>
   <div className="connection-actions"><button disabled={busy||checking} onClick={()=>void run(async()=>{const value=await api.connectHost(item);if(live.current){onSave({...item,origin:value.origin,enabled:true});setAccount(value.account);}})}>{busy?t('Connecting…'):account?t('Reconnect account…'):t('Connect')}</button>{account&&<button disabled={busy} onClick={()=>setConfirm('host')}>{t('Disconnect')}</button>}</div>
  </div>
  <p className="hint">{t("The saved host account also authenticates HTTPS Git operations. Allow Keychain access when macOS asks. SSH uses your existing agent.")}</p>
  <div className="connection-service jenkins-global"><div><strong>Jenkins</strong><p className="hint">{item.jenkins?.controller??t('Optional Jenkins controller for this company')}{jenkinsAccount&&<><br/>{t('Account saved: {0}',[jenkinsAccount])}</>}</p></div><button disabled={busy} onClick={()=>setEditing(!editing)}>{jenkinsAccount?t('Manage connection'):t('Connect Jenkins…')}</button></div>
  {editing&&<form className="jenkins-account-form" onSubmit={e=>{e.preventDefault();void run(async()=>{const url=normalizeHost(controller.trim());const login=await api.connectJenkinsAccount(url,username.trim());if(live.current){onSave({...item,jenkins:{controller:url,username:username.trim()}});setJenkinsAccount(login);setEditing(false);}});}}>
   <label htmlFor={`controller-${item.id}`}>{t('Jenkins controller URL')}</label><input id={`controller-${item.id}`} value={controller} maxLength={4096} disabled={busy} onChange={e=>setController(e.target.value)} placeholder="https://jenkins.company.example"/>
   <label htmlFor={`username-${item.id}`}>{t('Jenkins username')}</label><input id={`username-${item.id}`} value={username} maxLength={256} disabled={busy} autoComplete="off" onChange={e=>setUsername(e.target.value)}/>
   <p className="hint">{t('This Jenkins account is shared by repositories on this Git host. No repository, branch or job is required to connect.')}</p>
   <div className="connection-actions"><button type="button" disabled={busy} onClick={()=>setEditing(false)}>{t('Cancel')}</button><button disabled={busy||!controller.trim()||!username.trim()}>{busy?t('Connecting…'):t('Connect Jenkins account…')}</button>{jenkinsAccount&&<button type="button" disabled={busy} onClick={()=>setConfirm('jenkins')}>{t('Disconnect Jenkins')}</button>}</div>
  </form>}
  {confirm&&<div className="connection-confirm"><p>{confirm==='remove'?t('Remove this automatic host mapping? Saved accounts remain in Keychain and can still authenticate HTTPS Git.'):t('Disconnect this saved account for all repositories? It is shared by HTTPS Git, PR/MR and CI. Existing Git helpers and SSH agents remain independent.')}</p><div className="connection-actions"><button disabled={busy} onClick={()=>setConfirm(null)}>{t('Cancel')}</button><button className="danger" disabled={busy} onClick={()=>void run(async()=>{if(confirm==='remove'){onRemove();return;}if(confirm==='host'){await api.disconnectHost(item);if(live.current){setAccount(null);onSave({...item,enabled:false});}}else if(item.jenkins){await api.disconnectJenkinsAccount(item.jenkins.controller,item.jenkins.username);if(live.current){setJenkinsAccount(null);onSave({...item,jenkins:undefined});}}if(live.current)setConfirm(null);})}>{confirm==='remove'?t('Remove connection'):t('Disconnect')}</button></div></div>}
  {busy&&<p className="operation-wait" role="status"><span className="loading-spin">↻</span>{t('Connecting…')}</p>}{error&&<p role="alert" className="notice">{t(error)}</p>}
 </article>;
}
