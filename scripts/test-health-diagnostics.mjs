import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
const html = readFileSync("index.html", "utf8");
const app = readFileSync("src/app.js", "utf8");
const health = readFileSync("src-tauri/src/health.rs", "utf8");
const reports = readFileSync("src-tauri/src/audit_report.rs", "utf8");
const ids = [
  "tools-health-filter", "tools-health-severity", "tools-health-select-visible",
  "tools-health-clear-selection", "tools-health-export", "tools-health-selection-count",
];
for (const id of ids) {
  assert.match(html, new RegExp('id="' + id + '"'), 'Missing HTML control: ' + id);
  assert.equal((html.match(new RegExp('id="' + id + '"', 'g')) || []).length, 1,
    'Health control ID must be unique: ' + id);
  assert.ok(app.includes('#' + id), 'Unbound JS control: ' + id);
}
for (const fn of ["visibleHealthFindings", "renderHealthTools", "exportSelectedHealth"]) {
  assert.ok(app.includes('function ' + fn + '('), 'Missing controller ' + fn);
}
for (const category of [
  "invalid_header", "unsupported_version", "damaged_index", "unreadable_package",
  "empty_package", "repeated_tgi", "uncovered_resource_cfg", "disabled_file",
  "dbc_not_analyzed", "empty_folder", "outside_packages", "scan_error",
]) {
  assert.ok(health.includes('"' + category + '"'), 'Missing Health category: ' + category);
}
assert.ok(health.includes("HashSet"), "Repeated TGIs must be checked against a set");
assert.ok(
  health.includes("get_or_build_analysis_with_metrics") || health.includes("get_or_build_with_metrics"),
  "Health must reuse the validated DBPF cache",
);
assert.ok(health.includes("coverage_known"), "Resource.cfg coverage must distinguish unknown from uncovered");
assert.ok(health.includes('"resource_cfg_error"'), "Resource.cfg read failures must remain visible");
assert.ok(health.includes("follow_links(false)"), "Health traversal must not follow links");
assert.ok(health.includes("#[cfg(test)]"), "Rust regression coverage required");
assert.ok(reports.includes('"health" => "Health"'), "Health report type is missing");
assert.ok(app.includes('reportKind: "health"'), "Export payload needs scoped reportKind");
assert.ok(app.includes('state.healthSelected.add(item.id)'), "Selection must use stable IDs");
assert.ok(app.includes('state.healthFilter === "all"'), "Category filtering must allow all");
assert.ok(app.includes('state.healthSeverity === "all"'), "Severity filtering must allow all");
assert.ok(!html.includes('tools-subtab hidden" data-tools-tab="health"'), "Health tab is hidden");
console.log("Health diagnostics wiring regression: PASS");
