import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relative) => readFileSync(path.join(root, relative), "utf8");

const app = read("src/app.js");
const unified = read("src-tauri/src/unified_analysis.rs");
const scanner = read("src-tauri/src/scanner.rs");
const cache = read("src-tauri/src/cache.rs");
const planner = read("src-tauri/src/planner.rs");
const executor = read("src-tauri/src/executor.rs");
const html = read("index.html");

assert.match(unified, /scan_packages_core_quiet_progress/);
assert.match(unified, /analyze_duplicates_core/);
assert.match(unified, /analyze_conflicts_core/);
assert.match(unified, /unified_cache_reused/);
assert.match(unified, /UnifiedAnalysisStats::default\(\)/);
assert.match(unified, /assert_eq!\(second\.stats\.duplicate_passes, 0\)/);
assert.match(unified, /assert_eq!\(second\.stats\.conflict_passes, 0\)/);
assert.match(unified, /reconcile_organization_moves/);
assert.match(unified, /duplicate_passes = 0/);
assert.match(unified, /conflict_passes = 0/);
assert.match(scanner, /reconcile_latest_scan_moves/);
assert.match(cache, /relocate_cached_entries/);
assert.match(scanner, /PersistentScan/);
assert.match(cache, /analysis-snapshot-v1\.json/);
assert.match(planner, /get_or_build_identity_with_metrics/);
assert.doesNotMatch(planner, /get_or_build_analysis_with_metrics/);
assert.match(planner, /analysis_file_fingerprint/);
assert.match(planner, /Executor preflight still hashes every ready source/);
assert.match(executor, /universal_cleanup_roots/);
assert.doesNotMatch(executor, /requires_post_execution_package_scan/);

assert.match(app, /async function runOrganizerAnalysis/);
assert.match(app, /runOrganizerAnalysis[\s\S]*invoke\("scan_packages"/);
assert.equal((app.match(/invoke\("analyze_unified"/g) || []).length, 0,
  "The three page buttons must not invoke the eager unified analysis");
const duplicatesAction = app.slice(app.indexOf("async function analyzeDuplicates"), app.indexOf("async function analyzeConflicts"));
const conflictsAction = app.slice(app.indexOf("async function analyzeConflicts"), app.indexOf("\nfunction selectAllVisible()"));
assert.match(duplicatesAction, /invoke\("analyze_duplicates"/);
assert.doesNotMatch(duplicatesAction, /runUnifiedAnalysis/);
assert.match(conflictsAction, /invoke\("analyze_conflicts"/);
assert.doesNotMatch(conflictsAction, /runUnifiedAnalysis/);
assert.match(app, /scanCancelBtn\.addEventListener\("click", \(\) => cancelAnalysis\("scan"\)/);
assert.match(app, /duplicatesCancelBtn\.addEventListener\("click", \(\) => cancelAnalysis\("duplicates"\)/);
assert.match(app, /conflictsCancelBtn\.addEventListener\("click", \(\) => cancelAnalysis\("conflicts"\)/);
assert.doesNotMatch(app, /runAutomaticAnalyses/);
assert.match(app, /showOperationResult/);
assert.match(html, /id="operation-result-modal"/);
assert.match(app, /sharedAnalysisReconciled = result\.analysisReconciled === true/);
assert.match(app, /const reconciledAnalysis = result\.reconciledAnalysis/);
assert.match(app, /state\.duplicatesAnalysis = reconciledAnalysis\.duplicates/);
assert.match(app, /state\.conflictsAnalysis = reconciledAnalysis\.conflicts/);
assert.match(app, /state\.analysisStatus\.duplicates = sharedAnalysisReconciled \? "completed" : "stale"/);

const functionSlice = (name) => {
  const start = app.indexOf(`async function ${name}`);
  assert.notEqual(start, -1, `${name} must exist`);
  const next = app.indexOf("\nasync function ", start + 1);
  return app.slice(start, next === -1 ? app.length : next);
};

for (const name of [
  "executeQuarantine",
  "executeConflictQuarantine",
  "restoreQuarantineFromHistory",
  "recoverQuarantineFromHistory",
  "executeInboxImport",
]) {
  const implementation = functionSlice(name);
  assert.doesNotMatch(implementation, /scanFolder\(/,
    `${name} must invalidate results without reopening the package corpus`);
  assert.match(implementation, /refreshAfterStructureChange/,
    `${name} must mark shared analysis results stale after a write`);
}
assert.match(app, /async function refreshAfterStructureChange[\s\S]*refreshOperationHistory[\s\S]*refreshCacheInfo/);

console.log("unified analysis, cached plan preview, mutation invalidation, cleanup and result-modal regressions passed");
