import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const app = readFileSync("src/app.js", "utf8");
const html = readFileSync("index.html", "utf8");
const css = readFileSync("src/style.css", "utf8");

for (const id of [
  "select-all-btn", "select-none-btn",
  "duplicates-select-visible-review-btn", "duplicates-clear-review-visible-btn",
  "conflicts-select-visible-main-btn", "conflicts-clear-review-main-btn",
]) assert.match(html, new RegExp(`id="${id}"`));

assert.match(app, /function applyManagerSelection\(item, event, checked = null\)/);
assert.match(app, /event\.shiftKey/);
assert.match(app, /event\.ctrlKey \|\| event\.metaKey/);
assert.match(app, /lastManagerSelectionAnchor/);
assert.match(app, /packageList\.addEventListener\("keydown"/);
assert.match(app, /hiddenSelections/);
assert.match(app, /function cachedExactDuplicatePreview\(member\)/);
assert.match(app, /candidate\.fileSha256.*hash/);
assert.match(app, /loadingModeCold/);
assert.match(app, /loadingModeReuse/);
assert.match(app, /loadingModeIncremental/);
assert.match(app, /data-i18n="actions"|actions: "Actions"/);
assert.match(app, /function isAlreadyQuarantinedMember\(member\)/);
assert.match(app, /activeExactMembers\(group\)/);
assert.match(app, /function formatModifiedAt\(value\)/);
assert.match(app, /sortModified/);
assert.match(html, /id="conflicts-sort"[\s\S]*value="modified"/);
assert.match(html, /data-i18n="actions"/);
assert.match(css, /\.analysis-expandable-date/);
assert.match(css, /\.analysis-summary-secondary/);
assert.match(app, /visibleProcessed/);
assert.match(css, /\.loading-mode/);
assert.match(css, /\.analysis-expandable-details[\s\S]*min-width:\s*0/);
assert.match(css, /\.analysis-technical-body[\s\S]*overflow-wrap:\s*anywhere/);

console.log("final UI selection, expansion, thumbnail and loading checks passed");
