import {useEffect,useRef,useState} from 'react';
import {api,errorMessage} from './api';
import type {CommitIdentityContext,RepositoryIdentitySettings} from './contracts';
import {t,useLocale} from './i18n';

export function RepositoryIdentityFooter({membershipId,revision,blocked,onEdit}:{membershipId?:string;revision?:string;blocked:boolean;onEdit:()=>void}){
 useLocale();
 const [value,setValue]=useState<{id:string;context:CommitIdentityContext}|null>(null);
 const [failed,setFailed]=useState(false);
 useEffect(()=>{
  let live=true,flight=false,again=false;
  const load=async()=>{if(!membershipId)return;if(flight){again=true;return;}flight=true;
   try{const context=await api.commitIdentity(membershipId);if(live){setValue({id:membershipId,context});setFailed(false);}}
   catch{if(live){setValue(null);setFailed(true);}}
   finally{flight=false;if(live&&again){again=false;void load();}}
  };void load();
  const events=['focus','gitnova-commit-identity','gitnova-write-completed','gitnova-remotes-changed'];events.forEach(event=>window.addEventListener(event,load));
  return()=>{live=false;events.forEach(event=>window.removeEventListener(event,load));};
 },[membershipId,revision]);
 if(!membershipId)return null;
 const context=value?.id===membershipId?value.context:null;
 const identity=context?.identity;
 const source=context?.repository?t('Repository identity'):context?.connection?t('Connection identity · {0}',[context.connection.origin]):t('Using Git configuration');
 return <button className="footer-identity" aria-label={t('Edit repository commit identity')} disabled={blocked} onClick={onEdit} title={identity?`${identity.name} <${identity.email}>\n${source}`:t('Set the name and email for future commits.')}>
  {identity?`${identity.name} <${identity.email}>`:failed?t('Set commit identity'):t('Reading commit identity…')}
 </button>;
}

export function RepositoryIdentityEditor({membershipId,repositoryName,blocked,onBusyChange,focusOnLoad=false}:{focusOnLoad?:boolean;membershipId:string;repositoryName:string;blocked:boolean;onBusyChange:(busy:boolean)=>void}){
 const [settings,setSettings]=useState<RepositoryIdentitySettings|null>(null);
 const nameInput=useRef<HTMLInputElement>(null);const focused=useRef(false);
 useEffect(()=>{if(settings&&focusOnLoad&&!focused.current){focused.current=true;nameInput.current?.focus();}},[settings,focusOnLoad]);
 const [name,setName]=useState(''),[email,setEmail]=useState(''),[error,setError]=useState(''),[saved,setSaved]=useState(false),[busy,setBusy]=useState(false);
 useEffect(()=>{let live=true;void api.repositoryIdentitySettings(membershipId).then(value=>{if(live){setSettings(value);setName(value.name);setEmail(value.email);}}).catch(e=>{if(live)setError(errorMessage(e));});return()=>{live=false;};},[membershipId]);
 const save=async()=>{if(!settings||busy||blocked)return;setBusy(true);onBusyChange(true);setError('');setSaved(false);
  try{await api.setRepositoryCommitIdentity(membershipId,{name:name.trim(),email:email.trim()},settings.token);window.dispatchEvent(new Event('gitnova-commit-identity'));const value=await api.repositoryIdentitySettings(membershipId);setSettings(value);setName(value.name);setEmail(value.email);setSaved(true);}
  catch(e){setError(errorMessage(e));}finally{setBusy(false);onBusyChange(false);}
 };
 return <section className="repository-identity-settings"><h3>{t('Repository commit identity')}</h3><p><strong>{repositoryName}</strong></p>
 <p className="hint">{t('Applies to this repository and its linked worktrees. Saves local Git name and email, overrides the connection identity in GitNova, and leaves other repositories unchanged.')}</p>
 <form aria-label={t('Repository commit identity')} onSubmit={event=>{event.preventDefault();void save();}}>
 <label htmlFor="repository-commit-name">{t('Commit name')}</label><input ref={nameInput} id="repository-commit-name" value={name} maxLength={256} disabled={busy||blocked||!settings} required onChange={event=>{setName(event.target.value);setSaved(false);}}/>
 <label htmlFor="repository-commit-email">{t('Commit email')}</label><input id="repository-commit-email" type="email" value={email} maxLength={254} disabled={busy||blocked||!settings} required onChange={event=>{setEmail(event.target.value);setSaved(false);}}/>
 <p className="hint">{t('Only future commits are affected. Existing authors and Push authentication are unchanged.')}</p>
 <button className={busy?'loading-status primary':'primary'} disabled={busy||blocked||!settings||!name.trim()||!email.trim()}>{busy?t('Saving…'):t('Save for this repository')}</button>
 {!settings&&!error&&<p role="status" className="loading-status">{t('Reading commit identity…')}</p>}{saved&&<p role="status">{t('Repository commit identity saved.')}</p>}{error&&<p role="alert" className="notice">{t(error)}</p>}
 </form></section>;
}
