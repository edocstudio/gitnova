import {useEffect,useRef} from 'react';
import {StaleLockRecovery} from './StaleLockRecovery';
import {t} from './i18n';

/** Reuses the reviewed native recovery without retrying the interrupted write. */
export function LockRecoveryDialog({membershipId,disabled,onBusyChange,onRefresh,onClose}:{membershipId:string;disabled:boolean;onBusyChange:(busy:boolean)=>void;onRefresh:()=>Promise<void>;onClose:()=>void}){
 const dialog=useRef<HTMLDialogElement>(null);
 const busy=useRef(false);
 useEffect(()=>{const node=dialog.current;const previous=document.activeElement as HTMLElement|null;node?.showModal();return()=>{node?.close();if(previous?.isConnected)previous.focus();};},[]);
 return <dialog ref={dialog} className="settings-dialog lock-recovery-dialog" aria-labelledby="lock-recovery-title" onCancel={event=>{event.preventDefault();if(!busy.current)onClose();}} onKeyDown={event=>event.stopPropagation()}>
  <h2 id="lock-recovery-title">{t('Repository is locked')}</h2>
  <p>{t('Review the lock below. Recovery refreshes this repository automatically and does not retry the interrupted action.')}</p>
  <StaleLockRecovery membershipId={membershipId} disabled={disabled} autoInspect onBusyChange={value=>{busy.current=value;onBusyChange(value);}} onRefresh={onRefresh} onRecovered={onClose}/>
  <button disabled={disabled} onClick={onClose}>{t('Close')}</button>
 </dialog>;
}
