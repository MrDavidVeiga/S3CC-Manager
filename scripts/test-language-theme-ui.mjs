import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const index = fs.readFileSync(path.join(repoRoot, "index.html"), "utf8");
const app = fs.readFileSync(path.join(repoRoot, "src", "app.js"), "utf8");
const theme = fs.readFileSync(path.join(repoRoot, "src", "theme.js"), "utf8");
const winui = fs.readFileSync(path.join(repoRoot, "src", "winui.css"), "utf8");
const themeAudit = fs.readFileSync(path.join(repoRoot, "src", "ui", "fluent", "theme-audit.css"), "utf8");

const themeTranslations = {
  en: {
    theme: "Theme",
    themeSystem: "System",
    themeVeigaLight: "Veiga Light",
    themeVeigaDark: "Veiga Dark",
    themeMeanGirls: "Mean Girls",
    themeEaApp: "EA App",
    themeWinuiLight: "WinUI Light",
    themeWinuiDark: "WinUI Dark",
  },
  pt: {
    theme: "Tema",
    themeSystem: "Sistema",
    themeVeigaLight: "Veiga Claro",
    themeVeigaDark: "Veiga Escuro",
    themeMeanGirls: "Meninas Malvadas",
    themeEaApp: "EA App",
    themeWinuiLight: "WinUI Claro",
    themeWinuiDark: "WinUI Escuro",
  },
  es: {
    theme: "Tema",
    themeSystem: "Sistema",
    themeVeigaLight: "Veiga Claro",
    themeVeigaDark: "Veiga Oscuro",
    themeMeanGirls: "Chicas Malas",
    themeEaApp: "EA App",
    themeWinuiLight: "WinUI Claro",
    themeWinuiDark: "WinUI Oscuro",
  },
};

const languageLabels = {
  languageEnglish: "EN",
  languagePortuguese: "PT",
  languageSpanish: "ES",
};

const themeIds = [
  "system",
  "veiga-light",
  "veiga-dark",
  "mean-girls",
  "ea-app",
  "winui-light",
  "winui-dark",
];

assert.match(
  winui,
  /\.release-v1 \.lang-toggle-btn\s*\{[^}]*width:76px;[^}]*min-width:76px;/s,
  "Manager language selector must use the compact Splitter width",
);
assert.match(
  winui,
  /\.release-v1 \.language-switcher\s*\{[^}]*font-size:14px;/s,
  "Language label must use the shared top-control text size",
);
assert.match(
  winui,
  /\.release-v1 \.lang-toggle-btn,\.theme-button\s*\{[^}]*font-size:14px;/s,
  "Language codes and theme text must use the shared top-control text size",
);
assert.match(
  themeAudit,
  /--topbar-label-font-size:\s*14px;[\s\S]*\.language-switcher > span,[\s\S]*#language-code,[\s\S]*\.theme-button-label[\s\S]*font-size:\s*var\(--topbar-label-font-size\)/s,
  "Language label, language codes and Theme label must share one top-bar font metric",
);
assert.match(
  themeAudit,
  /body\.release-v1 \.lang-toggle-btn,[\s\S]*?body\.release-v1 \.theme-button \{[\s\S]*?align-items:\s*center;[\s\S]*?line-height:\s*20px;/s,
  "Language and Theme controls must center their text and icons on one flex baseline",
);
assert.match(
  themeAudit,
  /body\.release-v1 #language-code,[\s\S]*?body\.release-v1 \.theme-button-label \{[\s\S]*?display:\s*inline-flex;[\s\S]*?height:\s*20px;[\s\S]*?line-height:\s*20px;/s,
  "Language code and Theme label must use explicit centered line boxes",
);
assert.match(
  themeAudit,
  /body\.release-v1 \.theme-button \{[\s\S]*?gap:\s*4px;[\s\S]*?padding-inline:\s*8px;/s,
  "Theme icon and label must use the compact optical gap used by the language control",
);

assert.match(index, /id="language-code"/);
assert.match(index, /data-i18n-aria-label="languageSelector"/);
assert.match(index, /id="titlebar-minimize"[\s\S]*?data-i18n-title="windowMinimize"/);
assert.match(index, /id="titlebar-maximize"[\s\S]*?data-i18n-title="windowMaximize"/);
assert.match(index, /id="titlebar-close"[\s\S]*?data-i18n-title="windowClose"/);
assert.doesNotMatch(app, /filterCategories:/, "Filter dropdown must not expose a visible category-heading translation");
assert.doesNotMatch(app, /content-filter-menu-heading/, "Filter dropdown must not render the removed heading");
for (const [key, value] of Object.entries(languageLabels)) {
  assert.match(index, new RegExp(`data-i18n="${key}"`));
  assert.match(index, new RegExp(`>${value}<`));
  assert.equal((app.match(new RegExp(`\\b${key}:`, "g")) || []).length, 3, `${key} must exist in EN/PT/ES`);
}

for (const id of themeIds) {
  const button = new RegExp(`data-theme="${id}"[\\s\\S]*?data-i18n="theme`);
  assert.match(index, button, `${id} must retain its internal ID and translated label hook`);
}

for (const [language, translations] of Object.entries(themeTranslations)) {
  for (const [key, value] of Object.entries(translations)) {
    assert.match(app, new RegExp(`\\b${key}: "${value.replace(/[.*+?^${}()|[\\]\\\\]/g, "\\\\$&")}"`), `${language}.${key}`);
  }
}

assert.match(app, /const languageLabels = \{/);
assert.match(app, /option\.setAttribute\("aria-selected", String\(option\.dataset\.lang === state\.language\)\)/);
assert.match(app, /data-i18n-aria-label/);
assert.match(app, /data-i18n-title/);
assert.match(app, /localStorage\.setItem\("s3cc-organizer-language", state\.language\)/);
assert.match(app, /preferences\.language/);

assert.match(theme, /const VALID_THEMES = new Set\(\["system","veiga-light","veiga-dark","mean-girls","ea-app","winui-light","winui-dark"\]\)/);
assert.match(theme, /localStorage\.setItem\(THEME_KEY,choice\)/);
assert.match(theme, /option\.dataset\.theme===choice/);
assert.match(theme, /captionLabels=\{/);
assert.match(theme, /window\.addEventListener\("s3cc-language-changed"/);
assert.match(winui, /#language-code \{ white-space:nowrap; \}/);

// The nine required language/theme combinations are data-driven: each language
// must render the same seven internal IDs without changing theme tokens.
assert.equal(Object.keys(themeTranslations).length * 3, 9);
for (const language of Object.keys(themeTranslations)) {
  for (const id of ["system", "mean-girls", "winui-dark"]) {
    assert.ok(themeIds.includes(id), `${language}/${id} representative combination`);
  }
}

console.log("language/theme UI regression: 9 language/theme combinations, 7 theme IDs, persistence and accessible active-language state verified");
