import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { activeGroups, compileRules, nextBoundary, scheduled, validatePolicy, wallTime } from "../engine.js";

const at = text => Date.parse(text) / 1000;
const regex = "^https?://([a-z0-9-]+\\.)*example\\.com(:[0-9]+)?/work";
function policy() {
  const now = at("2026-10-31T09:00:17Z");
  return { protocol: 1, revision: 1, generated_at: now, expires_at: now + 8 * 86400,
    allow_regexes: ["^https?://help\\.example\\.com(:[0-9]+)?/"],
    groups: [{ id: 1, name: "Work", regexes: [regex], schedules: [] }],
    focus: { group_id: 1, end: now + 60, locked: false }, pause_until: 0 };
}
const schedule = (days, start_minute, end_minute, timezone) => ({ days, start_minute, end_minute, timezone });

test("IANA conversion, midnight, overnight and week rollover", () => {
  assert.deepEqual(wallTime("Europe/Minsk", at("2026-11-01T21:00:00Z")), { day: 0, minute: 0 });
  const s = schedule([6], 1380, 60, "Europe/Minsk");
  assert.equal(scheduled(s, at("2026-11-01T20:00:00Z")), true);
  assert.equal(scheduled(s, at("2026-11-01T21:59:59Z")), true);
  assert.equal(scheduled(s, at("2026-11-01T22:00:00Z")), false);
  assert.equal(scheduled(s, at("2026-11-02T20:00:00Z")), false);
});

test("fall-back repeats both minutes and spring-forward skips missing time", () => {
  const fall = schedule([6], 90, 150, "America/New_York");
  assert.equal(scheduled(fall, at("2026-11-01T05:45:00Z")), true);
  assert.equal(scheduled(fall, at("2026-11-01T06:45:00Z")), true);
  assert.equal(scheduled(fall, at("2026-11-01T07:30:00Z")), false);
  const spring = schedule([6], 120, 180, "America/New_York");
  assert.equal(scheduled(spring, at("2026-03-08T06:59:00Z")), false);
  assert.equal(scheduled(spring, at("2026-03-08T07:00:00Z")), false);
});

test("focus, break, expiry and clock rollback produce finite rules", () => {
  const p = validatePolicy(policy()), now = p.generated_at;
  assert.equal(activeGroups(p, now).length, 1);
  assert.equal(activeGroups(p, now + 59).length, 1);
  assert.equal(activeGroups(p, now + 60).length, 0);
  p.pause_until = now + 20;
  assert.equal(activeGroups(p, now).length, 0);
  assert.equal(nextBoundary(p, now), now + 20);
  assert.equal(activeGroups(p, now + 20).length, 1);
  assert.equal(activeGroups(p, p.expires_at).length, 0);
  assert.equal(activeGroups(p, now - 61).length, 0);
  assert.deepEqual(compileRules(undefined, now), []);
});

test("DNR rules keep global allow priority and host/path boundaries", () => {
  const p = policy(), rules = compileRules(p, p.generated_at);
  assert.equal(rules[0].action.type, "allow");
  assert.ok(rules[0].priority > rules[1].priority);
  assert.equal(rules[1].action.redirect.extensionPath, "/blocked.html");
  assert.deepEqual(rules[1].condition.resourceTypes, ["main_frame"]);
  const matcher = new RegExp(rules[1].condition.regexFilter);
  for (const url of ["https://example.com/work", "https://a.example.com:8080/work/item"]) assert.ok(matcher.test(url));
  for (const url of ["https://evil-example.com/work", "https://example.com.evil.org/work", "https://example.com/Work", "https://example.com/other"]) assert.equal(matcher.test(url), false);
});

test("invalid timezone and excessive or malformed policy are rejected", () => {
  const p = policy();
  p.groups[0].schedules = [schedule([0], 60, 120, "Imaginary/Zone")];
  assert.throws(() => validatePolicy(p));
  p.groups[0].schedules[0].timezone = "UTC";
  assert.doesNotThrow(() => validatePolicy(p));
  p.groups.push(p.groups[0]); assert.throws(() => validatePolicy(p));
});

test("Rust-generated matchers and browser matching share normalization fixtures", () => {
  const fixtures = JSON.parse(readFileSync(new URL("./fixtures/matchers.json", import.meta.url)));
  for (const fixture of fixtures) {
    const matcher = new RegExp(fixture.regex);
    for (const url of fixture.allow) assert.ok(matcher.test(new URL(url).href), url);
    for (const url of fixture.deny) assert.equal(matcher.test(new URL(url).href), false, url);
  }
});
