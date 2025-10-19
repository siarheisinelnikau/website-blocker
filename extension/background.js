import { validatePolicy, activeGroups, compileRules, nextBoundary } from "./engine.js";

const api = globalThis.chrome;
const HOST = "com.websiteblocker.companion";
let inflight = null;
let refreshPending = false;

async function update(refresh) {
  const cached = await api.storage.local.get(["policy", "status"]);
  let policy = cached.policy;
  let error = cached.status?.nativeError ?? "";
  let contactedAt = cached.status?.contactedAt ?? 0;
  const now = Math.floor(Date.now() / 1000);
  if (refresh) {
    try {
      const response = await api.runtime.sendNativeMessage(HOST, { protocol: 1, command: "snapshot" });
      if (response?.protocol !== 1 || response.error) throw new Error(response?.error || "Invalid companion response.");
      const candidate = validatePolicy(response.policy);
      // Check RE2 support before replacing the last known usable policy.
      const regexes = [...candidate.allow_regexes, ...candidate.groups.flatMap(g => g.regexes)];
      const previousRegexes = policy ? [...policy.allow_regexes, ...policy.groups.flatMap(g => g.regexes)] : [];
      for (const regex of new Set(JSON.stringify(regexes) === JSON.stringify(previousRegexes) ? [] : regexes)) {
        const supported = await api.declarativeNetRequest.isRegexSupported({ regex, isCaseSensitive: true });
        if (!supported.isSupported) throw new Error(`Browser rejected a website matcher: ${supported.reason}`);
      }
      policy = candidate;
      contactedAt = now;
      error = "";
    } catch (e) { error = `Desktop connection unavailable: ${e.message}`; }
  }
  const nativeError = error;
  if (policy) validatePolicy(policy);
  if (!policy) {
    error = error || "No policy. Open WebsiteBlocker, check native messaging setup, and sync again.";
  } else if (now >= policy.expires_at) {
    error = `Cached policy expired; blocking cleared. ${error}`;
  } else if (now < policy.generated_at - 60) {
    error = `Clock moved before this policy; blocking cleared until a fresh sync. ${error}`;
  }
  const rules = compileRules(policy, now);
  const existing = await api.declarativeNetRequest.getDynamicRules();
  if (JSON.stringify(existing) !== JSON.stringify(rules)) {
    await api.declarativeNetRequest.updateDynamicRules({ removeRuleIds: existing.map(r => r.id), addRules: rules });
  }
  if (policy) await api.storage.local.set({ policy });
  const active = activeGroups(policy, now).map(g => g.name);
  await api.storage.local.set({ status: { active, error, nativeError, contactedAt, appliedAt: now,
    revision: policy?.revision ?? null, expiresAt: policy?.expires_at ?? 0 } });
  await api.action.setBadgeText({ text: active.length ? String(active.length) : "" });
  await api.action.setBadgeBackgroundColor({ color: "#215c52" });
  await api.alarms.create("boundary", { when: nextBoundary(policy, now) * 1000 });
}

export function sync(refresh = true) {
  refreshPending ||= refresh;
  if (inflight) return inflight;
  inflight = (async () => {
    try {
      do {
        const requested = refreshPending;
        refreshPending = false;
        try { await update(requested); }
        catch (e) {
          await api.storage.local.set({ status: { active: [], error: `Rules could not be applied; previous browser rules may remain: ${e.message}`, appliedAt: 0 } });
          await api.action.setBadgeText({ text: "!" });
          await api.alarms.create("boundary", { delayInMinutes: 0.5 });
        }
      } while (refreshPending);
    } finally { inflight = null; }
  })();
  return inflight;
}

async function initialize() {
  await api.alarms.create("sync", { periodInMinutes: 0.5 });
  await sync();
}
api.runtime.onInstalled.addListener(initialize);
api.runtime.onStartup.addListener(initialize);
api.alarms.onAlarm.addListener(alarm => { void sync(alarm.name === "sync"); });
api.runtime.onMessage.addListener((message, sender, respond) => {
  if (sender.id !== api.runtime.id) return false;
  if (message.command === "sync") { sync().then(() => respond({ ok: true })); return true; }
  if (message.command === "status") { api.storage.local.get("status").then(result => respond(result.status ?? {})); return true; }
  return false;
});
// Service workers can restart without onStartup; reconcile cached rules immediately.
void sync(false);
