import test from "node:test";
import assert from "node:assert/strict";

test("service worker applies real DNR payloads, keeps a usable cache, and clears expired rules", async () => {
  const realNow = Date.now;
  let now = 1793437217;
  Date.now = () => now * 1000;
  const policy = { protocol: 1, revision: 1, generated_at: now, expires_at: now + 600,
    allow_regexes: [], groups: [{ id: 1, name: "Focus", regexes: ["^https?://example\\.com/"], schedules: [] }],
    focus: { group_id: 1, end: now + 120 }, pause_until: 0 };
  const storage = {}, alarms = [], writes = [];
  let dynamic = [], disconnected = false, rejectRules = false;
  const event = () => ({ addListener() {} });
  globalThis.chrome = {
    storage: { local: { async get(keys) { return typeof keys === "string" ? { [keys]: storage[keys] } : Object.fromEntries(keys.map(k => [k, storage[k]])); },
      async set(value) { Object.assign(storage, value); } } },
    runtime: { id: "fixture", onInstalled: event(), onStartup: event(), onMessage: event(), async sendNativeMessage(host, message) {
      assert.equal(host, "com.websiteblocker.companion"); assert.equal(message.command, "snapshot");
      if (disconnected) throw new Error("Host not found");
      return { protocol: 1, policy: structuredClone(policy) };
    } },
    declarativeNetRequest: { async isRegexSupported() { return { isSupported: true }; }, async getDynamicRules() { return dynamic; },
      async updateDynamicRules(value) {
        if (rejectRules) throw new Error("Fixture API rejection");
        writes.push(value); dynamic = value.addRules;
      } },
    action: { async setBadgeText() {}, async setBadgeBackgroundColor() {} },
    alarms: { onAlarm: event(), async create(name, options) { alarms.push({ name, options }); } }
  };
  try {
    const { sync } = await import("../background.js");
    await sync();
    assert.equal(dynamic.length, 1); assert.deepEqual(storage.status.active, ["Focus"]);
    policy.revision = 2;
    policy.groups[0].regexes = ["^https?://changed\\.example\\.com/"];
    rejectRules = true;
    await sync();
    assert.equal(storage.policy.revision, 1); // failed application must not poison the usable cache
    assert.match(storage.status.error, /previous browser rules may remain/);
    rejectRules = false;
    disconnected = true; now += 30; await sync();
    assert.equal(dynamic.length, 1); assert.match(storage.status.error, /Host not found/);
    now = policy.focus.end; await sync(false);
    assert.equal(dynamic.length, 0); // timer ends even when desktop is unreachable
    now = policy.expires_at; await sync(false);
    assert.equal(dynamic.length, 0); assert.ok(storage.status.error);
    assert.ok(writes.some(value => value.removeRuleIds.includes(1)));
    assert.ok(alarms.some(a => a.name === "boundary"));
  } finally { Date.now = realNow; delete globalThis.chrome; }
});
