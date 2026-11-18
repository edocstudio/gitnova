import {useEffect,useMemo,useRef,useState} from 'react';
import {api,errorMessage} from './api';
import {authorForLine,authorPathSupported,authorScopeKey,AUTHOR_CACHE_PAGES,planAuthors,readAuthorPage} from './diff-authors';
import type {AuthorPage,AuthorScope,AuthorSide,VisibleAuthorLine} from './diff-authors';
import type {DiffLine} from './diff';

export function useDiffAuthors(scope:AuthorScope|null,visible:VisibleAuthorLine[]){
 const [pages,setPages]=useState<Map<string,AuthorPage>>(new Map()),[reading,setReading]=useState(false),[revision,setRevision]=useState(0),[visibleWindow,setVisibleWindow]=useState(!document.hidden);
 const cache=useRef(pages);cache.current=pages;
 const scopeKey=scope?authorScopeKey(scope):'';
 const supported=!!scope&&authorPathSupported(scope.path);
 const plan=useMemo(()=>scope&&supported?planAuthors(scope,visible):{pages:[],limited:false},[scopeKey,supported,visible]);
 const requestKey=JSON.stringify(plan.pages.map(page=>page.key));
 useEffect(()=>{const update=()=>setVisibleWindow(!document.hidden);document.addEventListener('visibilitychange',update);return()=>document.removeEventListener('visibilitychange',update);},[]);
 useEffect(()=>{
  if(!scope||!supported||!visibleWindow){setReading(false);return;}
  const requests=plan.pages.filter(page=>!cache.current.has(page.key));
  if(!requests.length){setReading(false);return;}
  const controller=new AbortController();let live=true;setReading(true);
  const timer=setTimeout(()=>{
   api.diffLineAuthors(requests.map(page=>page.request),controller.signal).then(values=>{
    if(!live)return;
    const next=new Map(cache.current);
    requests.forEach((request,index)=>{
     try{next.set(request.key,readAuthorPage(scope,request,values[index]));}
     catch(error){next.set(request.key,{lines:new Map(),partial:true,issue:errorMessage(error)});}
    });
    while(next.size>AUTHOR_CACHE_PAGES)next.delete(next.keys().next().value!);
    cache.current=next;setPages(next);
   }).catch(error=>{
    if(!live||controller.signal.aborted)return;
    const next=new Map(cache.current);for(const request of requests)next.set(request.key,{lines:new Map(),partial:true,issue:errorMessage(error)});
    while(next.size>AUTHOR_CACHE_PAGES)next.delete(next.keys().next().value!);
    cache.current=next;setPages(next);
   }).finally(()=>{if(live)setReading(false);});
  },250);
  return()=>{live=false;clearTimeout(timer);controller.abort();};
 },[scopeKey,supported,requestKey,revision,visibleWindow]);
 const current=plan.pages.map(request=>pages.get(request.key));
 const issues=[...new Set(current.flatMap(page=>page?.issue?[page.issue]:[]))];
 return {reading,unsupported:!!scope&&!supported,partial:plan.limited||current.some(page=>page?.partial),issues,
  retry:()=>{cache.current=new Map();setPages(cache.current);setRevision(value=>value+1);},
  lookup:(line:DiffLine,side:AuthorSide)=>{
   if(!scope)return;
   const number=side==='before'?line.old:line.next;if(number===null)return;
   const request=plan.pages.find(page=>page.side===side&&page.start<=number&&page.end>=number);
   return authorForLine(request?pages.get(request.key):undefined,line,side);
  }};
}
