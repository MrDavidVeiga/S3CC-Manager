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
expect(theme.includes('suppressTooltipUntilPointerLeave'), "Theme selection suppresses tooltip re-opening until a new interaction");
expect(theme.includes("linear-gradient(135deg"), "Theme swatches use two real palette colors");
expect(!theme.includes('swatch.style.background=systemAccent()'), "System theme swatches keep their bicolor treatment");

expect(converter.includes('chooseFiles:"Selecionar .sims3pack"'), "Portuguese converter label is compact");
expect(converter.includes('chooseFiles:"Select .sims3pack"'), "English converter label is compact");
expect(converter.includes('chooseFiles:"Seleccionar .sims3pack"'), "Spanish converter label is compact");
expect(html.includes('class="theme-swatch winui-light-swatch"'), "WinUI Light has its own swatch");
expect(html.includes('class="theme-swatch winui-dark-swatch"'), "WinUI Dark has its own swatch");
expect(app.includes('body.classList.toggle("utility-page"'), "Tools and Restore use the full-width layout");
expect(theme.includes('surfaceFor'), "Theme swatches pair the accent with the actual surface");
expect(fluent.includes("#duplicates-batch-preview.quarantine-preview"), "Quarantine preflight is compact and revealed on demand");
expect(fluent.includes(".winui-social-footer .social-icons-list a:focus-visible"), "Social keyboard focus remains visible");
const css = fs.readFileSync(path.join(root, "src", "style.css"), "utf8");
expect(/\.operation-result-counts\s*\{\s*display:\s*flex;/.test(css), "Result counters center the cards themselves");
expect(/\.duplicates-action-bar\s*\{[\s\S]*?grid-template-columns:/.test(css), "Duplicate actions remain in one coherent action band");
expect(/\.conflicts-action-primary\s*\{[\s\S]*?grid-template-columns:/.test(css), "Conflict actions use the same proportional grid");
expect(/\.analysis-query-controls\s*\{[\s\S]*?display:\s*grid;/.test(css), "Analysis search controls prioritize the search field");
expect(/body\.utility-page\.right-panel\s*\{|body\.utility-page\s+\.right-panel\s*\{/.test(css) &&
  /body\.release-v1\.utility-page\s+\.right-panel\s*\{[\s\S]*?border-radius:\s*0/.test(fs.readFileSync(path.join(root, "src", "ui", "fluent", "theme-audit.css"), "utf8")),
  "Utility pages use a rectangular full-width surface");
expect(app.includes("planningStatus"), "Plan preparation exposes the real operation status");
expect(app.includes('kind === "planning"'), "Plan preparation supports cooperative cancellation");
expect(app.includes('planning: ""'), "Plan preparation does not duplicate its phase below the counter");

console.log("Beta 2 final interface regressions: PASS");
