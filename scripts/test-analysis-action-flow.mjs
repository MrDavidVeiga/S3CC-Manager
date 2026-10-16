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

assert.match(duplicates, /id="duplicates-review-selection-btn"[^>]*data-i18n="reviewSelection"/);
assert.match(duplicates, /<details class="analysis-more-actions">[\s\S]*data-i18n="moreActions"/);
for (const id of [
  "duplicates-select-exact-btn",
  "duplicates-select-current-btn",
  "duplicates-select-visible-btn",
  "duplicates-clear-selection-btn",
  "duplicates-preview-all-btn",
  "duplicates-restore-files-btn",
  "duplicates-history-btn",
  "duplicates-open-folder-btn",
]) assert.match(duplicates, new RegExp(`id="${id}"`));
assert.doesNotMatch(duplicates, /id="duplicates-clear-list-btn"/);
for (const key of ["selectionSection", "managementSection", "toolsSection"]) {
  assert.match(duplicates, new RegExp(`data-i18n="${key}"`));
}
assert.equal((duplicates.match(/class="analysis-action-bar/g) || []).length, 1,
  "duplicates must expose one review action bar below the result list");

assert.match(conflicts, /id="conflicts-select-detected-btn"[^>]*data-i18n="selectForReview"/);
assert.match(conflicts, /id="conflicts-review-selected-btn"[^>]*data-i18n="reviewSelected"/);
assert.match(conflicts, /<details class="analysis-more-actions">[\s\S]*data-i18n="moreActions"/);
for (const id of [
  "conflicts-select-all-btn",
  "conflicts-clear-review-btn",
  "conflicts-mark-intentional-btn",
  "conflicts-clear-marks-btn",
  "conflicts-show-pending-btn",
  "conflicts-show-intentional-btn",
  "conflicts-show-ignored-btn",
  "conflicts-history-btn",
  "conflicts-resourcecfg-btn",
]) assert.match(conflicts, new RegExp(`id="${id}"`));
assert.doesNotMatch(conflicts, /id="conflicts-clear-list-btn"/);
assert.doesNotMatch(conflicts, /id="conflicts-preview-quarantine-btn"/);
assert.equal((conflicts.match(/id="conflicts-(?:select-detected|review-selected)-btn"/g) || []).length, 2,
  "conflicts must expose exactly two primary action buttons");

for (const key of [
  "moreActions",
  "reviewSelection",
  "selectSafeExcessCopies",
  "showPendingConflicts",
  "showIntentionalOverrides",
  "showIgnoredConflicts",
  "consultResourceCfg",
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
assert.match(css, /\.analysis-more-actions-menu button\s*\{[\s\S]*width:\s*100%/,
  "nested More Actions buttons must share the menu width");
assert.match(css, /\.analysis-action-bar\s*>\s*\.primary-btn,[\s\S]*flex:\s*1 1 190px/);
assert.match(css, /@media\s*\(max-width:\s*480px\)[\s\S]*\.analysis-more-actions/);
assert.match(app, /function reviewSelectedConflicts\(\)[\s\S]*conflictExpanded\.add/,
  "conflict review must expand selected findings inline");
assert.match(app, /function reviewSelectedDuplicates\(\)[\s\S]*duplicateExpanded\.add/,
  "duplicate review must expand selected findings inline");
assert.match(app, /function openExistingQuarantine\(\)[\s\S]*invoke\("list_existing_quarantine"/,
  "View Quarantine must read existing files instead of preparing a new transaction");
assert.match(app, /el\.duplicatesPreviewAllBtn\.addEventListener\("click", openExistingQuarantine\)/);
assert.match(app, /el\.conflictsSelectDetectedBtn\.addEventListener\("click", selectAllVisibleConflicts\)/,
  "primary conflict selection must respect the active filter");
assert.match(app, /function navigateDuplicateReview\(direction\)/);
assert.match(app, /function visibleConflictFindings\(includeSelectedOnly = true\)/);
assert.match(app, /selectedVisibleOfTotal/);
assert.match(app, /function openConflictResourceCfg\(\)[\s\S]*analyzeHealth\(\)/);
assert.match(app, /function setConflictView\(filter\)/);
assert.match(app, /function navigateConflictDetails\(direction\)[\s\S]*renderConflictsPreview\(\)/);
assert.match(app, /el\.conflictPreviousBtn\.addEventListener\("click",[\s\S]*navigateConflictDetails\(-1\)/);
assert.match(app, /el\.conflictNextBtn\.addEventListener\("click",[\s\S]*navigateConflictDetails\(1\)/);
assert.match(app, /el\.conflictsReviewSelectedBtn\.disabled = busy \|\| !state\.conflictReviewSelected\.size/);

console.log("Duplicates/Conflicts approved action-flow regression: PASS (two primary actions, accessible More Actions, filtered selection, navigation)");
