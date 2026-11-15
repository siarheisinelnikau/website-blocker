import test from "node:test";
import assert from "node:assert/strict";
import { defaultState, policyFromState } from "../state.js";

async function fixture(initial = {}) {
  const storage = structuredClone(initial), alarms = [], calls = [];
  let dynamic = [], remote = null, failDnr = false, listener;
  const event = () => ({ addListener() {} });
  globalThis.chrome = {
    storage:{ local:{ async get(keys) { return Object.fromEntries((typeof keys === "string" ? [keys] : keys).map(k=>[k,structuredClone(storage[k])])); }, async set(value) { Object.assign(storage,structuredClone(value)); } } },
    runtime:{ id:"pepcehhhdngafeeljmaanapnofkkhbdf", getManifest:()=>({version:"0.5.0"}), getURL:path=>`chrome-extension://pepcehhhdngafeeljmaanapnofkkhbdf/${path}`,
      onInstalled:event(),onStartup:event(),onMessage:{addListener(f){listener=f;}},
      async sendNativeMessage(host,body) {
        calls.push(body); if (!remote) throw new Error("Native host not found");
        if (body.command === "applied") return {protocol:1};
        assert.equal(body.command,"sync");
        if (body.state && body.expected_revision !== remote.revision) return {protocol:1,state:structuredClone(remote),conflict:true};
        if (body.state) remote = {...structuredClone(body.state),revision:remote.revision+1};
        return {protocol:1,state:structuredClone(remote),conflict:false};
      } },
    declarativeNetRequest:{ async getDynamicRules(){return dynamic;}, async isRegexSupported(){return {isSupported:true};},async updateDynamicRules(r){if(failDnr)throw new Error("DNR rejection");dynamic=r.addRules;} },
    action:{async setBadgeText(){},async setBadgeBackgroundColor(){}},alarms:{onAlarm:event(),async create(name,options){alarms.push({name,options});}} };
  const worker = await import(`../background.js?test=${Math.random()}`); await worker.sync(false);
  const send = (message,sender={id:chrome.runtime.id}) => new Promise(resolve=>listener(message,sender,resolve));
  return {storage,alarms,calls,worker,send,get rules(){return dynamic;},set remote(s){remote=structuredClone(s);},get remote(){return remote;},set reject(s){failDnr=s;}};
}
test("no native host: editable local policy, focus and schedules survive worker restarts and eight days", async () => {
  const f = await fixture(); await f.worker.sync();
  assert.equal(f.storage.status.error, ""); assert.equal(f.storage.status.mode,"browser");
  assert.ok(f.storage.localState); assert.equal(f.rules.length,0);
  const result = await f.send({command:"change",change:{type:"start",group_id:1,minutes:25,locked:false}});
  assert.ok(result.ok); assert.equal(f.rules.length,2); assert.match(f.rules[0].action.redirect.extensionPath,/group=1&site=0/);
  assert.equal(f.storage.status.error,""); assert.match(f.storage.status.nativeError,/not found/);
  f.storage.localState.focus.end = Math.floor(Date.now()/1000)-1; await f.worker.sync(false);
  assert.equal(f.rules.length,0);
  const before=structuredClone(f.storage.localState);
  f.reject=true;
  const rejected=await f.send({command:"change",change:{type:"start",group_id:1,minutes:25,locked:false}});
  assert.match(rejected.error,/DNR rejection/);assert.deepEqual(f.storage.localState,before);
});
test("bidirectional native sync uses revisions and keeps both sides on offline conflicts", async () => {
  const f=await fixture(); const desktop=defaultState(); desktop.revision=4;desktop.groups[0].name="Desktop list";f.remote=desktop;
  await f.worker.sync();assert.equal(f.storage.localState.groups[0].name,"Desktop list");
  await f.send({command:"change",change:{type:"appearance",language:"ru",theme:"dark"}});
  assert.equal(f.remote.language,"ru");assert.equal(f.storage.syncMeta.dirty,false);
  const remoteCopy=f.remote;f.remote=null;
  await f.send({command:"change",change:{type:"appearance",language:"de",theme:"light"}});
  remoteCopy.revision++;remoteCopy.groups[0].name="Other desktop edit";f.remote=remoteCopy;
  await f.worker.sync();assert.equal(f.storage.localState.language,"de");assert.ok(f.storage.syncMeta.conflict);assert.equal(f.remote.language,"ru");
  await f.send({command:"resolve",source:"browser"});assert.equal(f.remote.language,"de");assert.equal(f.storage.syncMeta.conflict,null);assert.equal(f.storage.syncBackup.desktop.groups[0].name,"Other desktop edit");
  const latest=f.remote;latest.revision++;latest.theme="dark";f.remote=latest;
  await f.worker.sync();assert.equal(f.storage.localState.theme,"dark");
});
test("blocked attempts are bounded, deduplicated and contain configured domains only", async () => {
  const f=await fixture();await f.send({command:"change",change:{type:"start",group_id:1,minutes:25,locked:false}});
  const sender={id:chrome.runtime.id,tab:{id:9},url:chrome.runtime.getURL("blocked.html?group=1&site=0")};
  const visit="12345678-1234-1234-1234-123456789abc";
  assert.ok((await f.send({command:"attempt",visit},sender)).ok);
  await f.send({command:"attempt",visit},sender);assert.equal(f.storage.statistics.total,1);
  assert.equal(f.storage.statistics.recent[0].site,"youtube.com");assert.ok(!JSON.stringify(f.storage.statistics).includes("https://"));
  assert.equal((await f.send({command:"attempt",visit:"other-12345678901234"},{...sender,url:"https://example.com/"})).ok,false);
  await f.send({command:"clear-statistics"});assert.equal(f.storage.statistics.total,0);
});

test("legacy compiled policies stay intact until source settings can be migrated", async () => {
  const now = Math.floor(Date.now()/1000), original = defaultState();
  original.groups[0].name = "Preserved older list"; original.focus = {group_id:1,end:now+90,locked:false};
  const policy = policyFromState(original,now);
  const f = await fixture({policy});
  assert.equal(f.storage.localState,undefined);assert.equal(f.rules.length,2);
  const blocked = await f.send({command:"change",change:{type:"start",group_id:1,minutes:25}});
  assert.match(blocked.error,/migrate/);assert.equal(f.storage.policy.focus.end,now+90);
  f.remote=original;await f.worker.sync();
  assert.equal(f.storage.localState.groups[0].name,"Preserved older list");assert.equal(f.storage.localState.focus.end,now+90);
});

test("overlapping browser actions serialize without losing a newer mutation", async () => {
  const f=await fixture();
  const [start,appearance]=await Promise.all([
    f.send({command:"change",change:{type:"start",group_id:1,minutes:50,locked:false}}),
    f.send({command:"change",change:{type:"appearance",language:"fr",theme:"dark"}})
  ]);
  assert.ok(start.ok&&appearance.ok);assert.ok(f.storage.localState.focus);assert.equal(f.storage.localState.language,"fr");assert.equal(f.storage.localState.revision,2);
});
