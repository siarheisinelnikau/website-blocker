import { validatePolicy, activeGroups, compileRules, nextBoundary } from "./engine.js";
import { defaultState, validateState, policyFromState, changeState, locked, nowSeconds } from "./state.js";

const api = globalThis.chrome, HOST = "com.websiteblocker.companion";
const BROWSER = globalThis.navigator?.userAgent?.includes("Edg/") ? "edge" : "chrome";
// Native sync, settings writes and attempt counters share one ordered transaction queue.
let queue = Promise.resolve();
const serial = task => { const result = queue.then(task); queue = result.catch(() => {}); return result; };
const native = async body => {
  const r = await api.runtime.sendNativeMessage(HOST, { protocol: 1, ...body });
  if (r?.protocol !== 1 || r.error) throw new Error(r?.error || "Invalid companion response.");
  return r;
};

async function apply(state, policy, meta, connection = {}) {
  const now = nowSeconds();
  validatePolicy(policy);
  const rules = compileRules(policy, now, true);
  const existing = await api.declarativeNetRequest.getDynamicRules();
  if (JSON.stringify(existing) !== JSON.stringify(rules)) {
    for (const regex of new Set(rules.map(r => r.condition.regexFilter))) {
      const result = await api.declarativeNetRequest.isRegexSupported({ regex, isCaseSensitive: true });
      if (!result.isSupported) throw new Error(`Browser rejected a website matcher: ${result.reason}`);
    }
    await api.declarativeNetRequest.updateDynamicRules({ removeRuleIds: existing.map(r => r.id), addRules: rules });
  }
  const active = activeGroups(policy, now).map(g => g.name);
  const status = { active, error: "", nativeError: "", appliedAt: now, revision: policy.revision,
    expiresAt: policy.expires_at, ...connection, mode: meta?.baseRevision != null ? "desktop" : "browser", dirty: !!meta?.dirty, conflict: !!meta?.conflict };
  await api.storage.local.set({ ...(state ? { localState: state, syncMeta: meta } : {}), policy, status });
  await api.action.setBadgeText({ text: active.length ? String(active.length) : "" });
  await api.action.setBadgeBackgroundColor({ color: "#365cce" });
  await api.alarms.create("boundary", { when: nextBoundary(policy, now) * 1000 });
  return status;
}

async function update(refresh, resolution) {
  const cached = await api.storage.local.get(["localState", "syncMeta", "policy", "status", "statistics", "clientId"]);
  let state = cached.localState, meta = cached.syncMeta || { baseRevision: null, dirty: false };
  let connection = { contactedAt: cached.status?.contactedAt || 0, nativeError: cached.status?.nativeError || "" };
  if (state) validateState(state);
  if (resolution && meta.conflict) await api.storage.local.set({ syncBackup: { browser: state, desktop: meta.conflict, at: nowSeconds() } });
  // Keep pre-0.5 cached enforcement until the old native host can be migrated.
  let legacy = !state && cached.policy;
  if (!state && !legacy) state = defaultState();
  if (refresh) {
    try {
      const clientId = cached.clientId || crypto.randomUUID();
      if (!cached.clientId) await api.storage.local.set({clientId});
      const activity = cached.statistics ? { total: cached.statistics.total, sites: cached.statistics.sites, recent: cached.statistics.recent } : null;
      const remote = await native({ command: "sync", expected_revision: resolution === "browser" ? meta.conflict?.revision ?? null : meta.baseRevision,
        state: (meta.dirty || resolution === "browser") && resolution !== "desktop" ? state : null, client_id: clientId, activity });
      validateState(remote.state);
      if (remote.conflict && resolution !== "desktop") meta.conflict = remote.state;
      else if (state && locked(state) && JSON.stringify(state.focus) !== JSON.stringify(remote.state.focus)) meta.conflict = remote.state;
      else {
        state = remote.state;
        meta = { baseRevision: state.revision, dirty: false, conflict: null };
        legacy = false;
      }
      connection = { contactedAt: nowSeconds(), nativeError: "" };
    } catch (e) {
      connection.nativeError = `Desktop connection unavailable: ${e.message}`;
      if (legacy) {
        try { const r = await native({ command: "snapshot" }); legacy = validatePolicy(r.policy); connection = { contactedAt: nowSeconds(), nativeError: "" }; }
        catch { /* Keep the last usable policy. */ }
      }
    }
  }
  const policy = legacy ? validatePolicy(legacy) : policyFromState(state);
  const status = await apply(legacy ? null : state, policy, meta, connection);
  if (legacy && (nowSeconds() >= policy.expires_at || nowSeconds() < policy.generated_at - 60)) {
    status.error = "Cached policy expired; connect the desktop to migrate your settings.";
    await api.storage.local.set({ status });
  }
  if (refresh && !connection.nativeError && !meta.conflict) {
    try { await native({ command: "applied", browser: BROWSER, revision: policy.revision,
      extension_id: api.runtime.id, version: api.runtime.getManifest().version, error: null }); }
    catch { /* Retry the connection receipt next time; local enforcement is already applied. */ }
  }
  return status;
}

