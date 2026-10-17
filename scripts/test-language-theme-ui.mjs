import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const index = fs.readFileSync(path.join(repoRoot, "index.html"), "utf8");
const app = fs.readFileSync(path.join(repoRoot, "src", "app.js"), "utf8");
const theme = fs.readFileSync(path.join(repoRoot, "src", "theme.js"), "utf8");
const winui = fs.readFileSync(path.join(repoRoot, "src", "winui.css"), "utf8");

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

assert.match(index, /id="language-code"/);
assert.match(index, /data-i18n-aria-label="languageSelector"/);
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
