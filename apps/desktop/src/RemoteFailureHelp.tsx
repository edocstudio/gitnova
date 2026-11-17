import {lazy,Suspense,useState} from 'react';
import {t} from './i18n';
import type {OperationReceipt} from './contracts';
const RemoteEndpointDialog=lazy(()=>import('./RemoteEndpointDialog').then(module=>({default:module.RemoteEndpointDialog})));

export function isRemoteRedirect(value:unknown):boolean {
  if(!value||typeof value!=='object')return false;
  if('code' in value&&value.code==='git_redirect')return true;
  const message='summary' in value?value.summary:'message' in value?value.message:'';
  return typeof message==='string'&&message.startsWith('The Git server redirected this repository URL.');
}
export function RemoteFailureHelp({membershipId,remote,blocked,onBusyChange,onDone,onChanged}:{membershipId:string;remote:string;blocked:boolean;onBusyChange:(value:boolean)=>void;onDone:(receipt:OperationReceipt)=>Promise<void>;onChanged:()=>void}){
  const [editing,setEditing]=useState(false);
  return <div className="notice remote-failure-help" role="region" aria-label={t('Resolve remote URL')}>
    <p>{t('The server redirected the Git URL. Use the exact clone URL, including .git when required. Credentials were not forwarded to the redirect destination.')}</p>
    <button disabled={blocked||!remote} onClick={()=>setEditing(true)}>{t('Review remote URL…')}</button>
    <p className="hint">{t('After saving, review and retry this operation yourself. No automatic retry is performed.')}</p>
    {editing&&<Suspense fallback={<span role="status">{t('Loading…')}</span>}><RemoteEndpointDialog membershipId={membershipId} repositoryName={t('Current repository')} names={[remote]} initialName={remote} blocked={blocked} onBusyChange={onBusyChange} onDone={async receipt=>{await onDone(receipt);if(receipt.status==='succeeded')onChanged();}} onClose={()=>setEditing(false)}/></Suspense>}
  </div>;
}
