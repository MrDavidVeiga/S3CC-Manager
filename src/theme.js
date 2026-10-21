import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { closeTooltip, suppressTooltipUntilPointerLeave } from "./ui/fluent/tooltips.js";

const THEME_KEY = "s3cc-manager-theme";
const VALID_THEMES = new Set(["system","veiga-light","veiga-dark","mean-girls","ea-app","winui-light","winui-dark"]);
const VEIGA_ACCENT = "#7FAD2D";
const MEAN_GIRLS_ACCENT = "#F92F60";
const EA_APP_ACCENT = "#276AFC";
const WINUI_ACCENT_FALLBACK = "#0078D4";
const osDark = window.matchMedia("(prefers-color-scheme: dark)");

let nativeAppearance = {
  mode: null, accent: null,
  accentLight1: null, accentLight2: null, accentLight3: null,
  accentDark1: null, accentDark2: null, accentDark3: null,
  source: "fallback"
};

function parseRgb(color) {
  const value=String(color||"").trim();
  const rgb=value.match(/rgba?\(\s*(\d+)\D+(\d+)\D+(\d+)/i);
  if (rgb) return [Number(rgb[1]),Number(rgb[2]),Number(rgb[3])];
  const hex=value.replace("#","");
  if (/^[0-9a-f]{6}$/i.test(hex)) return [parseInt(hex.slice(0,2),16),parseInt(hex.slice(2,4),16),parseInt(hex.slice(4,6),16)];
  return null;
}
function toRgb(v) { return `rgb(${v.map(n=>Math.max(0,Math.min(255,Math.round(n)))).join(", ")})`; }
function mix(color,target,amount) {
  const a=parseRgb(color), b=parseRgb(target);
  if (!a || !b) return color;
  return toRgb(a.map((v,i)=>v+(b[i]-v)*amount));
}
function alpha(color,a) {
  const v=parseRgb(color);
  return v ? `rgba(${v[0]}, ${v[1]}, ${v[2]}, ${a})` : `rgba(0,120,212,${a})`;
}
function cssAccentProbe() {
  const probe=document.createElement("span");
  probe.style.cssText="position:fixed;left:-9999px;top:-9999px;color:#010203;color:AccentColor;";
  document.body.appendChild(probe);
  const value=getComputedStyle(probe).color;
  probe.remove();
  const rgb=parseRgb(value);
  if (!rgb || (rgb[0]===1 && rgb[1]===2 && rgb[2]===3)) return null;
  return toRgb(rgb);
}
function systemMode() {
  return nativeAppearance.mode==="light" || nativeAppearance.mode==="dark" ? nativeAppearance.mode : (osDark.matches ? "dark" : "light");
}
function systemAccent() { return nativeAppearance.accent || cssAccentProbe() || WINUI_ACCENT_FALLBACK; }
function accentRamp(base,mode,useNative) {
  return {
    base, mode,
    light1:useNative && nativeAppearance.accentLight1 ? nativeAppearance.accentLight1 : mix(base,"#ffffff",.12),
    light2:useNative && nativeAppearance.accentLight2 ? nativeAppearance.accentLight2 : mix(base,"#ffffff",.24),
    light3:useNative && nativeAppearance.accentLight3 ? nativeAppearance.accentLight3 : mix(base,"#ffffff",.42),
    dark1:useNative && nativeAppearance.accentDark1 ? nativeAppearance.accentDark1 : mix(base,"#000000",.12),
    dark2:useNative && nativeAppearance.accentDark2 ? nativeAppearance.accentDark2 : mix(base,"#000000",.24),
    dark3:useNative && nativeAppearance.accentDark3 ? nativeAppearance.accentDark3 : mix(base,"#000000",.38),
  };
}
function resolvedTheme(choice) {
  if (choice==="veiga-light") return accentRamp(VEIGA_ACCENT,"light",false);
  if (choice==="veiga-dark") return accentRamp(VEIGA_ACCENT,"dark",false);
  if (choice==="mean-girls") return accentRamp(MEAN_GIRLS_ACCENT,"light",false);
  if (choice==="ea-app") return {
    ...accentRamp(EA_APP_ACCENT,"dark",false),
    light1: "#3978FC",
    dark1: "#215BD8"
  };
  if (choice==="winui-light") return accentRamp(systemAccent(),"light",true);
  if (choice==="winui-dark") return accentRamp(systemAccent(),"dark",true);
  return accentRamp(systemAccent(),systemMode(),true);
}
function applyAccent(ramp) {
  const root=document.documentElement;
  root.style.setProperty("--accent",ramp.base);
  // RGB channel tokens preserve the original opacity of each UI affordance
  // while following the chosen theme (including the Windows system accent).
  root.style.setProperty("--accent-rgb",(parseRgb(ramp.base) || [127,173,45]).join(", "));
  root.style.setProperty("--accent-dark-rgb",(parseRgb(ramp.dark3) || [67,92,23]).join(", "));
  root.style.setProperty("--accent-light-1",ramp.light1);
  root.style.setProperty("--accent-light-2",ramp.light2);
  root.style.setProperty("--accent-light-3",ramp.light3);
  root.style.setProperty("--accent-dark-1",ramp.dark1);
  root.style.setProperty("--accent-dark-2",ramp.dark2);
  root.style.setProperty("--accent-dark-3",ramp.dark3);
  root.style.setProperty("--accent-low",alpha(ramp.base,ramp.mode==="dark"?.12:.09));
  root.style.setProperty("--accent-outline",alpha(ramp.base,ramp.mode==="dark"?.44:.34));
}
function applyThemeSwatches() {
  const surfaceFor = (choice, mode) => {
    if (choice === "mean-girls") return "#FEF9FD";
    if (choice === "ea-app") return "#1D2030";
    // These are the actual WinUI mica surfaces used by the generic light and
    // dark token sets.  The system sample therefore follows the OS mode.
    return mode === "dark" ? "#202020" : "#F3F3F3";
  };
  const palettes = [
    [".system-swatch", "system"],
    [".veiga-light-swatch", "veiga-light"],
    [".veiga-dark-swatch", "veiga-dark"],
    [".mean-girls-swatch", "mean-girls"],
    [".ea-app-swatch", "ea-app"],
    [".winui-light-swatch", "winui-light"],
    [".winui-dark-swatch", "winui-dark"],
  ];
  const setSwatch = (selector, choice) => {
    const palette = resolvedTheme(choice);
    const surface = surfaceFor(choice, palette.mode);
    const background = `linear-gradient(135deg, ${palette.base} 0 50%, ${surface} 50% 100%)`;
    document.querySelectorAll(selector).forEach((swatch) => {
      swatch.style.background = background;
      swatch.style.setProperty("--swatch-light", palette.base);
      swatch.style.setProperty("--swatch-dark", surface);
    });
  };
  palettes.forEach(([selector, choice]) => setSwatch(selector, choice));
}
function applyTheme(choice,{persist=true}={}) {
  if (!VALID_THEMES.has(choice)) choice="system";
  const resolved=resolvedTheme(choice);
  const root=document.documentElement;
  root.dataset.themeChoice=choice;
  root.dataset.resolvedMode=resolved.mode;
  applyAccent(resolved);
  applyThemeSwatches();
  document.querySelectorAll(".theme-option").forEach(option=>option.setAttribute("aria-checked",String(option.dataset.theme===choice)));
  if (persist) try { localStorage.setItem(THEME_KEY,choice); } catch {}
}
function currentChoice() {
  try {
    const saved=localStorage.getItem(THEME_KEY);
    return VALID_THEMES.has(saved) ? saved : "system";
  } catch { return "system"; }
}
async function refreshNativeAppearance() {
  try {
    const data=await invoke("get_system_appearance");
    if (data && typeof data==="object") nativeAppearance={...nativeAppearance,...data};
  } catch {
    nativeAppearance={...nativeAppearance,mode:null,accent:null,source:"web-fallback"};
  }
  applyTheme(document.documentElement.dataset.themeChoice || currentChoice(),{persist:false});
}

const themeButton=document.getElementById("theme-button");
const themeFlyout=document.getElementById("theme-flyout");
function closeThemeFlyout() {
  closeTooltip();
  themeFlyout?.classList.add("hidden");
  themeButton?.setAttribute("aria-expanded","false");
}
themeButton?.addEventListener("click",event=>{
  event.stopPropagation();
  if (!themeFlyout) return;
  const opening=themeFlyout.classList.contains("hidden");
  themeFlyout.classList.toggle("hidden",!opening);
  themeButton.setAttribute("aria-expanded",String(opening));
});
themeFlyout?.addEventListener("click",event=>event.stopPropagation());
document.querySelectorAll(".theme-option").forEach(option=>option.addEventListener("click",()=>{
  suppressTooltipUntilPointerLeave(themeButton);
  applyTheme(option.dataset.theme || "system");
  closeThemeFlyout();
}));
document.addEventListener("click",closeThemeFlyout);
document.addEventListener("keydown",event=>{ if (event.key==="Escape") closeThemeFlyout(); });
osDark.addEventListener?.("change",()=>{ if ((document.documentElement.dataset.themeChoice || "system")==="system") applyTheme("system",{persist:false}); });

const appWindow=getCurrentWindow();
const maximizeButton=document.getElementById("titlebar-maximize");
const captionLabels={
  en:{controls:"Window controls",minimize:"Minimize",maximize:"Maximize",restore:"Restore Down",close:"Close"},
  pt:{controls:"Controles da janela",minimize:"Minimizar",maximize:"Maximizar",restore:"Restaurar",close:"Fechar"},
  es:{controls:"Controles de ventana",minimize:"Minimizar",maximize:"Maximizar",restore:"Restaurar",close:"Cerrar"},
};
function applyCaptionLabels(isMaximized=false) {
  const labels=captionLabels[document.documentElement.lang] || captionLabels.en;
  document.querySelector(".winui-caption-buttons")?.setAttribute("aria-label",labels.controls);
  const minimize=document.getElementById("titlebar-minimize");
  const close=document.getElementById("titlebar-close");
  const setLabel=(element,label)=>{
    element?.setAttribute("aria-label",label);
    if (element) element.title=label;
  };
  setLabel(minimize,labels.minimize);
  setLabel(maximizeButton,isMaximized ? labels.restore : labels.maximize);
  setLabel(close,labels.close);
}
async function updateCaptionMaximize() {
  if(!maximizeButton) return;
  try {
    const isMaximized=await appWindow.isMaximized();
    maximizeButton.querySelector(".winui-maximize-glyph")?.toggleAttribute("hidden",isMaximized);
    maximizeButton.querySelector(".winui-restore-glyph")?.toggleAttribute("hidden",!isMaximized);
    applyCaptionLabels(isMaximized);
  } catch { /* Web fallback: static maximize glyph remains available. */ applyCaptionLabels(false); }
}
document.getElementById("titlebar-minimize")?.addEventListener("click",()=>appWindow.minimize());
maximizeButton?.addEventListener("click",async ()=>{
  await appWindow.toggleMaximize();
  await updateCaptionMaximize();
});
document.getElementById("titlebar-close")?.addEventListener("click",()=>appWindow.close());
appWindow.onResized(()=>{void updateCaptionMaximize();}).catch(()=>{});
appWindow.onFocusChanged(({payload})=>{ if(payload){ refreshNativeAppearance(); void updateCaptionMaximize(); } }).catch(()=>{});
window.addEventListener("s3cc-language-changed",()=>{ void updateCaptionMaximize(); });
void updateCaptionMaximize();

applyTheme(currentChoice(),{persist:false});
refreshNativeAppearance();
