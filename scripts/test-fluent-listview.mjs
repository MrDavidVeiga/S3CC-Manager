/* DOM-simulated ListView regression tests (not a WebView screenshot test). */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { JSDOM } from "jsdom";

const read = path => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const viewSpecs = [
  ["package-list", "package-row", 66, 56, 3000],
  ["duplicates-list", "duplicate-row", 62, 53, 247],
  ["conflicts-list", "conflict-row", 62, 53, 1672],
];
const html = `<!DOCTYPE html><html><body class="release-v1">${viewSpecs.map(([id])=>
  `<div id="${id}" class="${id}"></div>`).join("")}</body></html>`;
const dom = new JSDOM(html, {url:"https://localhost/",runScripts:"outside-only",pretendToBeVisual:true});
const {window} = dom; const {document} = window;
const flush=()=>new Promise(done=>setTimeout(done,0));
const send=(element,key)=>{
  const event=new window.KeyboardEvent("keydown",{key,bubbles:true,cancelable:true});
  element.dispatchEvent(event);
  return event;
};

const renderers = new Map();
for(const [id,rowClass,stride,rowHeight,count] of viewSpecs) {
  const list=document.getElementById(id);
  list.dataset.fluentCount=String(count);
  Object.defineProperty(list,"clientHeight",{get:()=>190});
  const render=()=>{
    const first=Math.max(0,Math.min(count-1,Math.floor(list.scrollTop/stride)-2));
    const fragment=document.createDocumentFragment();
    for(let i=first;i<Math.min(count,first+8);i++){
      const row=document.createElement(id==="package-list"?"div":"button");
      row.className=rowClass;
      row.dataset.fluentIndex=String(i);
      row.tabIndex=0;
      row.textContent=`Item ${i}`;
      if(id==="package-list"){
        row.setAttribute("role","button");
        const checkbox=document.createElement("input");
        checkbox.type="checkbox";
        row.appendChild(checkbox);
      }else row.type="button";
      fragment.appendChild(row);
    }
    list.replaceChildren(fragment);
  };
  list.addEventListener("scroll",render);
  renderers.set(id,render);
  render();
}
const source=read("src/ui/fluent/listview.js").replace(/^export\s+\{[^}]*\};?\s*$/gm,"");
window.eval(source);
const get=(list,index)=>list.querySelector(`[data-fluent-index="${index}"]`);

const manager=document.getElementById("package-list");
assert.equal(manager.getAttribute("aria-label"),"Manager packages");
assert.equal(manager.dataset.fluentKeyboard,"true");
get(manager,0).focus();
let e=send(get(manager,0),"ArrowDown");
assert.equal(e.defaultPrevented,true);
assert.equal(document.activeElement.dataset.fluentIndex,"1");
e=send(get(manager,1),"End");
assert.equal(e.defaultPrevented,true);
await flush();
assert.equal(document.activeElement.dataset.fluentIndex,"2999","End scrolls and focuses last virtual item");
send(document.activeElement,"ArrowUp");
await flush();
assert.equal(document.activeElement.dataset.fluentIndex,"2998");
send(document.activeElement,"Home");
await flush();
assert.equal(document.activeElement.dataset.fluentIndex,"0","Home navigates to first item");
send(document.activeElement,"PageDown");
await flush();
assert(Number(document.activeElement.dataset.fluentIndex)>0,"PageDown advances by viewport");
send(document.activeElement,"PageUp");
await flush();
assert.equal(document.activeElement.dataset.fluentIndex,"0");
const checkbox=get(manager,0).querySelector("input");
checkbox.focus();
e=send(checkbox,"ArrowDown");
assert.equal(e.defaultPrevented,false,"keyboard navigation must not hijack checkbox");
assert.equal(document.activeElement,checkbox);

for(const [id,rowClass,, ,count] of viewSpecs.slice(1)){
  const list=document.getElementById(id);
  assert.equal(list.dataset.fluentCount,String(count));
  get(list,0).focus();
  e=send(get(list,0),"ArrowDown");
  assert.equal(e.defaultPrevented,true);
  assert.equal(document.activeElement.dataset.fluentIndex,"1");
  send(document.activeElement,"End");
  await flush();
  assert.equal(document.activeElement.dataset.fluentIndex,String(count-1));
  send(document.activeElement,"ArrowDown");
  await flush();
  assert.equal(document.activeElement.dataset.fluentIndex,String(count-1),"can't navigate beyond final item");
  e=send(document.activeElement,"Enter");
  assert.equal(e.defaultPrevented,false,"Enter still belongs to existing open-details handler");
}

const appSource=read("src/app.js");
assert.match(appSource,/el\.packageList\.dataset\.fluentCount/);
assert.match(appSource,/el\.duplicatesList\.dataset\.fluentCount/);
assert.match(appSource,/el\.conflictsList\.dataset\.fluentCount/);
assert.equal((appSource.match(/row\.dataset\.fluentIndex = String\(index\)/g)||[]).length,3);
assert.match(appSource,/MANAGER_ROW_STRIDE = 66/);
assert.match(appSource,/ANALYSIS_ROW_STRIDE = 64/);
const scroll=read("src/ui/fluent/scrollbars.css");
assert.match(scroll,/::-webkit-scrollbar-thumb:hover/);
assert.match(scroll,/scrollbar-width: auto/);
assert.match(scroll,/border: 4px solid transparent/);
assert.match(scroll,/border-width: 2px/);
assert.match(scroll,/\(forced-colors: active\)/);
const index=read("index.html");
assert.match(index,/src\/ui\/fluent\/listview\.js/);
assert.match(index,/src\/ui\/fluent\/scrollbars\.css/);
console.log("Fluent ListView/ScrollBar tests: PASS (3000 rows, keyboard navigation, virtual focus, checkbox isolation, layout invariants)");
dom.window.close();
