import { useState } from 'react';
import { api,desktop } from './api';
import type { ReactNode } from 'react';
import { t } from './i18n';
export function CiRunLink({url,children}:{url:string;children:ReactNode}){
 const [error,setError]=useState(false),[opening,setOpening]=useState(false);
 return <><a href={url} target="_blank" rel="noreferrer" aria-busy={opening} onClick={event=>{
  if(!desktop)return;event.preventDefault();if(opening)return;setOpening(true);setError(false);
  void api.openCiUrl(url).catch(()=>setError(true)).finally(()=>setOpening(false));
 }}>{children}</a>{error&&<p className="hint" role="status">{t('Could not open the browser. Copy this build link.')}<code className="commit-link-url">{url}</code></p>}</>;
}
