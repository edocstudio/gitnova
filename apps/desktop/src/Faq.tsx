import {useState} from 'react';
import {faqEntries} from './faq-content';
import {t,useLocale} from './i18n';
export function Faq(){
 useLocale();const [query,setQuery]=useState('');const needle=query.trim().toLocaleLowerCase();
 const matches=faqEntries.filter(v=>!needle||[v.group,v.question,v.answer,t(v.group),t(v.question),t(v.answer)].some(text=>text.toLocaleLowerCase().includes(needle)));
 return <section className="settings-faq"><h3>{t('FAQ & guide')}</h3>
  <p className="hint">{t('Learn the Git concepts behind the actions you use.')}</p>
  <input type="search" aria-label={t('Search the guide…')} placeholder={t('Search the guide…')} value={query} onChange={e=>setQuery(e.target.value)} maxLength={200}/>
  {!matches.length&&<p role="status" className="hint">{t('No matching guide entries.')}</p>}
  {[...new Set(matches.map(v=>v.group))].map(group=><div key={group}><h4>{t(group)}</h4>{matches.filter(v=>v.group===group).map(v=><details key={v.question}><summary>{t(v.question)}</summary><p>{t(v.answer)}</p></details>)}</div>)}
 </section>;
}
