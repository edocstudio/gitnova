import type {Commit,InvestigationPage,InvestigationRequest,LineAttribution,NativePath} from './contracts';
import type {DiffLine} from './diff';
import {dateLocale} from './i18n';
export type AuthorSide='before'|'after';
export type AuthorScope={workspaceId:string;membershipId:string;beforeOid:string|null;afterOid:string;path:NativePath};
export type VisibleAuthorLine={side:AuthorSide;line:DiffLine};
export type AuthorPageRequest={key:string;side:AuthorSide;oid:string;start:number;end:number;request:InvestigationRequest};
export type AuthoredLine={commit:Commit;line:LineAttribution;side:AuthorSide;revision:string;partial:boolean};
export type AuthorPage={lines:Map<number,AuthoredLine>;partial:boolean;issue:string|null};
export const AUTHOR_PAGE_LINES=200,AUTHOR_BATCH_PAGES=4,AUTHOR_CACHE_PAGES=8;
export function authorScopeKey(scope:AuthorScope):string{return JSON.stringify([scope.workspaceId,scope.membershipId,scope.beforeOid,scope.afterOid,scope.path.encoding,scope.path.hex]);}
export function authorPathSupported(path:NativePath):boolean{
 let bytes:Uint8Array;
 if(path.encoding==='unix')bytes=new TextEncoder().encode(path.display);
 else if(path.encoding==='windows'){bytes=new Uint8Array(path.display.length*2);const view=new DataView(bytes.buffer);for(let i=0;i<path.display.length;i++)view.setUint16(i*2,path.display.charCodeAt(i),true);}
 else return false;
 return [...bytes].map(byte=>byte.toString(16).padStart(2,'0')).join('')===path.hex.toLowerCase();
}
export function planAuthors(scope:AuthorScope,visible:VisibleAuthorLine[]):{pages:AuthorPageRequest[];limited:boolean}{
 const pages=new Map<string,AuthorPageRequest>(),key=authorScopeKey(scope);let limited=false;
 for(const {side,line} of visible){
  const number=side==='before'?line.old:line.next,oid=side==='before'?scope.beforeOid:scope.afterOid;
  if(number===null||!oid||line.kind==='hunk'||line.kind==='meta')continue;
  if(!Number.isSafeInteger(number)||number<1||number>1000000){limited=true;continue;}
  const start=Math.floor((number-1)/AUTHOR_PAGE_LINES)*AUTHOR_PAGE_LINES+1,end=start+AUTHOR_PAGE_LINES-1;
  const pageKey=JSON.stringify([key,side,oid,start]);if(pages.has(pageKey))continue;
  if(pages.size===AUTHOR_BATCH_PAGES){limited=true;continue;}
  pages.set(pageKey,{key:pageKey,side,oid,start,end,request:{selection:{workspaceId:scope.workspaceId,currentMembershipId:scope.membershipId,scope:'current',selectedIds:[],query:'',cursor:null},mode:'blame',path:scope.path.display,source:oid,target:'',needle:'',lineStart:start,lineEnd:end}});
 }
 return {pages:[...pages.values()],limited};
}
export function readAuthorPage(scope:AuthorScope,request:AuthorPageRequest,page:InvestigationPage):AuthorPage{
 if(!page)throw Error('Line author evidence did not match this diff.');
 const evidence=page.evidence[0];
 if(page.evidence.length!==1||!evidence||evidence.location.membershipId!==scope.membershipId)throw Error('Line author evidence did not match this diff.');
 if(evidence.issue)return {lines:new Map(),partial:true,issue:evidence.issue};
 if(evidence.sourceOid!==request.oid)throw Error('Line author evidence did not match this diff.');
 const commits=new Map(page.hits.filter(hit=>hit.locations.some(location=>location.membershipId===scope.membershipId)).map(hit=>[hit.commit.oid,hit.commit]));
 const lines=new Map<number,AuthoredLine>();const partial=page.partial||evidence.shallow||evidence.limited;
 for(const line of evidence.lineAttribution??[]){
  if(!Number.isInteger(line.line)||line.line<request.start||line.line>request.end||lines.has(line.line))throw Error('Line author evidence did not match this diff.');
  const commit=commits.get(line.oid);
  if(commit&&/^(?:[a-fA-F0-9]{40}|[a-fA-F0-9]{64})$/.test(commit.oid)&&Number.isFinite(commit.timestamp))lines.set(line.line,{commit,line,side:request.side,revision:request.oid,partial});
 }
 return {lines,partial,issue:null};
}
export function authorForLine(page:AuthorPage|undefined,line:DiffLine,side:AuthorSide):AuthoredLine|undefined{
 const number=side==='before'?line.old:line.next;if(number===null)return;
 const value=page?.lines.get(number);
 return value&&value.line.content.replace(/\r$/,'')===line.text.replace(/\r$/,'')?value:undefined;
}
const relative=new Map<string,Intl.RelativeTimeFormat>();
export function relativeAuthorTime(timestamp:number,now=Date.now()/1000,locale=dateLocale()):string{
 if(!Number.isFinite(timestamp)||!Number.isFinite(now))return '';
 let formatter=relative.get(locale);if(!formatter){formatter=new Intl.RelativeTimeFormat(locale,{numeric:'auto'});relative.set(locale,formatter);if(relative.size>4)relative.delete(relative.keys().next().value!);}
 const delta=timestamp-now,age=Math.abs(delta);
 const [unit,size]:[Intl.RelativeTimeFormatUnit,number]=age<60?['second',1]:age<3600?['minute',60]:age<86400?['hour',3600]:age<2629800?['day',86400]:age<63115200?['month',2629800]:['year',31557600];
 return formatter.format(Math.sign(delta)*Math.floor(age/size),unit);
}
