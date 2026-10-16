import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const html = fs.readFileSync(path.join(root, "index.html"), "utf8");
const app = fs.readFileSync(path.join(root, "src", "app.js"), "utf8");
const theme = fs.readFileSync(path.join(root, "src", "theme.js"), "utf8");
const converter = fs.readFileSync(path.join(root, "src", "sims3pack-converter.js"), "utf8");
const fluent = fs.readFileSync(path.join(root, "src", "ui", "fluent", "theme-audit.css"), "utf8");

const expect = (condition, message) => assert.ok(condition, message);

expect(html.includes('id="duplicates-quarantine-preflight-btn"'), "Duplicates exposes a preflight action");
expect(html.includes('data-i18n="verifyQuarantine"'), "Preflight action is localized");
expect(!html.includes('id="conflicts-select-detected-btn"'), "Conflicts has no redundant select-for-review command");
expect(!html.includes('id="conflicts-select-all-btn"'), "Conflicts has no duplicate select-all command");
expect(!html.includes('id="conflicts-clear-review-btn"'), "Conflicts has no duplicate clear-selection command");

const toolsOrder = [
  html.indexOf('data-tools-tab="technical"'),
  html.indexOf('data-tools-tab="health"'),
  html.indexOf('data-tools-tab="maintenance"'),
  html.indexOf('data-tools-tab="history"'),
];
expect(toolsOrder.every((value) => value >= 0), "Visible tool tabs exist");
expect(toolsOrder.every((value, index) => index === 0 || value > toolsOrder[index - 1]), "Tools tabs are Converter, Health, Maintenance, History");
expect(html.includes('class="theme-button-label" data-i18n="theme"'), "Theme control includes a localized label");
expect(html.includes('class="social-icons-list"'), "Shared social footer remains present");

expect(app.includes("replaceAnalysisListKeepingScroll"), "Analysis list rendering preserves scroll position");
expect(!app.includes("el.duplicatesList.scrollTop = 0"), "Duplicates rendering does not reset scrollTop");
expect(app.includes("verifyQuarantinePreflight"), "Quarantine preflight is an explicit non-executing action");
expect(app.includes('invoke("open_external_url", { url })'), "Social links use the Tauri external URL command");
expect(theme.includes('closeTooltip'), "Theme selection closes tooltip state");
expect(theme.includes("linear-gradient(135deg"), "Theme swatches use two real palette colors");
expect(!theme.includes('swatch.style.background=systemAccent()'), "System theme swatches keep their bicolor treatment");

expect(converter.includes('chooseFiles:"Selecionar arquivos Sims3Pack"'), "Portuguese converter label is explicit");
expect(converter.includes('chooseFiles:"Select Sims3Pack files"'), "English converter label is explicit");
expect(converter.includes('chooseFiles:"Seleccionar archivos Sims3Pack"'), "Spanish converter label is explicit");
expect(fluent.includes("#duplicates-batch-preview.quarantine-preview"), "Quarantine preflight is compact and revealed on demand");
expect(fluent.includes(".winui-social-footer .social-icons-list a:focus-visible"), "Social keyboard focus remains visible");

console.log("Beta 2 final interface regressions: PASS");
