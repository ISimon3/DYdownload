import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
const script=readFileSync(new URL('../src-tauri/src/capture.js',import.meta.url),'utf8');
function environment({hostname='www.douyin.com',path='/user/test',data={aweme_list:[{aweme_id:'123',author:{sec_uid:'test'}}],has_more:0}}={}) {
 const calls=[],navigations=[],timers=[];class XHR{open(){}send(){}addEventListener(_,fn){this.loaded=fn;}}
 const response={clone:()=>({json:async()=>data})};
 const window={fetch:async()=>response,__TAURI_INTERNALS__:{invoke:async(command,args)=>calls.push({command,args})}};
 const location={hostname,pathname:path,hash:'#sg=test-query',origin:`https://${hostname}`,href:`https://${hostname}${path}#sg=test-query`,assign:url=>navigations.push(url)};
 const context=vm.createContext({window,location,XMLHttpRequest:XHR,URL,URLSearchParams,AbortSignal,console,document:{querySelectorAll:()=>[]},setInterval:fn=>timers.push(fn),clearInterval(){}});
 vm.runInContext(script,context);return {calls,window,context,XHR,navigations,timers};
}
test('captures only work-list fetch responses with query isolation',async()=>{const e=environment();await e.window.fetch('/aweme/v1/web/aweme/post/');await new Promise(setImmediate);assert.equal(e.calls.length,2);const data=JSON.parse(e.calls[1].args.payload);assert.equal(data.queryId,'test-query');assert.equal(data.kind,'works');await e.window.fetch('/unrelated');await new Promise(setImmediate);assert.equal(e.calls.length,2);});
test('does not install on unrelated origins',()=>{const e=environment({hostname:'example.com'});assert.equal(e.window.__shiguangInstalled,undefined);});
test('installation is idempotent',()=>{const e=environment();const first=e.window.fetch;vm.runInContext(script,e.context);assert.equal(e.window.fetch,first);});
test('captures XHR json response',()=>{const e=environment();const xhr=new e.XHR();xhr.open('GET','/aweme/v1/web/aweme/post/');xhr.send();xhr.responseType='json';xhr.response={aweme_list:[],has_more:0};xhr.loaded();assert.equal(e.calls.length,1);});
test('resolves only exact Douyin ID and navigates internally',async()=>{const e=environment({path:'/search/40204190842',data:{user_list:[{user_info:{unique_id:'40204190842',uid:'1234',sec_uid:'MS4wLjABAAAA-test'}}]}});await e.window.fetch('/aweme/v1/web/discover/search/');await new Promise(setImmediate);assert.equal(e.navigations[0],'https://www.douyin.com/user/MS4wLjABAAAA-test?_sg=test-query#sg=test-query');assert.equal(JSON.parse(e.calls[0].args.payload).kind,'author');});
test('does not pick a fuzzy matching author',async()=>{const e=environment({path:'/search/40204190842',data:{user_list:[{user_info:{unique_id:'402041908421',sec_uid:'MS4wLjABAAAA-test'}}]}});await e.window.fetch('/aweme/v1/web/discover/search/');await new Promise(setImmediate);assert.equal(e.navigations.length,0);});

test('ignores the logged-in account profile and unrelated author works',async()=>{const e=environment({data:{user:{sec_uid:'someone-else',nickname:'wrong'},aweme_list:[{aweme_id:'9',author:{sec_uid:'someone-else'}}]}});await e.window.fetch('/aweme/v1/web/user/profile/self/');await e.window.fetch('/aweme/v1/web/user/profile/other/');await e.window.fetch('/aweme/v1/web/aweme/post/');await new Promise(setImmediate);assert.equal(e.calls.length,0);});
test('accepts only the target author profile',async()=>{const e=environment({data:{user:{sec_uid:'test',nickname:'right'}}});await e.window.fetch('/aweme/v1/web/user/profile/other/');await new Promise(setImmediate);assert.equal(JSON.parse(e.calls[0].args.payload).data.nickname,'right');});
test('DOM fallback separates rendered ID from adjacent statistics',async()=>{const e=environment({path:'/search/40204190842'});e.context.document.querySelectorAll=()=>[{innerText:'豌豆杀手\n抖音号: 40204190842\n2.6万获赞',textContent:'豌豆杀手抖音号:402041908422.6万获赞',href:'https://www.douyin.com/user/MS4wLjABAAAA-test',querySelectorAll:()=>[]}];e.timers[0]();await new Promise(setImmediate);assert.equal(e.navigations.length,1);});
test('DOM fallback handles ID label independently without prefix matches',async()=>{const e=environment({path:'/search/40204190842'});e.context.document.querySelectorAll=()=>[{innerText:'',href:'https://www.douyin.com/user/MS4wLjABAAAA-test',querySelectorAll:()=>[{textContent:'抖音号:402041908421'}]}];e.timers[0]();assert.equal(e.navigations.length,0);e.context.document.querySelectorAll=()=>[{innerText:'',href:'https://www.douyin.com/user/MS4wLjABAAAA-test',querySelectorAll:()=>[{textContent:'抖音号:40204190842'}]}];e.timers[0]();await new Promise(setImmediate);assert.equal(e.navigations.length,1);});
test('pagination cannot scroll the search page',async()=>{const e=environment({path:'/search/40204190842'});e.context.document.querySelectorAll=()=>{throw Error('search page must not scroll')};await e.window.__shiguangLoadMore();assert.equal(e.calls.length,0);});
test('nested search payload is supported',async()=>{const e=environment({path:'/search/40204190842',data:{data:{user_list:[{user_info:{unique_id:'40204190842',sec_uid:'MS4wLjABAAAA-test'}}]}}});await e.window.fetch('/aweme/v1/web/discover/search/');await new Promise(setImmediate);assert.equal(e.navigations.length,1);});

test('waits for author IPC acknowledgement before leaving the search document',async()=>{
 const e=environment({path:'/search/30847026',data:{user_list:[{user_info:{unique_id:'30847026',sec_uid:'MS4wLjABAAAA-test'}}]}});
 let acknowledge; e.window.__TAURI_INTERNALS__.invoke=()=>new Promise(resolve=>{acknowledge=resolve;});
 await e.window.fetch('/aweme/v1/web/discover/search/');await new Promise(setImmediate);
 assert.equal(e.navigations.length,0);acknowledge();await new Promise(setImmediate);
 assert.equal(e.navigations.length,1);
});

test('posts recover author information when profile response is absent',async()=>{
 const e=environment({data:{aweme_list:[{aweme_id:'123',author:{sec_uid:'test',nickname:'Daydreamer',unique_id:'30847026'}}],has_more:0}});
 await e.window.fetch('/aweme/v1/web/aweme/post/');await new Promise(setImmediate);
 const payloads=e.calls.map(call=>JSON.parse(call.args.payload));
 assert.deepEqual(payloads.map(payload=>payload.kind),['author','works']);
 assert.equal(payloads[0].data.nickname,'Daydreamer');
});

test('profile ID may come from the scoped request, but conflicting author is rejected',async()=>{
 const e=environment({data:{user:{nickname:'Daydreamer'}}});
 await e.window.fetch('/aweme/v1/web/user/profile/other/?sec_user_id=wrong');await new Promise(setImmediate);
 assert.equal(e.calls.length,0);
 await e.window.fetch('/aweme/v1/web/user/profile/other/?sec_user_id=test');await new Promise(setImmediate);
 assert.equal(JSON.parse(e.calls[0].args.payload).data.sec_uid,'test');
});
