import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = file => readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
const html = read("index.html");
const css = read("src/style.css");
const fluent = read("src/ui/fluent/button-geometry.css");

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
assert.doesNotMatch(css, /@media\s*\(max-width:\s*760px\)\s*\{[\s\S]*?\.manager-list-footer-actions\s*,/);
for (const id of ids) assert.match(footer[1], new RegExp(`id="${id}"`));
console.log("Organizer three-action layout regression: PASS (one row, outside scrollable list, responsive label wrapping)");
