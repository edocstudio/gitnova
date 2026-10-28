import {useSyncExternalStore} from 'react';
export type Theme='dark'|'light';
const key='gitnova.theme';
export function initialTheme(storage:Pick<Storage,'getItem'>|null):Theme{
 try{return storage?.getItem(key)==='light'?'light':'dark';}catch{return 'dark';}
}
let theme:Theme=(()=>{try{return initialTheme(typeof localStorage==='undefined'?null:localStorage);}catch{return 'dark';}})();
const listeners=new Set<()=>void>();
function apply(){if(typeof document!=='undefined'){document.documentElement.dataset.theme=theme;document.documentElement.style.colorScheme=theme;}}
export function setTheme(next:Theme){
 if(next!=='dark'&&next!=='light')return;
 theme=next;apply();try{localStorage.setItem(key,next);}catch{/* Keep the preference for this session if storage is unavailable. */}
 listeners.forEach(listener=>listener());
}
export function useTheme(){return useSyncExternalStore(listener=>{listeners.add(listener);return()=>listeners.delete(listener);},()=>theme,()=> 'dark' as Theme);}
apply();
if(typeof window!=='undefined')window.addEventListener('storage',event=>{if(event.key!==key)return;theme=event.newValue==='light'?'light':'dark';apply();listeners.forEach(listener=>listener());});
