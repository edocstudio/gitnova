import {useEffect,useRef,useState} from 'react';
import type {Catalog,Workspace} from './contracts';
import {api,errorMessage} from './api';
import {t,useLocale} from './i18n';

export function WorkspaceSelector({workspace,workspaces,blocked,draft,onDraft,onSelect,onCatalog,onBusyChange}:{workspace:Workspace;workspaces:Workspace[];blocked:boolean;draft:string|undefined;onDraft:(name:string|undefined)=>void;onSelect:(id:string)=>void;onCatalog:(value:Catalog)=>void;onBusyChange:(busy:boolean)=>void}){
 useLocale();
 const editing=draft!==undefined,name=draft??workspace.name;
 const [pending,setPending]=useState(false),[problem,setProblem]=useState(''),[saved,setSaved]=useState(false);
 const input=useRef<HTMLInputElement>(null),trigger=useRef<HTMLButtonElement>(null),flight=useRef(false),restoreFocus=useRef(false),mounted=useRef(true);
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 useEffect(()=>{if(editing){input.current?.focus();input.current?.select();}},[editing]);
 useEffect(()=>{if(restoreFocus.current&&!editing&&!blocked&&!pending){restoreFocus.current=false;trigger.current?.focus();}},[editing,blocked,pending]);
 const cancel=()=>{if(flight.current)return;onDraft(undefined);setProblem('');trigger.current?.focus();};
 const trimmed=name.trim();const valid=Array.from(trimmed).length>0&&Array.from(trimmed).length<=80;
 const save=async()=>{
  if(blocked||flight.current||!valid||trimmed===workspace.name)return;
  flight.current=true;setPending(true);setProblem('');onBusyChange(true);
  try{
   const catalog=await api.renameWorkspace(workspace.id,name);
   onDraft(undefined);onCatalog(catalog);
   if(mounted.current){restoreFocus.current=true;setSaved(true);}
  }catch(error){if(mounted.current)setProblem(errorMessage(error));}
  finally{flight.current=false;if(mounted.current)setPending(false);onBusyChange(false);}
 };
 return <div className="workspace-selector">
  <label className="field-label" htmlFor="workspace">{t('Workspace')}</label>
  <div className="workspace-selector-row"><select id="workspace" value={workspace.id} disabled={blocked||pending||editing} onChange={event=>onSelect(event.target.value)}>{workspaces.map(entry=><option key={entry.id} value={entry.id}>{entry.name}</option>)}</select>
   <button ref={trigger} type="button" disabled={blocked||pending} aria-expanded={editing} onClick={()=>{if(editing){input.current?.focus();return;}setProblem('');setSaved(false);onDraft(workspace.name);}}>{t('Rename workspace')}</button></div>
  {editing&&<form className="workspace-rename" aria-busy={pending} aria-label={t('Rename workspace')} onSubmit={event=>{event.preventDefault();void save();}} onKeyDown={event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();cancel();}}}>
   <label className="field-label" htmlFor="workspace-rename-name">{t('Workspace name')}</label>
   <input ref={input} id="workspace-rename-name" value={name} disabled={blocked||pending} aria-invalid={!valid} aria-describedby={!valid?'workspace-rename-validation':undefined} onChange={event=>onDraft(event.target.value)} autoComplete="off"/>
   {!valid&&<p id="workspace-rename-validation" className="hint">{t('Workspace name must contain 1–80 characters')}</p>}
   <div className="workspace-rename-buttons"><button className="primary" disabled={blocked||pending||!valid||trimmed===workspace.name}>{t(pending?'Saving workspace…':'Save workspace name')}</button><button type="button" disabled={pending} onClick={cancel}>{t('Cancel')}</button></div>
   {problem&&<p className="inline-error" role="alert">{t(problem)}</p>}
  </form>}
  {saved&&!editing&&<p className="hint" role="status">{t('Workspace renamed.')}</p>}
 </div>;
}
