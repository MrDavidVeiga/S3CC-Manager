import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const app = readFileSync("src/app.js", "utf8");
const health = readFileSync("src-tauri/src/health.rs", "utf8");
const css = readFileSync("src/style.css", "utf8");

// Extract and exercise the actual virtual-list implementation, not a
// rewritten approximation. No browser or installed test dependency needed.
const start = app.indexOf("// Keep a single expanded result in a virtualized list.");
const end = app.indexOf("function renderDuplicateVirtualRows(", start);
assert(start >= 0 && end > start, "Missing shared virtualized expander implementation");
const code = app.slice(start, end);
const sandbox = {
  ANALYSIS_ROW_STRIDE: 64,
  ANALYSIS_ROW_GAP: 9,
  ANALYSIS_COLLAPSED_HEIGHT: 55,
  VIRTUAL_OVERSCAN: 10,
  document: {
    createElement: (tag) => ({ tag, dataset: {}, textContent: "", className: "" }),
    createDocumentFragment: () => ({
      children: [],
      appendChild(child) { this.children.push(child); },
    }),
  },
  virtualSpacer: (height) => ({ spacer: true, height }),
  t: (key) => key,
};
vm.createContext(sandbox);
vm.runInContext(code, sandbox);
const { analysisVirtualRange, renderExpandableAnalysisRows } =
  vm.runInContext("({ analysisVirtualRange, renderExpandableAnalysisRows })", sandbox);

const findings = Array.from({ length: 10000 }, (_, index) => ({ id: `finding-${index}` }));
const container = {
  dataset: {},
  scrollTop: 0,
  clientHeight: 540,
  children: [],
  replaceCount: 0,
  replaceChildren(child) {
    this.children = child?.children || [child];
    this.replaceCount += 1;
    // The browser retains its scroll position; code must never forcibly
    // overwrite the scrollTop being controlled by the user.
  },
};
const view = { items: null, start: -1, end: -1, raf: 0 };
let expandedId = "";
const buildRow = (finding) => ({
  id: finding.id,
  expanded: finding.id === expandedId,
  dataset: {},
  getBoundingClientRect() {
    return { height: this.expanded ? 455 : 55 };
  },
});

renderExpandableAnalysisRows(container, view, findings, expandedId, buildRow, "duplicateId");
assert(container.children.length < 50, "Collapsed list must stay virtualized");
assert.equal(container.scrollTop, 0);

container.scrollTop = 800 * 64;
renderExpandableAnalysisRows(container, view, findings, expandedId, buildRow, "duplicateId");
assert.equal(container.scrollTop, 800 * 64, "Dragging scrollbar must not be overwritten");

expandedId = "finding-800";
const beforeExpand = container.replaceCount;
renderExpandableAnalysisRows(container, view, findings, expandedId, buildRow, "duplicateId");
assert(container.replaceCount > beforeExpand, "Opening details must re-render");
assert(container.children.length < 50, "One expanded row must not render 10000 rows");
assert(container.children.some((row) => row.id === expandedId && row.expanded));
assert.equal(view.extraHeight, 400, "Expanded result height must contribute to scrollbar geometry");

container.scrollTop = 930 * 64 + view.extraHeight;
renderExpandableAnalysisRows(container, view, findings, expandedId, buildRow, "duplicateId");
assert.equal(container.scrollTop, 930 * 64 + 400, "Scrolling below expanded item must remain stable");
assert(container.children.length < 50);
const expectedRange = analysisVirtualRange(container, findings.length, 800, 400);
assert(expectedRange.start >= 800 && expectedRange.end < 1000, "Expanded offset must adjust visible row indices");
assert(container.children[0]?.spacer && container.children[0].height > 800 * 64, "Top spacer must retain height of expanded item");

container.scrollTop = 800 * 64;
renderExpandableAnalysisRows(container, view, findings, expandedId, buildRow, "duplicateId");
expandedId = "";
const beforeCollapse = container.replaceCount;
renderExpandableAnalysisRows(container, view, findings, expandedId, buildRow, "duplicateId");
assert(container.replaceCount > beforeCollapse, "Closing details must invalidate virtual range");
assert.equal(view.expandedId, "");
assert.equal(view.extraHeight, 0);
assert(!container.children.some((row) => row.expanded), "Details must really collapse");
assert.equal(container.scrollTop, 800 * 64);

// Stress boundary rows and viewport transitions: the virtualized DOM must
// retain the same total scroll geometry as a fully rendered 10,000-row list.
// 55px collapsed row + 9px gap = 64px stride; outer padding = 16px.
for (const index of [0, 1, 9, 10, 100, 999, 5000, 9999]) {
  expandedId = `finding-${index}`;
  container.scrollTop = index * 64;
  renderExpandableAnalysisRows(container, view, findings, expandedId, buildRow, "duplicateId", true);
  const height = container.children.reduce((sum, child) =>
    sum + (child.spacer ? child.height : child.expanded ? 455 : 55), 0
  ) + Math.max(0, container.children.length - 1) * 9 + 16;
  assert.equal(height, findings.length * 64 + 7 + view.extraHeight,
    "Virtualized scrollbar geometry must stay identical to the full list");
  assert(container.children.length < 50, "Expanding at any index must retain bounded DOM size");
}
expandedId = "";
renderExpandableAnalysisRows(container, view, findings, expandedId, buildRow, "duplicateId");

// The identical helper is used by both independent pages.
assert.match(app, /function renderDuplicateVirtualRows[^]*?renderExpandableAnalysisRows\(/);
assert.match(app, /function renderConflictVirtualRows[^]*?renderExpandableAnalysisRows\(/);
assert.doesNotMatch(app.slice(start, end), /requestAnimationFrame\([^]*?scrollTop\s*=/);
assert.match(css, /\.duplicates-list,\s*\.conflicts-list\s*\{\s*overflow-anchor:\s*none;/);

// Health cancellation and real phase progress, including directory scans.
assert.match(health, /operation::set_total\("health", file_entries\.len\(\)\.saturating_add\(4\)\)/);
assert.match(health, /operation::update\("health", 0, None, "discovering"\)/);
assert.match(health, /operation::update\("health", processed_files\.saturating_add\(3\), None, "finalizing"\)/);
assert((health.match(/operation::is_cancelled\("health"\)/g) || []).length >= 8);
assert.match(app, /state\.toolsNotice = t\("cancelled"\)/);

console.log("Beta 3 Elif stability regression tests passed");
