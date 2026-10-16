/* Delegated Fluent ToolTip for existing title attributes.
 * No per-row listeners, no changes to dynamically virtualized list items.
 */
const tooltip=document.createElement("div");
tooltip.id="fluent-shared-tooltip";
tooltip.className="fluent-tooltip";
tooltip.setAttribute("role","tooltip");
tooltip.hidden=true;
document.body.appendChild(tooltip);
let current=null;
let originalDescription=null;
let fromKeyboard=false;
let suppressedNode=null;
function closeTooltip() {
  if (!current) return;
  if(!current.hasAttribute("title") && current.dataset.fluentTooltipTitle!==undefined){
    current.setAttribute("title",current.dataset.fluentTooltipTitle);
  }
  delete current.dataset.fluentTooltipTitle;
  if(originalDescription===null)current.removeAttribute("aria-describedby");
  else current.setAttribute("aria-describedby",originalDescription);
  current=null; originalDescription=null;
  tooltip.removeAttribute("data-visible");
  tooltip.hidden=true;
}
function showTooltip(node,keyboard=false) {
  if(!node || !(node instanceof Element))return;
  if(suppressedNode && (node===suppressedNode || suppressedNode.contains(node)))return;
  if(node===current)return;
  closeTooltip();
  const title=node.getAttribute("title");
  if(!title)return;
  current=node;fromKeyboard=keyboard;
  originalDescription=node.getAttribute("aria-describedby");
  node.dataset.fluentTooltipTitle=title;
  node.removeAttribute("title");
  tooltip.textContent=title;
  node.setAttribute("aria-describedby",
    [originalDescription,"fluent-shared-tooltip"].filter(Boolean).join(" "));
  tooltip.hidden=false;
  const rect=node.getBoundingClientRect();
  const tip=tooltip.getBoundingClientRect();
  const left=Math.min(Math.max(8,rect.left+(rect.width-tip.width)/2),Math.max(8,window.innerWidth-tip.width-8));
  const below=rect.bottom+8;
  const top=below+tip.height<=window.innerHeight-8 ? below : Math.max(8,rect.top-tip.height-8);
  tooltip.style.left=`${left}px`;tooltip.style.top=`${top}px`;
  tooltip.dataset.visible="true";
}
document.addEventListener("pointerover",event=>{
  const target=event.target.closest?.("[title]");
  if(target && !target.closest(".fluent-tooltip"))showTooltip(target);
});
document.addEventListener("pointerout",event=>{
  if(suppressedNode && event.target instanceof Element && suppressedNode.contains(event.target) &&
     !suppressedNode.contains(event.relatedTarget))suppressedNode=null;
  if(current && event.target instanceof Element && current.contains(event.target) &&
     !current.contains(event.relatedTarget))closeTooltip();
});
document.addEventListener("focusin",event=>{
  const target=event.target.closest?.("[title]");
  if(target)showTooltip(target,true);
});
document.addEventListener("focusout",event=>{
  if(suppressedNode && event.target instanceof Element && suppressedNode.contains(event.target) &&
     !suppressedNode.contains(event.relatedTarget))suppressedNode=null;
  if(current && current.contains(event.target) &&
     !current.contains(event.relatedTarget))closeTooltip();
});
document.addEventListener("pointerdown",event=>{
  if(suppressedNode && event.target instanceof Element &&
     (event.target===suppressedNode || suppressedNode.contains(event.target)))suppressedNode=null;
});
document.addEventListener("keydown",event=>{if(event.key==="Escape")closeTooltip();});
window.addEventListener("scroll",closeTooltip,true);
window.addEventListener("blur",closeTooltip);
function suppressTooltipUntilPointerLeave(node) {
  if(!(node instanceof Element))return;
  suppressedNode=node;
  closeTooltip();
}

export { showTooltip, closeTooltip, suppressTooltipUntilPointerLeave };
