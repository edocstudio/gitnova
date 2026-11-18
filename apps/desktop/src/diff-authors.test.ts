import {expect,it} from 'vitest';
import {authorForLine,authorPathSupported,authorScopeKey,planAuthors,readAuthorPage,relativeAuthorTime} from './diff-authors';
import type {AuthorScope,AuthorPageRequest} from './diff-authors';
import type {DiffLine} from './diff';
import type {InvestigationPage,NativePath} from './contracts';
const path=(display:string):NativePath=>({display,encoding:'unix',hex:[...new TextEncoder().encode(display)].map(byte=>byte.toString(16).padStart(2,'0')).join('')});
const scope:AuthorScope={workspaceId:'workspace',membershipId:'member',beforeOid:'a'.repeat(40),afterOid:'b'.repeat(40),path:path('code.ts')};
const line=(old:number|null,next:number|null,kind:DiffLine['kind']='context',text='code'):DiffLine=>({old,next,kind,text});
const location={membershipId:'member',repositoryId:'repo',worktreeId:null,name:'repo',path:path('/repo')};
const request=()=>planAuthors(scope,[{side:'after',line:line(null,1,'add')}]).pages[0];
const evidence=(r:AuthorPageRequest):InvestigationPage=>({partial:false,hits:[{commit:{oid:'c'.repeat(40),parents:[],author:'ittipon.b',email:'test@example.invalid',timestamp:100,subject:'Add code'},excerpt:'',locations:[location]}],evidence:[{location,sourceOid:r.oid,targetOid:null,sourceOnly:null,targetOnly:null,changedFiles:null,dirty:true,scanned:1,limited:false,shallow:false,issue:null,lineAttribution:[{oid:'c'.repeat(40),path:path('old name.ts'),line:1,originalLine:9,content:'code'}]}]});
it('plans exact before/after revisions in bounded pages and omits the empty root side',()=>{
 const values=[{side:'before' as const,line:line(4,null,'remove')},{side:'after' as const,line:line(null,4,'add')},{side:'after' as const,line:line(5,5)}];
 const result=planAuthors(scope,values);expect(result.pages).toHaveLength(2);expect(result.pages.map(page=>page.oid)).toEqual([scope.beforeOid,scope.afterOid]);expect(result.pages.every(page=>page.end-page.start===199&&page.request.selection.scope==='current')).toBe(true);
 expect(planAuthors({...scope,beforeOid:null},values).pages).toHaveLength(1);
});
it('caps disjoint viewport ranges and rejects invalid or excessively distant line positions',()=>{
 const result=planAuthors(scope,Array.from({length:10},(_,i)=>({side:'after' as const,line:line(null,1+i*200,'add')})));
 expect(result.pages).toHaveLength(4);expect(result.limited).toBe(true);
 expect(planAuthors(scope,[{side:'after',line:line(null,1000001,'add')}])).toEqual({pages:[],limited:true});
});
it('binds author evidence to membership and revision and preserves original path and shallow limits',()=>{
 const r=request(),page=evidence(r);page.evidence[0].shallow=true;
 const result=readAuthorPage(scope,r,page);expect(result.lines.get(1)).toMatchObject({commit:{author:'ittipon.b'},side:'after',revision:scope.afterOid,partial:true,line:{originalLine:9,path:{display:'old name.ts'}}});
 page.evidence[0].sourceOid='d'.repeat(40);expect(()=>readAuthorPage(scope,r,page)).toThrow();page.evidence[0].sourceOid=r.oid;page.evidence[0].location={...location,membershipId:'another'};expect(()=>readAuthorPage(scope,r,page)).toThrow();
});
it('does not fabricate authors for missing commit data, unavailable reads or mismatched code',()=>{
 const r=request(),page=evidence(r),mapped=readAuthorPage(scope,r,page);
 expect(authorForLine(mapped,line(null,1,'add','code\r'),'after')?.commit.author).toBe('ittipon.b');expect(authorForLine(mapped,line(null,1,'add','other code'),'after')).toBeUndefined();
 page.hits=[];expect(readAuthorPage(scope,r,page).lines.size).toBe(0);page.evidence[0].issue='Unavailable';expect(readAuthorPage(scope,r,page)).toMatchObject({partial:true,issue:'Unavailable'});
});
it('isolates cache identities and requires lossless native path display for attribution',()=>{
 expect(authorScopeKey(scope)).not.toBe(authorScopeKey({...scope,membershipId:'another'}));expect(authorScopeKey(scope)).not.toBe(authorScopeKey({...scope,afterOid:'d'.repeat(40)}));
 expect(authorPathSupported(path('ไฟล์.ts'))).toBe(true);expect(authorPathSupported({...path('bad'),hex:'ff'})).toBe(false);expect(authorPathSupported({...path('bad'),encoding:'unknown'})).toBe(false);
});
it('formats eighteen months, future authorship and Thai relative times without caching stale formatted ages',()=>{
 const month=2629800,now=2000000000;
 expect(relativeAuthorTime(now-18*month,now,'en-US')).toBe('18 months ago');expect(relativeAuthorTime(now+2*month,now,'en-US')).toBe('in 2 months');expect(relativeAuthorTime(now-18*month,now,'th-TH')).toContain('เดือน');expect(relativeAuthorTime(NaN,now,'en-US')).toBe('');
});
