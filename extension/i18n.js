import { catalogs } from "./catalog.js";

export const languages = ["en", "ru", "fr", "de", "es", "it"];
let language = "en";

export function translate(key, selected = language) {
  return catalogs[key]?.[Math.max(0, languages.indexOf(selected))] ?? key;
}
export function t(key, ...values) {
  let index = 0;
  return translate(key).replace(/\{\}/g, () => String(values[index++] ?? "{}"));
}
export function currentLanguage() { return language; }

/** Translate known error templates while preserving browser/OS diagnostic details. */
export function statusMessage(value) {
  if (!value) return "";
  if (catalogs[value]) return t(value);
  for (const key of Object.keys(catalogs).filter(key => key.endsWith("{}"))) {
    const prefix = key.slice(0, -2);
    if (prefix && value.startsWith(prefix)) return t(key, value.slice(prefix.length));
  }
  return value;
}

export function applyPreferences(policy = {}) {
  language = languages.includes(policy.language) ? policy.language : "en";
  document.documentElement.lang = language;
  document.documentElement.dataset.theme = policy.theme === "dark" ? "dark" : "light";
  document.querySelectorAll("[data-i18n]").forEach(element => {
    element.textContent = t(element.dataset.i18n);
  });
  document.querySelectorAll("[data-i18n-label]").forEach(element => {
    element.setAttribute("aria-label", t(element.dataset.i18nLabel));
  });
  if (document.title && document.querySelector("title[data-i18n]")) {
    document.title = t(document.querySelector("title[data-i18n]").dataset.i18n);
  }
}

export async function initializeI18n(onChange = () => {}) {
  try { applyPreferences((await chrome.storage.local.get("policy")).policy); }
  catch { applyPreferences(); }
  chrome.storage.onChanged?.addListener((changes, area) => {
    if (area === "local" && changes.policy) {
      applyPreferences(changes.policy.newValue);
      onChange();
    }
  });
}
