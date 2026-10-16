import assert from "node:assert/strict";
import fs from "node:fs";

const app = fs.readFileSync("src/app.js", "utf8");
const html = fs.readFileSync("index.html", "utf8");
const css = fs.readFileSync("src/style.css", "utf8");

assert.match(app, /duplicateReviewSelected: new Set\(\)/);
assert.match(app, /duplicateExpanded: new Set\(\)/);
assert.match(app, /conflictExpanded: new Set\(\)/);
assert.match(app, /function toggleDuplicateReviewFromRow\(finding, event\)/);
assert.match(app, /event\.shiftKey/);
assert.match(app, /event\.ctrlKey \|\| event\.metaKey/);
assert.match(app, /selectAllVisibleDuplicates\(\)/);
assert.match(app, /function appendDuplicateExpandableDetails/);
assert.match(app, /function appendConflictExpandableDetails/);
assert.match(app, /analysis-technical-details/);
assert.match(app, /state\.duplicateExpanded\.size/);
assert.match(app, /state\.conflictExpanded\.size/);

const duplicateRow = app.slice(app.lastIndexOf("function createDuplicateFindingRow"), app.indexOf("function renderDuplicateVirtualRows", app.lastIndexOf("function createDuplicateFindingRow")));
const conflictRow = app.slice(app.lastIndexOf("function createConflictFindingRow"), app.indexOf("function renderConflictVirtualRows", app.lastIndexOf("function createConflictFindingRow")));
assert.doesNotMatch(duplicateRow, /openDuplicateDetails\(\)/);
assert.doesNotMatch(conflictRow, /openConflictDetails\(\)/);
assert.match(html, /id="duplicates-sort"/);
assert.match(html, /id="conflicts-sort"/);
assert.match(css, /\.analysis-expandable-item/);
assert.match(css, /\.analysis-comparison-grid/);
assert.match(css, /\.analysis-technical-details/);

console.log("expandable Duplicates/Conflicts UI checks passed");
