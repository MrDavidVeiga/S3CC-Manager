/* Regression smoke checks for the first WinUI foundation migration.
 * No browser mocks: runtime interactions will be covered in follow-up batches.
 */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (p) => readFileSync(new URL(`../${p}`, import.meta.url), "utf8");
const index = read("index.html");
const tokens = read("src/ui/fluent/tokens.css");
const controls = read("src/ui/fluent/controls.css");
const buttonGeometry = read("src/ui/fluent/button-geometry.css");
const themeAudit = read("src/ui/fluent/theme-audit.css");
const listview = read("src/ui/fluent/listview.js");
const scrollbars = read("src/ui/fluent/scrollbars.css");
const feedback = read("src/ui/fluent/feedback.js");
const feedbackCss = read("src/ui/fluent/feedback.css");
const chrome = read("src/ui/fluent/chrome.js");
const tooltips = read("src/ui/fluent/tooltips.js");
const fields = read("src/ui/fluent/fields.js");
const theme = read("src/theme.js");
const comboJs = read("src/ui/fluent/combobox.js");
const comboCss = read("src/ui/fluent/combobox.css");
const overlays = read("src/ui/fluent/overlays.js");
const menus = read("src/ui/fluent/menus.js");
const overlaysCss = read("src/ui/fluent/overlays.css");

const imports = [
  "src/style.css", "src/winui.css",
  "src/ui/fluent/tokens.css", "src/ui/fluent/controls.css", "src/ui/fluent/combobox.css", "src/ui/fluent/overlays.css",
];
const positions = imports.map(p=>index.indexOf(`href="./${p}"`));
assert(positions.every(p => p >= 0), "all style sheets must be imported");
assert(positions.every((p,i) => i === 0 || p > positions[i-1]), "Fluent foundation must load after existing styles");
assert.match(index, /body class="[^"]*release-v1/);
assert.match(index, /src\/ui\/fluent\/scrollbars\.css/);
assert.match(index, /src\/ui\/fluent\/listview\.js/);
assert.match(listview, /PageDown/);
assert.match(listview, /MutationObserver/);
assert.match(scrollbars, /::-webkit-scrollbar-thumb/);
assert.match(index, /src\/ui\/fluent\/feedback\.css/);
assert.match(index, /src\/ui\/fluent\/feedback\.js/);
assert.match(feedback, /syncFluentInfoBar/);
assert.match(feedbackCss, /fluent-ring-spin/);
// Four operation-specific bars plus the global loading overlay.
assert.equal((index.match(/role="progressbar"/g) || []).length, 5);
assert.match(index, /id="organize-progress"/);
assert.match(index, /id="organize-progress-bar"/);
assert.match(index, /src\/ui\/fluent\/chrome\.js/);
assert.match(index, /src\/ui\/fluent\/fields\.js/);
assert.match(index, /src\/ui\/fluent\/tooltips\.js/);
assert.match(chrome, /ArrowRight/);
assert.match(tooltips, /aria-describedby/);
assert.match(fields, /dispatchEvent\(new Event\("input"/);
assert(index.includes("src=\"./src/ui/fluent/combobox.js\""));
assert.match(comboJs, /MutationObserver/);
assert.match(comboJs, /role","combobox"/);
assert.match(comboJs, /role","option"/);
assert.match(comboJs, /ITEM_HEIGHT/);
assert.match(comboJs, /dispatchEvent\(new Event\("change"/);
assert.match(comboJs, /closeFluentComboBox/);
assert.match(comboCss, /fluent-combobox-flyout/);
assert(index.includes("src=\"./src/ui/fluent/overlays.js\""));
assert(index.includes("src=\"./src/ui/fluent/menus.js\""));
assert.match(overlays, /focusFirst/);
assert.match(overlays, /focusables/);
assert.match(overlays, /\.inert=/);
assert.match(overlays, /modalIds=/);
assert.match(overlays, /closeFluentComboBox/);
assert.match(menus, /aria-selected/);
assert.match(menus, /ArrowDown/);
assert.match(overlaysCss, /\[role="dialog"\]/);
assert.match(index, /catalog-import-modal-title/);
assert.match(tokens, /--fluent-radius-control: var\(--winui-control-radius, 4px\)/);
assert.match(tokens, /--fluent-radius-overlay: var\(--winui-overlay-radius, 8px\)/);
assert.match(controls, /:focus-visible/);
assert.match(controls, /input\[type="checkbox"\]/);
assert.match(controls, /:indeterminate/);
assert.match(controls, /:disabled/);
assert.match(controls, /prefers-reduced-motion/);
assert.match(controls, /forced-colors/);
assert.match(controls, /input\.text-input/);
assert.match(controls, /\.primary-btn/);
assert.match(controls, /\.secondary-btn/);
assert.match(controls, /\.danger-btn/);
assert.match(index, /src\/ui\/fluent\/button-geometry\.css/);
assert.match(buttonGeometry, /min-height: var\(--fluent-control-height\)/);
assert.match(buttonGeometry, /padding: 5px 12px/);
assert.match(buttonGeometry, /\.sidebar > \.primary-btn/);
assert.match(index, /src\/ui\/fluent\/theme-audit\.css/);
assert.match(themeAudit, /--fluent-list-hover-fill/);
assert.match(themeAudit, /--fluent-list-selected-fill/);

for (const themeId of ["system","veiga-light","veiga-dark","mean-girls","ea-app","winui-light","winui-dark"]) {
  assert(index.includes(`data-theme="${themeId}"`), `missing ${themeId} theme choice`);
  assert(theme.includes(`"${themeId}"`), `missing ${themeId} theme logic`);
}
for (const id of ["choose-folder-btn","scan-btn","plan-btn","duplicate-details-modal","conflict-details-modal","confirm-modal","tools-readonly","tools-profile-select","duplicates-filter","conflicts-filter"]) {
  assert(index.includes(`id="${id}"`), `missing existing control ${id}`);
}
console.log("Fluent foundation static regression checks: PASS");
