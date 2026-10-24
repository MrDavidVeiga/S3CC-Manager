import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = path => readFileSync(path, "utf8");
const compression = read("src-tauri/src/compression.rs");
const dbpf = read("src-tauri/src/dbpf.rs");
const cache = read("src-tauri/src/cache.rs");
const health = read("src-tauri/src/health.rs");
const app = read("src/app.js");
const css = read("src/style.css");

// Health must validate the same actual SHA-256 content (no sampling-only
// fallback for new/modified packages) without caching decoded payload bytes.
assert.match(health, /get_or_build_health_with_metrics\(e\.path\(\),&mut cache\)/);
assert.match(health, /if error == operation::CANCELLED_ERROR/);
assert.match(cache, /pub fn get_or_build_health_with_metrics\(/);
assert.match(cache, /get_or_build_with_validation\(path, cache, false, Some\("health"\)\)/);
assert.match(cache, /sha256_file_health\(path\)\?/);
assert.match(cache, /const BLOCK: usize = 4 \* 1024 \* 1024/);
assert.match(cache, /package\.health_resource_digest\(entry\)/);
assert.match(cache, /ensure_analysis_not_cancelled\(cancellation_kind\)/);
assert.match(dbpf, /pub fn health_resource_digest\(/);
assert.match(dbpf, /read_exact_at\(&self\.data_file, offset, &mut buffer\[\.\.count\]\)/);
assert.match(dbpf, /uncompress_stream_with_digest_cancellable\(/);
assert.match(dbpf, /health_streams_a_multi_block_payload_with_exact_sha256_and_no_payload_cache/);
assert.match(compression, /chunks % 256 == 0 && cancelled\(\)/);
assert.match(compression, /uncompress_stream_with_digest_cancellable/);
assert.match(compression, /assert_eq!\(streamed_digest, digest\)/);

// Context menus must reveal the selected file in Explorer, never quarantine,
// delete, or edit a file, and must be attached to BOTH virtualized result rows.
assert.match(app, /function openAnalysisExplorerMenu\(event, members\)/);
assert.match(app, /openAnalysisExplorerMenu\(event, members\)/);
assert.match(app, /openAnalysisExplorerMenu\(event, \[finding\.left, finding\.right\]\.filter\(Boolean\)\)/);
assert.match(app, /openAnalysisExplorerMenu\(event, \[member\]\)/);
assert.match(app, /void revealSafe\(member\.path\)/);
assert.match(app, /activeAnalysisExplorerMenu/);
assert.match(app, /event\.key === "Escape"/);
assert.match(css, /\.analysis-explorer-context-menu/);
console.log("Beta 3 Elif Health streaming and Explorer context menu checks passed");
