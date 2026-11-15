import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { defaultState, validateState, parsePattern, parsePatterns, patternRegex, changeState, policyFromState } from "../state.js";
import { activeGroups } from "../engine.js";

test("browser matchers agree with Rust fixtures, including IDN and exact hosts", () => {
  const fixtures = JSON.parse(readFileSync(new URL("fixtures/matchers.json", import.meta.url)));
  for (const f of fixtures) {
    assert.equal(patternRegex(parsePattern(f.pattern)), f.regex);
    for (const url of f.allow) assert.ok(new RegExp(patternRegex(parsePattern(f.pattern))).test(url));
    for (const url of f.deny) assert.ok(!new RegExp(patternRegex(parsePattern(f.pattern))).test(url));
  }
  assert.equal(parsePattern("пример.рф").domain, "xn--e1afmkfd.xn--p1ai");
  for (const input of ["localhost", "127.0.0.1", "https://example.com", "example.com?q=1", "-bad.example", "example.com#x"]) assert.throws(() => parsePattern(input), input);
});
test("local focus, breaks and locks enforce the same deliberate actions without an EXE", () => {
  const now = 1794762000;
  const s = defaultState();
  const focus = changeState(s, { type: "start", group_id: 1, minutes: 25, locked: true }, now);
  assert.equal(focus.focus.end, now + 1500);
  for (const action of [{ type: "stop", phrase: "END FOCUS" }, { type: "break", phrase: "TAKE A BREAK" }, { type: "rules", revision: focus.revision, groups: [] }]) assert.throws(() => changeState(focus, action, now));
  const fresh = changeState(focus, { type: "start", group_id: 1, minutes: 1, locked: false }, now + 1500);
  assert.throws(() => changeState(fresh, { type: "break", phrase: "" }, now + 1501));
  const paused = changeState(fresh, { type: "break", phrase: "TAKE A BREAK" }, now + 1501);
  assert.equal(paused.pause_until, now + 1801);
  assert.throws(() => changeState(paused, { type: "break", phrase: "TAKE A BREAK" }, now + 1810));
  assert.equal(changeState(paused, { type: "resume" }, now + 1502).pause_until, 0);
});
test("standalone schedules renew beyond the native cache horizon, with stale-write protection", () => {
  const s = defaultState(); s.groups[0].schedules = [{ days:[0,1,2,3,4,5,6], start_minute:0, end_minute:1439, timezone:"UTC" }];
  const now = Date.UTC(2026,10,15,12)/1000;
  assert.equal(activeGroups(policyFromState(s, now+10*86400), now+10*86400).length, 1);
  assert.throws(() => changeState(s,{type:"rules",revision:99,groups:s.groups,allowlist:[]},now), /another window/);
  s.groups[0].enabled = false; s.groups[0].schedules[0].days = [];
  assert.throws(() => validateState(s), /schedule/);
});