export function sync(refresh = true) {
  return serial(async () => {
    try { return await update(refresh); }
    catch (e) {
      const { status = {} } = await api.storage.local.get("status");
      await api.storage.local.set({ status: { ...status, error: `Rules could not be applied; previous browser rules may remain: ${e.message}` } });
      await api.action.setBadgeText({ text: "!" });
      await api.alarms.create("boundary", { delayInMinutes: 0.5 });
    }
  });
}

async function change(change) {
  const { localState, syncMeta = {} } = await api.storage.local.get(["localState", "syncMeta"]);
  if (!localState) throw new Error("Connect the updated desktop once to migrate your existing settings.");
  const state = changeState(validateState(localState), change);
  await apply(state, policyFromState(state), { ...syncMeta, dirty: true });
  await update(true);
  return { ok: true };
}

async function recordAttempt(message, sender) {
  // Record the configured domain, never the requested URL/path/query.
  const url = new URL(sender.url || "about:blank");
  if (!sender.url?.startsWith(api.runtime.getURL("blocked.html") + "?") || url.pathname !== "/blocked.html" ||
      !sender.tab || !/^[a-zA-Z0-9-]{16,80}$/.test(message.visit || "")) return { ok: false };
  const { localState, policy, statistics = { total: 0, sites: {}, days: {}, recent: [], visits: [] } } = await api.storage.local.get(["localState", "policy", "statistics"]);
  const group = activeGroups(policy, nowSeconds()).find(g => g.id === Number(url.searchParams.get("group")));
  const index = Number(url.searchParams.get("site"));
  if (!url.searchParams.has("site") || !Number.isInteger(index) || index < 0 || !group?.regexes[index] || statistics.visits.includes(message.visit)) return { ok: false };
  const configured = localState?.groups.find(g => g.id === group.id)?.patterns[index];
  if (!configured) return { ok: false };
  const site = configured.domain;
  const day = new Date().toLocaleDateString("en-CA");
  const stats = statistics;
  stats.total++; stats.sites[site] = (stats.sites[site] || 0) + 1; stats.days[day] = (stats.days[day] || 0) + 1;
  stats.recent.unshift({ site, at: nowSeconds() }); stats.recent = stats.recent.slice(0, 30);
  stats.visits = [...stats.visits.slice(-199), message.visit];
  stats.days = Object.fromEntries(Object.entries(stats.days).sort().slice(-31));
  stats.sites = Object.fromEntries(Object.entries(stats.sites).sort((a, b) => b[1] - a[1]).slice(0, 100));
  await api.storage.local.set({ statistics: stats });
  return { ok: true };
}

async function initialize() {
  await api.alarms.create("sync", { periodInMinutes: 0.5 });
  await sync();
}
api.runtime.onInstalled.addListener(async details => {
  await initialize();
  if (details.reason === "install") await api.tabs.create({ url: api.runtime.getURL("dashboard.html") });
});
api.runtime.onStartup.addListener(initialize);
api.alarms.onAlarm.addListener(alarm => { void sync(alarm.name === "sync"); });
api.runtime.onMessage.addListener((message, sender, respond) => {
  if (sender.id !== api.runtime.id) return false;
  let result;
  if (message.command === "sync") result = sync();
  if (message.command === "status") result = api.storage.local.get("status").then(r => r.status || {});
  if (message.command === "change") result = serial(() => change(message.change));
  if (message.command === "resolve") result = serial(() => {
    if (!["browser", "desktop"].includes(message.source)) throw new Error("Choose the settings to keep.");
    return update(true, message.source);
  });
  if (message.command === "attempt") result = serial(() => recordAttempt(message, sender));
  if (message.command === "clear-statistics") result = serial(() => api.storage.local.set({ statistics: { total: 0, sites: {}, days: {}, recent: [], visits: [] } }));
  if (!result) return false;
  result.then(value => respond(value ?? { ok: true }), error => respond({ error: error.message })); return true;
});
void sync(false);
