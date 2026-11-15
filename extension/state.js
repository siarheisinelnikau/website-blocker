// Browser-owned settings use the desktop schema. No native host is needed to enforce them.
import { validatePolicy, wallTime } from "./engine.js";

export const nowSeconds = () => Math.floor(Date.now() / 1000);
const bytes = value => new TextEncoder().encode(value).length;
const escapeRegex = value => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
export function parsePattern(input) {
  input = input.trim();
  const subdomains = !input.startsWith("=");
  input = input.replace(/^(=|\*\.)/, "");
  if (!input || bytes(input) > 512 || /[?#@:\\]/.test(input)) throw new Error("Use a domain and optional /path prefix, without scheme, port, query or fragment.");
  const url = new URL(`https://${input}`), domain = url.hostname.toLowerCase();
  if (domain.length > 253 || !domain.includes(".") || domain.split(".").some(s => !/^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(s)) || /^\d+(\.\d+){3}$/.test(domain)) throw new Error("Use a public-style domain name, for example example.com.");
  return { domain, subdomains, path_prefix: url.pathname };
}
export const patternText = p => `${p.subdomains ? "" : "="}${p.domain}${p.path_prefix === "/" ? "" : p.path_prefix}`;
export function parsePatterns(text) {
  const result = text.split(/\r?\n/).filter(s => s.trim()).map(parsePattern);
  if (result.length > 50) throw new Error("At most 50 websites are allowed per list.");
  return result;
}
export const patternRegex = p => `^https?://${p.subdomains ? "([a-z0-9_-]+\\.)*" : ""}${escapeRegex(p.domain)}\\.?(:[0-9]+)?${escapeRegex(p.path_prefix)}`;
export function defaultState() {
  return { schema: 1, revision: 0, groups: [{ id: 1, name: "Distractions", enabled: true,
    patterns: parsePatterns("youtube.com\nreddit.com"), schedules: [] }], allowlist: [], focus: null,
    pause_until: 0, next_break: 0, startup_updates: true, language: "en", theme: "light" };
}
export function validateState(s) {
  if (!s || s.schema !== 1 || !Number.isSafeInteger(s.revision) || s.revision < 0 || !Array.isArray(s.groups) || s.groups.length > 20 ||
      !Array.isArray(s.allowlist) || s.allowlist.length > 50 || !Number.isSafeInteger(s.pause_until) || s.pause_until < 0 ||
      !Number.isSafeInteger(s.next_break) || s.next_break < 0) throw new Error("Invalid browser settings.");
  const ids = new Set(); let total = s.allowlist.length;
  for (const g of s.groups) {
    if (!Number.isSafeInteger(g.id) || g.id < 1 || g.id > 4294967295 || ids.has(g.id) || typeof g.name !== "string" || !g.name.trim() || bytes(g.name) > 80 || typeof g.enabled !== "boolean" ||
        !Array.isArray(g.patterns) || !g.patterns.length || g.patterns.length > 50 || !Array.isArray(g.schedules) || g.schedules.length > 14) throw new Error("Each rule set needs a unique ID, a name (1–80 bytes), 1–50 websites, and at most 14 schedules.");
    ids.add(g.id); total += g.patterns.length;
    for (const s of g.schedules) {
      if (!Array.isArray(s.days) || !s.days.length || s.days.some(d => !Number.isInteger(d) || d < 0 || d > 6) || !Number.isInteger(s.start_minute) || !Number.isInteger(s.end_minute) || s.start_minute < 0 || s.end_minute < 0 || s.start_minute > 1439 || s.end_minute > 1439 || s.start_minute === s.end_minute || typeof s.timezone !== "string" || s.timezone.length > 80) throw new Error("Invalid schedule.");
      wallTime(s.timezone, nowSeconds());
    }
  }
  if (total > 1000) throw new Error("At most 1,000 website patterns are supported.");
  for (const p of [...s.allowlist, ...s.groups.flatMap(g => g.patterns)]) {
    if (typeof p.domain !== "string" || typeof p.path_prefix !== "string" || typeof p.subdomains !== "boolean" || JSON.stringify(parsePattern(patternText(p))) !== JSON.stringify({ domain: p.domain, subdomains: p.subdomains, path_prefix: p.path_prefix })) throw new Error("Invalid saved website pattern.");
  }
  if (s.focus && (!s.groups.some(g => g.id === s.focus.group_id && g.enabled) || !Number.isSafeInteger(s.focus.end) || typeof s.focus.locked !== "boolean")) throw new Error("Invalid focus timer.");
  if (s.language !== undefined && !["en", "ru", "fr", "de", "es", "it"].includes(s.language)) throw new Error("Invalid language.");
  if (s.theme !== undefined && !["light", "dark"].includes(s.theme)) throw new Error("Invalid theme.");
  policyFromState(s); return s;
}
export function policyFromState(s, now = nowSeconds()) {
  return validatePolicy({ protocol: 1, revision: s.revision, generated_at: now, expires_at: now + 8 * 86400,
    groups: s.groups.filter(g => g.enabled).map(g => ({ id: g.id, name: g.name, regexes: g.patterns.map(patternRegex), schedules: g.schedules })),
    allow_regexes: s.allowlist.map(patternRegex), focus: s.focus, pause_until: s.pause_until, language: s.language || "en", theme: s.theme || "light" });
}
export const locked = (s, now = nowSeconds()) => !!(s.focus?.locked && s.focus.end > now);
export function changeState(current, change, now = nowSeconds()) {
  const s = structuredClone(current);
  if (s.focus?.end <= now) s.focus = null;
  switch (change.type) {
    case "rules":
      if (locked(s, now)) throw new Error("Rules are protected until the locked focus session ends.");
      if (change.revision !== s.revision) throw new Error("Settings changed in another window. Reload before saving.");
      s.groups = change.groups; s.allowlist = change.allowlist; break;
    case "start":
      if (s.focus?.end > now) throw new Error("A focus session is already running.");
      if (!Number.isInteger(change.minutes) || change.minutes < 1 || change.minutes > 240 || !s.groups.some(g => g.id === change.group_id && g.enabled)) throw new Error("Choose an enabled rule set and a duration from 1 to 240 minutes.");
      s.focus = { group_id: change.group_id, end: now + change.minutes * 60, locked: !!change.locked }; s.pause_until = 0; break;
    case "stop":
      if (locked(s, now)) throw new Error("This session is locked until its timer ends.");
      if (change.phrase !== "END FOCUS") throw new Error("Type END FOCUS to end the session deliberately.");
      s.focus = null; break;
    case "break":
      if (locked(s, now)) throw new Error("Breaks are unavailable during a locked session.");
      if (now < s.next_break) throw new Error("A break is available only once every 30 minutes.");
      if (change.phrase !== "TAKE A BREAK") throw new Error("Type TAKE A BREAK to pause all rules for five minutes.");
      s.pause_until = now + 300; s.next_break = now + 1800; break;
    case "resume": s.pause_until = 0; break;
    case "appearance": s.language = change.language; s.theme = change.theme; break;
    default: throw new Error("Unknown settings action.");
  }
  s.revision++; return validateState(s);
}
