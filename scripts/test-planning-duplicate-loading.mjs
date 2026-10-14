import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "..");
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");

const planner = read("src-tauri/src/planner.rs");
const executor = read("src-tauri/src/executor.rs");
const app = read("src/app.js");
const html = read("index.html");
const css = read("src/style.css");
const converter = read("src/sims3pack-converter.js");

assert.match(planner, /duplicate_quarantine_ready/);
assert.match(planner, /ready_duplicate_quarantine/);
assert.match(planner, /duplicate_quarantine_file_name/);
assert.match(planner, /confirmed_quarantine_keeper/);
assert.match(planner, /blocked_duplicate_no_keeper/);
assert.match(planner, /\.package\.duplicated/);
assert.match(planner, /mark_intra_plan_destination_collisions\(/);
assert.match(planner, /resolve_intra_plan_source_conflicts\(/);
assert.match(planner, /duplicate_source_skipped/);
assert.match(planner, /blocked_duplicate_source_conflict/);
assert.doesNotMatch(planner, /move_intra_plan_conflicts_to_review/);
assert.match(planner, /source stays in place; no file is overwritten/);
assert.match(executor, /Plan contains multiple ready operations for the same destination/);
assert.match(executor, /Plan contains multiple ready operations for the same source/);
assert.match(executor, /integrity_check/);
assert.match(app, /function preparePlanInBackground\(/);
assert.match(app, /planPreparationPromise/);
assert.match(app, /state\.planKey === requestedKey/);
assert.match(app, /build_organization_plan_with_cfg/);
assert.equal((app.match(/invoke\("build_organization_plan_with_cfg"/g) || []).length, 1,
  "the UI must have one deduplicated planner invocation path");
assert.match(app, /loading-overlay/);
assert.match(app, /loadingPhaseDuplicates/);
assert.match(app, /continueLoadingInBackground/);
assert.match(app, /cancelLoadingOperation/);
assert.match(app, /s3cc-operation-progress/);
assert.match(app, /planStatDuplicatesReady/);
assert.match(html, /id="loading-overlay"/);
assert.match(html, /id="loading-overlay-continue"/);
assert.match(html, /id="loading-overlay-cancel"/);
assert.match(css, /\.loading-overlay/);
assert.match(css, /pointer-events: auto/);
assert.doesNotMatch(css, /\.loading-overlay\[data-blocking="true"\]\s*\{\s*pointer-events: auto/);
assert.match(converter, /emitOperationProgress/);
assert.match(converter, /sims3pack_inspect/);
assert.match(converter, /sims3pack_convert/);

console.log("planning duplicate quarantine and loading overlay checks passed");
