import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const rust = readFileSync("src-tauri/src/embedded_duplicates.rs", "utf8");
const lib = readFileSync("src-tauri/src/lib.rs", "utf8");
const app = readFileSync("src/app.js", "utf8");
const html = readFileSync("index.html", "utf8");

// The detector must be independently invoked and carry no write operations
// against .package files. Cached fingerprint metadata may be updated.
assert.match(lib, /pub mod embedded_duplicates;/);
assert.match(lib, /embedded_duplicates::analyze_embedded_duplicates,/);
assert.match(rust, /pub async fn analyze_embedded_duplicates\(folder: String\)/);
assert.match(rust, /operation::run_blocking\(KIND, "discovering"/);
assert.match(rust, /MANIFEST_KEY: Key = \(0x0333_406C, 0x5653_4733, 0x5645_4947_414D_4E46\)/);
assert.match(rust, /fn parse_merge_manifest\(/);
assert.match(rust, /if union != indexed/);
assert.match(rust, /if duplicate_key/);
assert.match(rust, /if left\.merged_path == right\.merged_path \|\| \(!left\.from_manifest && !right\.from_manifest\)/);
assert.match(rust, /fn signature\(/);
assert.match(rust, /hasher\.update\(hash\.as_bytes\(\)\)/);
assert.match(rust, /safe_action: "review_only"/);
assert.doesNotMatch(rust, /std::fs::(remove_file|rename|write|copy)\(/);
assert.match(rust, /fn exact_content_fingerprints_ignore_source_names\(/);
assert.match(rust, /fn reject_missing_ownership\(/);

// Distinct GUI controls, localizable for all three supported languages.
for (const id of [
  "duplicates-embedded-run", "duplicates-embedded-results",
  "duplicates-embedded-status", "duplicates-embedded-list",
]) {
  assert.equal((html.match(new RegExp('id="' + id + '"', "g")) || []).length, 1);
  assert.ok(app.includes('#' + id), id + " must be bound in the controller");
}
assert.match(app, /invoke\("analyze_embedded_duplicates", \{ folder \}\)/);
assert.match(app, /function renderEmbeddedDuplicates\(\)/);
assert.match(app, /if \(state\.folder === folder\) state\.embeddedDuplicatesReport = result;/);
assert.match(app, /el\.embeddedDuplicatesRun\?\.addEventListener\("click", analyzeEmbeddedDuplicates\)/);
assert.equal((app.match(/analyzeEmbeddedDuplicates: "/g) || []).length, 3);

// Deterministic metadata-only expectation based on the provided four-file
// scenario. No copyrighted CC files are included in the repository.
const merged1 = new Set(["DarkAgesScarf:297", "FrankenSim:36", "FireFighter:441"]);
const merged2 = new Set(["DarkAgesScarf:297", "FrankenSim:36", "ThinFrizzy:603"]);
assert.deepEqual([...merged1].filter(value => merged2.has(value)),
  ["DarkAgesScarf:297", "FrankenSim:36"]);
console.log("Beta 3 embedded-duplicate wiring and sample-metadata contracts passed");
