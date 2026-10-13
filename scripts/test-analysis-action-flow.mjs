import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = file => readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
const html = read("index.html");
const css = read("src/style.css");
const app = read("src/app.js");

const duplicatesStart = html.indexOf('<div class="analysis-list-footer" data-report-panel="duplicates">');
const conflictsStart = html.indexOf('<div class="analysis-list-footer conflicts-footer" data-report-panel="conflicts">');
assert(duplicatesStart >= 0 && conflictsStart > duplicatesStart, "analysis footers must remain ordered");
const duplicates = html.slice(duplicatesStart, conflictsStart);
const conflictsEnd = html.indexOf("</section>", conflictsStart);
const conflicts = html.slice(conflictsStart, conflictsEnd);

assert.match(duplicates, /id="duplicates-select-exact-btn"[^>]*data-i18n="selectExactDuplicatesMain"/);
assert.match(duplicates, /id="duplicates-preview-all-btn"[^>]*data-i18n="viewQuarantine"/);
assert.match(duplicates, /<details class="analysis-more-actions">[\s\S]*data-i18n="moreActions"/);
for (const id of [
  "duplicates-select-current-btn",
  "duplicates-select-visible-btn",
  "duplicates-clear-selection-btn",
  "duplicates-clear-list-btn",
]) assert.match(duplicates, new RegExp(`id="${id}"`));
assert.equal((duplicates.match(/id="duplicates-(?:select-exact|preview-all)-btn"/g) || []).length, 2,
  "duplicates must expose exactly two primary action buttons");

assert.match(conflicts, /id="conflicts-select-detected-btn"[^>]*data-i18n="selectForReview"/);
assert.match(conflicts, /id="conflicts-review-selected-btn"[^>]*data-i18n="reviewSelected"/);
assert.match(conflicts, /<details class="analysis-more-actions">[\s\S]*data-i18n="moreActions"/);
for (const id of [
  "conflicts-select-all-btn",
  "conflicts-clear-review-btn",
  "conflicts-mark-intentional-btn",
  "conflicts-mark-reviewed-btn",
  "conflicts-mark-ignored-btn",
  "conflicts-preview-quarantine-btn",
  "conflicts-clear-list-btn",
]) assert.match(conflicts, new RegExp(`id="${id}"`));
assert.equal((conflicts.match(/id="conflicts-(?:select-detected|review-selected)-btn"/g) || []).length, 2,
  "conflicts must expose exactly two primary action buttons");

for (const key of [
  "moreActions",
  "selectExactDuplicatesMain",
  "viewQuarantine",
  "selectForReview",
  "reviewSelected",
  "previousConflict",
  "nextConflict",
]) {
  assert.equal((app.match(new RegExp(`\\b${key}:`, "g")) || []).length, 3,
    `${key} must be translated in PT, EN and ES`);
}

assert.match(html, /id="conflict-previous-btn"[^>]*data-i18n="previousConflict"/);
assert.match(html, /id="conflict-next-btn"[^>]*data-i18n="nextConflict"/);
assert.match(css, /\.analysis-more-actions-menu\s*\{[\s\S]*position:\s*absolute[\s\S]*max-height/);
assert.match(css, /\.analysis-action-bar\s*>\s*\.primary-btn,[\s\S]*flex:\s*1 1 190px/);
assert.match(css, /@media\s*\(max-width:\s*480px\)[\s\S]*\.analysis-more-actions/);
assert.match(app, /function reviewSelectedConflicts\(\)[\s\S]*prioritizeSelectedConflicts\(\)[\s\S]*openConflictDetails\(\)/);
assert.match(app, /function navigateConflictDetails\(direction\)[\s\S]*renderConflictsPreview\(\)/);
assert.match(app, /el\.conflictPreviousBtn\.addEventListener\("click",[\s\S]*navigateConflictDetails\(-1\)/);
assert.match(app, /el\.conflictNextBtn\.addEventListener\("click",[\s\S]*navigateConflictDetails\(1\)/);
assert.match(app, /el\.conflictsReviewSelectedBtn\.disabled = busy \|\| !state\.conflictReviewSelected\.size/);

console.log("Duplicates/Conflicts approved action-flow regression: PASS (two primary actions, accessible More Actions, filtered selection, navigation)");
