import {setAutoFetchInterval,useAutoFetchInterval} from './auto-fetch';
import {useEffect,useRef,useState} from 'react';
import {setTheme,useTheme} from './theme';
import {Faq} from './Faq';
import {Connections} from './Connections';
import {Icon} from './Icons';
import {t,setLocale,useLocale} from './i18n';
export function Settings({open,onClose}:{open:boolean;onClose:()=>void}){
 const fetchInterval=useAutoFetchInterval();
 const close=onClose;
 const tabs=['general','connections','faq'] as const;
 type Tab=typeof tabs[number];
 const [tab,setTab]=useState<Tab>('general');
 const [visited,setVisited]=useState<Tab[]>(['general']);
 const scroll=useRef<HTMLDivElement>(null);
 const labels={general:'General',connections:'Connections',faq:'FAQ'};
 const activate=(next:Tab)=>{setTab(next);setVisited(old=>old.includes(next)?old:[...old,next]);if(scroll.current)scroll.current.scrollTop=0;};
 const dialog=useRef<HTMLDialogElement>(null);const locale=useLocale();const theme=useTheme();
 useEffect(()=>{const node=dialog.current;if(!node)return;if(open&&!node.open){setTab('general');setVisited(['general']);node.showModal();}else if(!open&&node.open)node.close();},[open]);

 return <dialog ref={dialog} className="settings-dialog" aria-labelledby="settings-title" onKeyDown={e=>{e.stopPropagation();if((e.metaKey||e.ctrlKey)&&[',','o','k'].includes(e.key.toLowerCase()))e.preventDefault();}} onCancel={e=>{e.preventDefault();close();}} onClick={e=>{if(e.target===dialog.current){const box=dialog.current.getBoundingClientRect();if(e.clientX<box.left||e.clientX>box.right||e.clientY<box.top||e.clientY>box.bottom)close();}}}>
  <div className="settings-header"><h2 id="settings-title"><Icon name="settings" size={20}/>{t('Settings')}</h2><button autoFocus className="icon-button" aria-label={t('Close settings')} onClick={close}><Icon name="close"/></button></div>
  {open&&<><div className="settings-tabs" role="tablist" aria-label={t('Settings sections')}>
   {tabs.map(item=><button key={item} type="button" role="tab" id={`settings-tab-${item}`} aria-selected={tab===item} aria-controls={`settings-panel-${item}`} tabIndex={tab===item?0:-1} onClick={()=>activate(item)} onKeyDown={e=>{
    const index=tabs.indexOf(item);const next=e.key==='ArrowRight'?tabs[(index+1)%tabs.length]:e.key==='ArrowLeft'?tabs[(index+tabs.length-1)%tabs.length]:e.key==='Home'?tabs[0]:e.key==='End'?tabs[tabs.length-1]:null;
    if(next){e.preventDefault();activate(next);dialog.current?.querySelector<HTMLButtonElement>(`#settings-tab-${next}`)?.focus();}
   }}>{t(labels[item])}</button>)}
  </div><div className="settings-scroll" ref={scroll}>
   <div role="tabpanel" id="settings-panel-general" aria-labelledby="settings-tab-general" hidden={tab!=='general'} tabIndex={0}>
   <section className="settings-general" aria-labelledby="settings-general-title"><h3 id="settings-general-title">{t('General')}</h3><p className="hint">{t('Applies to GitNova on this device.')}</p><label className="field-label" htmlFor="settings-language">{t('Language')}</label><select id="settings-language" value={locale} onChange={e=>setLocale(e.target.value as 'en'|'th')}><option value="en">English</option><option value="th">ไทย</option></select><label className="field-label" htmlFor="settings-theme">{t('Theme')}</label><select id="settings-theme" value={theme} onChange={e=>setTheme(e.target.value as 'dark'|'light')}><option value="dark">{t('Dark')}</option><option value="light">{t('Light')}</option></select><p className="hint">{t('Dark is the default. Your theme applies to every workspace on this device.')}</p><label className="field-label" htmlFor="settings-auto-fetch">{t('Auto fetch')}</label><select id="settings-auto-fetch" value={fetchInterval} onChange={e=>setAutoFetchInterval(Number(e.target.value))}>{[0,30,60,120,300].map(seconds=><option key={seconds} value={seconds}>{t(seconds===0?'Off':seconds===30?'Every 30 seconds':seconds===60?'Every minute':seconds===120?'Every 2 minutes':'Every 5 minutes')}</option>)}</select><p className="hint">{t('Fetches the active repository’s upstream remote (otherwise origin or its only remote). Pauses while hidden or during Git operations. Local files and branches stay unchanged.')}</p><p className="hint">{t('Background HTTPS uses an already-authorized GitNova account; SSH uses your agent/config. If authorization is needed, fetch manually. Failures retry less often, without dialogs.')}</p></section>
   </div>
   <div role="tabpanel" id="settings-panel-connections" aria-labelledby="settings-tab-connections" hidden={tab!=='connections'} tabIndex={0}>{visited.includes('connections')&&<Connections/>}</div>
   <div role="tabpanel" id="settings-panel-faq" aria-labelledby="settings-tab-faq" hidden={tab!=='faq'} tabIndex={0}>{visited.includes('faq')&&<Faq/>}</div>


  </div></>}
 </dialog>;
}
