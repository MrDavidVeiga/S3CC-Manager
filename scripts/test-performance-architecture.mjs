import { readFileSync } from "node:fs";

const read = (path) => readFileSync(path, "utf8");
const operation = read("src-tauri/src/operation.rs");
const scanner = read("src-tauri/src/scanner.rs");
const cache = read("src-tauri/src/cache.rs");
const planner = read("src-tauri/src/planner.rs");
const unified = read("src-tauri/src/unified_analysis.rs");
const app = read("src/app.js");
const html = read("index.html");

const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

assert(operation.includes("spawn_blocking"), "Heavy commands must use a blocking worker pool.");
assert(operation.includes("begin_mutating"), "Mutating operations need a backend serialization lock.");
assert(operation.includes("run_blocking"), "Read-only worker helper is missing.");
assert(operation.includes("run_mutating_blocking"), "Mutating worker helper is missing.");
assert(operation.includes("OperationRun") && operation.includes("generation"), "Operation generations must isolate stale completions.");
assert(operation.includes("begin_mutating_scoped"), "Mutating writes need destination scopes.");
assert(cache.includes("file_sha256") && cache.includes("sha256_file"), "Cache invalidation must verify file content.");
assert(cache.includes("quick_fingerprint") && cache.includes("integrity_verified"), "Analysis and integrity cache paths must remain distinct.");
assert(read("src-tauri/src/cc_catalog.rs").includes('rename = "save_cc_catalog"'), "Catalog writes need mutation coordination.");
assert(read("src-tauri/src/health.rs").includes('rename = "remove_empty_folder"'), "Empty-folder removal needs mutation coordination.");
assert(scanner.includes("file_signatures") && scanner.includes("latest_scan_is_current"), "In-memory scans must validate external changes.");
assert(scanner.includes("PersistentScan") && cache.includes("analysis-snapshot-v1.json"), "Scanner classifications must survive restarts only when their signatures remain current.");
assert(scanner.includes("scan_packages_core_with_progress(folder, language, operation_kind, true, true, false)"), "Organizer scans must not prebuild full resource fingerprints.");
assert(scanner.includes("scan_packages_core_with_progress(folder, language, operation_kind, false, false, true)"), "Legacy unified callers must retain the complete shared fingerprint path.");
assert(unified.includes("analyze_duplicates_core") && unified.includes("analyze_conflicts_core"), "The compatibility unified path must retain shared-index analyzers.");
assert(planner.includes("get_or_build_identity_with_metrics"), "Plan preview must use its bounded identity cache before execution revalidates SHA-256.");
assert(cache.includes("get_or_build_identities_with_metrics"), "Large previews must resolve independent identity checks through the bounded batch helper.");
assert(planner.includes("get_or_build_identities_with_metrics"), "Planner must use the bounded identity batch instead of serial full-file hashing.");
assert(!planner.includes("get_or_build_analysis_with_metrics"), "Plan preview must not build resource fingerprints owned by Duplicates/Conflicts.");
assert(planner.includes("analysis_file_fingerprint"), "Plan preview must narrow same-size duplicate candidates before a full hash.");
assert(planner.includes("pub async fn build_organization_plan_with_cfg"), "Planner command must be asynchronous.");
assert(app.includes("beginToolsOperation") && app.includes("cancelToolsOperation"), "Tools operations need progress and cancellation wiring.");
assert(html.includes('id="tools-cancel"'), "Tools cancellation control is missing.");
assert(app.includes("requestAnimationFrame"), "Large Manager lists must retain batched rendering.");
assert(app.includes("el.planItems.replaceChildren(planItemsFragment)"), "Plan cards must be committed as one DOM batch.");
assert(app.includes("el.planTree.replaceChildren(planTreeFragment)"), "Before/after rows must be committed as one DOM batch.");

console.log(JSON.stringify({
  ok: true,
  checks: [
    "blocking-worker-pool",
    "mutation-serialization",
    "sha256-cache-invalidation",
    "scan-cache-external-change-validation",
    "unified-analysis",
    "async-planner",
    "bounded-planner-identities",
    "batched-plan-dom",
    "tools-progress-cancellation",
    "virtualized-rendering",
  ],
}));
