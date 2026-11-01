import {useEffect} from 'react';
import type {Dispatch,SetStateAction} from 'react';

/** Clear the recovered lock error without remounting editors or losing drafts. */
export function useRecoveredLockError(membershipId:string,setError:Dispatch<SetStateAction<string>>){
 useEffect(()=>{
  const recovered=(event:Event)=>{
   if((event as CustomEvent<string>).detail===membershipId){
    setError(current=>current.includes('gitnova-operation.lock already exists')?'':current);
   }
  };
  window.addEventListener('gitnova-lock-recovered',recovered);
  return()=>window.removeEventListener('gitnova-lock-recovered',recovered);
 },[membershipId,setError]);
}
