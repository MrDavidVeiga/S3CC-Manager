import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";

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

const start = app.indexOf("let activeAnalysisExplorerMenu = null;");
const end = app.indexOf("async function openDirectorySafe(", start);
assert(start > 0 && end > start, "Explorer context menu must be extractable for interaction tests");
const handlers = new Map();
const reveals = [];
let activeElement = null;
class FakeNode {
  constructor(tag) {
    this.tag = tag;
    this.children = [];
    this.attrs = {};
    this.style = {};
    this.isConnected = true;
    this.listeners = new Map();
  }
  setAttribute(key, value) { this.attrs[key] = value; }
  addEventListener(key, callback) { this.listeners.set(key, callback); }
  appendChild(child) { this.children.push(child); return child; }
  querySelector(selector) { return selector === "button" ? this.children.find(child => child.tag === "button") : null; }
  querySelectorAll(selector) { return selector === '[role="menuitem"]' ? this.children.filter(child => child.attrs.role === "menuitem") : []; }
  getBoundingClientRect() { return { left: 25, bottom: 66, width: 230, height: 110 }; }
  contains(node) { return this === node || this.children.includes(node); }
  remove() { this.isConnected = false; }
  focus() { activeElement = this; }
}
const body = new FakeNode("body");
const documentMock = {
  body,
  createElement: tag => new FakeNode(tag),
  addEventListener: (key, callback) => handlers.set(key, callback),
  get activeElement() { return activeElement; }
};
const windowMock = {
  innerWidth: 720,
  innerHeight: 520,
  addEventListener: (key, callback) => handlers.set("window:" + key, callback)
};
const t = key => key;
const ctx = { document: documentMock, window: windowMock, t,
  state: {}, render() {}, invoke: (command, arg) => { reveals.push({ command, arg }); return Promise.resolve(); } };
runInNewContext(app.slice(start, end) + "\nthis.menuApi = { openAnalysisExplorerMenu, closeAnalysisExplorerMenu };", ctx);
const trigger = new FakeNode("row");
const options = [
  { name: "A.package", path: "C:\\Mods\\A.package" },
  { name: "B.package", path: "C:\\Mods\\B.package" }
];
const rightClick = (x, y) => ({
  clientX: x, clientY: y, currentTarget: trigger,
  preventDefault() { this.defaultPrevented = true; },
  stopPropagation() { this.stopped = true; }
});
const normalEvent = rightClick(420, 280);
ctx.menuApi.openAnalysisExplorerMenu(normalEvent, options);
assert(normalEvent.defaultPrevented && normalEvent.stopped);
let shown = body.children.at(-1);
assert.equal(shown.children.filter(n => n.tag === "button").length, 2);
assert.equal(shown.style.left, "420px");
assert.equal(shown.style.top, "280px");
assert.equal(activeElement, shown.children[0], "First menu action must receive focus");
handlers.get("keydown")({ key: "ArrowDown", preventDefault() {} });
assert.equal(activeElement, shown.children[1], "ArrowDown should focus next package");
shown.children[1].listeners.get("click")();
assert.deepEqual(JSON.parse(JSON.stringify(reveals)),
  [{ command: "reveal_path", arg: { path: "C:\\Mods\\B.package" } }],
  "The selected CC must be revealed, never a different member");
const keyboardEvent = rightClick(0, 0);
ctx.menuApi.openAnalysisExplorerMenu(keyboardEvent, options);
shown = body.children.at(-1);
assert.equal(shown.style.left, "37px", "Keyboard context menus anchor to the row");
assert.equal(shown.style.top, "66px");
handlers.get("keydown")({ key: "Escape", preventDefault() {} });
assert.equal(activeElement, trigger, "Escape must restore focus to the source row");
console.log("Explorer menu interaction: pointer, keyboard, selected path, Escape: PASS");

console.log("Beta 3 Elif Health streaming and Explorer context menu checks passed");
