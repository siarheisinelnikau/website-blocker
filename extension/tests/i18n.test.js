import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { catalogs } from "../catalog.js";
import { languages, translate, applyPreferences, t, statusMessage } from "../i18n.js";

test("every browser page and manifest has six complete catalogs with matching placeholders", () => {
  for (const [key, row] of Object.entries(catalogs)) {
    assert.equal(row.length, languages.length, key);
    for (const value of row) {
      assert.ok(value.trim(), key);
      assert.equal(value.split("{}").length, key.split("{}").length, key);
    }
  }
  for (const name of ["popup", "blocked", "welcome"]) {
    const html = readFileSync(new URL(`../${name}.html`, import.meta.url), "utf8");
    for (const match of html.matchAll(/data-i18n(?:-label)?="([^"]+)"/g)) {
      const key = match[1].replaceAll("&amp;", "&").replaceAll("&quot;", '"');
      assert.ok(catalogs[key], `${name}: ${key}`);
    }
  }
  for (const language of languages) {
    const locale = JSON.parse(readFileSync(new URL(`../_locales/${language}/messages.json`, import.meta.url), "utf8"));
    assert.equal(locale.extensionName.message, translate("WebsiteBlocker Companion", language));
    assert.ok(locale.extensionDescription.message);
  }
});

test("policy preferences change the page and preserve supplied rule names and error details", () => {
  const element = { dataset: { i18n: "Settings" }, textContent: "" };
  globalThis.document = {
    documentElement: { dataset: {} }, title: "",
    querySelectorAll(selector) { return selector === "[data-i18n]" ? [element] : []; },
  };
  try {
    applyPreferences({ language: "ru", theme: "dark" });
    assert.equal(document.documentElement.lang, "ru");
    assert.equal(document.documentElement.dataset.theme, "dark");
    assert.equal(element.textContent, "Настройки");
    assert.equal(t("Active: {}", "My personal list <data>"), "Активно: My personal list <data>");
    assert.equal(statusMessage("Desktop connection unavailable: Host not found"), "Подключение к приложению недоступно: Host not found");
    applyPreferences({ language: "unsupported", theme: "unsupported" });
    assert.equal(element.textContent, "Settings");
    assert.equal(document.documentElement.lang, "en");
    assert.equal(document.documentElement.dataset.theme, "light");
  } finally { delete globalThis.document; }
});
