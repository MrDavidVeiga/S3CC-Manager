import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = file => readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
const html = read("index.html");
const css = read("src/style.css");
const fluent = read("src/ui/fluent/button-geometry.css");
const app = read("src/app.js");

const footer = html.match(/<div class="manager-list-footer-actions">([\s\S]*?)<\/div>/);
assert(footer, "Organizer list action footer must exist");
const ids = [...footer[1].matchAll(/id="([^"]+)"/g)].map(match => match[1]);
assert.deepEqual(ids, ["plan-btn", "open-organized-folder-btn", "clear-list-btn"],
  "the three Organizer actions must remain in their canonical DOM order");
const listStart = html.indexOf('<div id="package-list"');
const footerStart = html.indexOf('<div class="manager-plan-footer">');
assert(listStart >= 0 && footerStart > listStart, "actions must follow the package list in the DOM");
assert.match(css, /\.manager-list-footer-actions\s*\{[\s\S]*?grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\)/);
assert.match(fluent, /#page-organizer \.manager-list-footer-actions\s*\{[\s\S]*?grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\)/);
assert.match(css, /\.release-v1 #page-organizer \.manager-list-card[\s\S]*?display:\s*flex;\s*flex-direction:\s*column/);
assert.match(css, /\.release-v1 #page-organizer \.manager-list-card \.package-list[\s\S]*?overflow-y:\s*auto/);
assert.match(css, /\.release-v1 #page-organizer \.content-filter-current\s*\{[\s\S]*?display:\s*block;[\s\S]*?flex:\s*1\s+1\s+auto;[\s\S]*?min-width:\s*0;[\s\S]*?color:\s*var\(--text\)/,
  "the closed Organizer filter must reserve visible space for the active label");
assert.match(app, /el\.statusFilterCurrent\.textContent\s*=\s*`\$\{active\[1\]\} \(\$\{counts\[active\[0\]\] \?\? 0\}\)`/,
  "the closed Organizer filter must display the active localized label and count");
assert.doesNotMatch(css, /@media\s*\(max-width:\s*760px\)\s*\{[\s\S]*?\.manager-list-footer-actions\s*,/);
assert.match(html, /id="plan-partial-notice" class="planner-partial-notice hidden"/);
assert.match(html, /id="plan-cfg-row" class="planner-section planner-cfg-section"/);
assert.match(html, /id="plan-cfg-summary" class="planner-cfg-summary"/);
assert.doesNotMatch(html, /id="plan-cfg-row"[^>]*style=/);
assert.doesNotMatch(html, /id="plan-cfg-summary"[^>]*style=/);
assert.match(css, /\.planner-modal\s*\{[\s\S]*?grid-template-rows:\s*auto auto auto auto minmax\(0,\s*1fr\) auto/);
assert.match(css, /\.planner-partial-notice\s*\{[\s\S]*?height:\s*auto;[\s\S]*?overflow-wrap:\s*anywhere/);
assert.match(css, /\.planner-cfg-summary\s*\{[\s\S]*?white-space:\s*pre-wrap;[\s\S]*?overflow:\s*auto/);
const cfgHandler = app.match(/el\.planCfgCheckbox\.addEventListener\("change",[\s\S]*?el\.reviewDuplicatesBtn/);
assert(cfgHandler, "Resource.cfg checkbox handler must remain present");
assert.match(cfgHandler[0], /preview_resource_cfg_for_plan/);
assert.match(cfgHandler[0], /const existingPlan = state\.plan/);
assert.doesNotMatch(cfgHandler[0], /\bbuildPlan\s*\(/,
  "Resource.cfg toggles must not rebuild the full organization plan");
assert.doesNotMatch(cfgHandler[0], /state\.plan\s*=\s*null/,
  "Resource.cfg toggles must preserve the existing plan while refreshing its preview");
for (const id of ids) assert.match(footer[1], new RegExp(`id="${id}"`));
console.log("Organizer three-action layout regression: PASS (one row, outside scrollable list, responsive label wrapping)");
