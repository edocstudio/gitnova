import {useEffect,useState} from 'react';
import {api} from './api';
import {t,useLocale} from './i18n';
import type {BranchTracking,RemoteBranches} from './contracts';
export function RemoteFooter({membershipId,branch,tracking,revision}:{membershipId?:string;branch:string|null|undefined;tracking?:BranchTracking[];revision?:string}){
 useLocale();const [value,setValue]=useState<{id:string;data:RemoteBranches}|null>(null),[failed,setFailed]=useState(false);
 useEffect(()=>{let alive=true,flight=false,again=false;const load=async()=>{if(!membershipId)return;if(flight){again=true;return;}flight=true;try{const data=await api.remoteBranches(membershipId);if(alive){setValue({id:membershipId,data});setFailed(false);}}catch{if(alive){setValue(null);setFailed(true);}}finally{flight=false;if(alive&&again){again=false;void load();}}};void load();window.addEventListener('focus',load);window.addEventListener('gitnova-remotes-changed',load);return()=>{alive=false;window.removeEventListener('focus',load);window.removeEventListener('gitnova-remotes-changed',load);};},[membershipId,revision]);
 if(!membershipId)return null;const data=value?.id===membershipId?value.data:null;const current=tracking?.find(row=>row.branch===branch);const names=current?.remote&&current.remote!=='.'?[current.remote]:data?.remotes??[];
 const urls=names.flatMap(name=>data?.urls?.[name]??[]);
 const text=!data?failed?t('Remote information unavailable'):t('Reading remote…'):!names.length?t('Remote: not configured'):current?.remote?t('Remote: {0}',[current.remote]):t('Remotes: {0}',[names.join(', ')]);
 const target=current?.upstream?.replace(/^refs\/(remotes|heads)\//,'')??t('No upstream');return <span className="footer-remote" aria-label={t('Repository remote')} title={[text,target,...urls].join('\n')}><span>{text}</span>{urls.length===1&&<span className="footer-remote-url">{urls[0]}</span>}<span className="footer-upstream">{target}</span>{current?.pushRemote&&current.pushRemote!==current.remote&&<span>{t("Push: {0}",[current.pushRemote])}</span>}</span>;
}
