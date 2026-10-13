import { readFileSync } from "node:fs";

const read = (path) => readFileSync(path, "utf8");
const operation = read("src-tauri/src/operation.rs");
const scanner = read("src-tauri/src/scanner.rs");
const cache = read("src-tauri/src/cache.rs");
const planner = read("src-tauri/src/planner.rs");
const app = read("src/app.js");
const html = read("index.html");

const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

assert(operation.includes("spawn_blocking"), "Heavy commands must use a blocking worker pool.");
assert(operation.includes("begin_mutating"), "Mutating operations need a backend serialization lock.");
assert(operation.includes("run_blocking"), "Read-only worker helper is missing.");
assert(operation.includes("run_mutating_blocking"), "Mutating worker helper is missing.");
assert(cache.includes("file_sha256") && cache.includes("sha256_file"), "Cache invalidation must verify file content.");
assert(scanner.includes("file_signatures") && scanner.includes("latest_scan_is_current"), "In-memory scans must validate external changes.");
assert(planner.includes("pub async fn build_organization_plan_with_cfg"), "Planner command must be asynchronous.");
assert(app.includes("beginToolsOperation") && app.includes("cancelToolsOperation"), "Tools operations need progress and cancellation wiring.");
assert(html.includes('id="tools-cancel"'), "Tools cancellation control is missing.");
assert(app.includes("requestAnimationFrame"), "Large Manager lists must retain batched rendering.");

console.log(JSON.stringify({
  ok: true,
  checks: [
    "blocking-worker-pool",
    "mutation-serialization",
    "sha256-cache-invalidation",
    "scan-cache-external-change-validation",
    "async-planner",
    "tools-progress-cancellation",
    "virtualized-rendering",
  ],
}));
