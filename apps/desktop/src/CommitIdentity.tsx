import {useEffect,useState} from 'react';
import {api,errorMessage} from './api';
import {t} from './i18n';
import type {HostProfile,GitCommitIdentity,CommitIdentityContext} from './contracts';
export function ConnectionIdentity({profile,blocked=false,onBusyChange}:{profile:HostProfile;blocked?:boolean;onBusyChange?:(busy:boolean)=>void}) {
 const [name,setName]=useState(''),[email,setEmail]=useState(''),[saved,setSaved]=useState<GitCommitIdentity|null>(null),[loading,setLoading]=useState(true),[busy,setBusy]=useState(false),[error,setError]=useState('');
 useEffect(()=>{let live=true;setLoading(true);void api.connectionCommitIdentity(profile).then(value=>{if(live){setSaved(value);setName(value?.name??'');setEmail(value?.email??'');}}).catch(e=>{if(live)setError(errorMessage(e));}).finally(()=>{if(live)setLoading(false);});return()=>{live=false;};},[profile.provider,profile.origin]);
 const save=async(value:GitCommitIdentity|null)=>{if(busy||loading||blocked)return;setBusy(true);onBusyChange?.(true);setError('');try{await api.setConnectionCommitIdentity(profile,value);setSaved(value);setName(value?.name??'');setEmail(value?.email??'');window.dispatchEvent(new Event('gitnova-commit-identity'));}catch(e){setError(errorMessage(e));}finally{setBusy(false);onBusyChange?.(false);}};
 return <form className="connection-commit-identity" aria-label={t('Commit identity for {0}',[profile.origin])} onSubmit={e=>{e.preventDefault();void save({name:name.trim(),email:email.trim()});}}>
  <h4>{t('Commit identity')}</h4><p className="hint">{t('Optional name and email for commits made in GitNova. Authentication accounts and commit authors are independent.')}</p>
  <p className="hint">{t('Matches origin, or the only remote, on this host. Git configuration files stay unchanged. Existing commits are never rewritten.')}</p>
  {loading?<p role="status">{t('Reading commit identity…')}</p>:<p className="hint">{saved?t('Saved commit identity: {0} <{1}>',[saved.name,saved.email]):t('Using Git configuration')}</p>}
  <label htmlFor={`commit-name-${profile.origin}`}>{t('Commit name')}</label><input id={`commit-name-${profile.origin}`} value={name} maxLength={256} disabled={blocked||loading||busy} onChange={e=>setName(e.target.value)}/>
  <label htmlFor={`commit-email-${profile.origin}`}>{t('Commit email')}</label><input id={`commit-email-${profile.origin}`} value={email} maxLength={254} disabled={blocked||loading||busy} onChange={e=>setEmail(e.target.value)} autoComplete="off"/>
  <p className="hint">{t('For GitHub attribution, use a verified email or your GitHub noreply address. It is not inferred from your login.')}</p>
  <div className="connection-actions"><button disabled={blocked||loading||busy||!name.trim()||!email.trim()}>{busy?t('Saving…'):t('Save commit identity')}</button>{saved&&<button type="button" disabled={blocked||busy} onClick={()=>void save(null)}>{t('Use Git configuration')}</button>}</div>
  {error&&<p role="alert" className="notice">{t(error)}</p>}
 </form>;
}
export function CommitIdentityNotice({membershipId,refreshKey}:{membershipId:string;refreshKey?:string}) {
 const [value,setValue]=useState<CommitIdentityContext|null>(null),[error,setError]=useState('');
 useEffect(()=>{let live=true;let sequence=0;const load=()=>{const request=++sequence;void api.commitIdentity(membershipId).then(v=>{if(live&&sequence===request){setValue(v);setError('');}}).catch(e=>{if(live&&sequence===request){setValue(null);setError(errorMessage(e));}});};load();window.addEventListener('gitnova-commit-identity',load);return()=>{live=false;window.removeEventListener('gitnova-commit-identity',load);};},[membershipId,refreshKey]);
 return <div className="commit-identity-notice" aria-label={t('Commit identity')}>
  {value?<><span>{t('Committer')}: <strong>{value.identity.name}</strong> &lt;{value.identity.email}&gt;</span><small>{value.connection?t('Connection identity · {0}',[value.connection.origin]):t('Using Git configuration')}</small></>:<small role={error?'alert':'status'}>{error?t(error):t('Reading commit identity…')}</small>}
  <small>{t('Amend, cherry-pick and rebase preserve original authors. Change commit identity in Settings → Connections.')}</small>
 </div>;
}
