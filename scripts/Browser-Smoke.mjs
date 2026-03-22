// Explicit opt-in live tests. Uses disposable profiles and restores the native-host registry value.
// No authentication, personal profile access, security interstitial bypass or browser sandbox disabling.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync, readFileSync, renameSync, existsSync } from "node:fs";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "node:http";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const browser = process.argv[2];
const headless = process.argv.includes("--headless");
const visualOnly = process.argv.includes("--visual");
const managed = !visualOnly && process.argv.includes("--managed");
assert.ok(["Chrome", "Edge"].includes(browser), "Usage: node scripts/Browser-Smoke.mjs Chrome|Edge");
const executable = browser === "Chrome" ? "C:/Program Files/Google/Chrome/Application/chrome.exe" : "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe";
const output = join(root, ".test-output", `live-${browser.toLowerCase()}`);
const dataDir = join(output, "data");
mkdirSync(dataDir, { recursive: true });
const backup = join(output, "native-registry-backup.json");
const manifest = managed ? join(dataDir, "native", `${browser}.json`) : join(output, "native-host.json");
const bundle = managed ? join(dataDir, "browser-companion", "extension") : join(root, "extension");
const version = JSON.parse(readFileSync(join(root, "extension/manifest.json"), "utf8")).version;
const hostExe = resolve(process.env.WEBSITEBLOCKER_TEST_EXE || join(root, `dist/WebsiteBlocker-v${version}.exe`));
const hostName = "com.websiteblocker.companion";
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const state = { schema: 1, revision: 0, groups: [{ id: 1, name: "Browser verification", enabled: true,
  patterns: [{ domain: "focus.websiteblocker.test", subdomains: true, path_prefix: "/" }], schedules: [] }],
  allowlist: [{ domain: "focus.websiteblocker.test", subdomains: false, path_prefix: "/allowed" }],
  focus: null, pause_until: 0, next_break: 0, startup_updates: false };
function save() {
  state.revision++;
  writeFileSync(join(dataDir, "state.next"), JSON.stringify(state));
  renameSync(join(dataDir, "state.next"), join(dataDir, "state.json"));
}
save();

function nativeRegistration(mode) {
  execFileSync("powershell.exe", ["-NoProfile", "-File", join(root, "scripts/Browser-NativeFixture.ps1"),
    "-Browser", browser, "-Mode", mode, "-Manifest", manifest, "-Backup", backup], { encoding: "utf8", windowsHide: true });
}
const server = createServer((req, res) => {
  res.writeHead(200, { "Content-Type": "text/html", "Cache-Control": "no-store" });
  res.end("<!doctype html><title>WebsiteBlocker local fixture</title><h1>Allowed local test navigation</h1>");
});
await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
const port = server.address().port;
const profile = join(output, `profile-${Date.now()}`);
const transportFlags = browser === "Edge"
  ? ["--remote-debugging-port=0"]
  : ["--remote-debugging-pipe"];
const processHandle = spawn(executable, [
  `--user-data-dir=${profile}`, ...transportFlags, ...(headless ? ["--headless=new", "--disable-gpu"] : []), "--enable-unsafe-extension-debugging",
  "--no-first-run", "--no-default-browser-check", "--disable-background-networking", "--disable-sync",
  "--window-size=1100,800", "--host-resolver-rules=MAP *.websiteblocker.test 127.0.0.1", "about:blank"
], { env: { ...process.env, WEBSITEBLOCKER_DATA_DIR: dataDir }, stdio: ["ignore", "ignore", "pipe", "pipe", "pipe"] });
let stderr = "", buffer = "", id = 0, pending = new Map(), exited = false, socket = null;
processHandle.stderr.on("data", data => { stderr = (stderr + data).slice(-24000); });
function receive(message) {
  const request = pending.get(message.id);
  if (request) { pending.delete(message.id); message.error ? request.reject(new Error(JSON.stringify(message.error))) : request.resolve(message.result); }
}
processHandle.on("exit", code => {
  // Edge's launcher can exit successfully after handing off to its browser process.
  if (browser === "Edge" && code === 0) return;
  exited = true;
  for (const request of pending.values()) request.reject(new Error(`Browser exited (${code}): ${stderr}`));
  pending.clear();
});
async function connect() {
if (browser === "Chrome") {
  processHandle.stdio[4].on("data", data => {
    buffer += data;
    for (;;) {
      const end = buffer.indexOf("\0"); if (end < 0) break;
      const message = JSON.parse(buffer.slice(0, end)); buffer = buffer.slice(end + 1); receive(message);
    }
  });
} else {
  for (let tries = 0; tries < 100 && !existsSync(join(profile, "DevToolsActivePort")); tries++) await sleep(100);
  assert.ok(existsSync(join(profile, "DevToolsActivePort")), `Edge did not expose its test-profile DevTools port: ${stderr}`);
  const [port, endpoint] = readFileSync(join(profile, "DevToolsActivePort"), "utf8").trim().split("\n");
  socket = new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  socket.onmessage = event => receive(JSON.parse(event.data));
  socket.onclose = () => { exited = true; };
}
}
function send(method, params = {}, sessionId) {
  return new Promise((resolve, reject) => {
    if (exited) return reject(new Error(`Browser is not running: ${stderr}`));
    const requestId = ++id;
    const timeout = setTimeout(() => { pending.delete(requestId); reject(new Error(`Timed out: ${method}`)); }, 20000);
    pending.set(requestId, { resolve: value => { clearTimeout(timeout); resolve(value); }, reject: error => { clearTimeout(timeout); reject(error); } });
    const message = JSON.stringify({ id: requestId, method, params, ...(sessionId ? { sessionId } : {}) });
    if (socket) socket.send(message); else processHandle.stdio[3].write(message + "\0");
  });
}
async function page(url) {
  const { targetId } = await send("Target.createTarget", { url });
  const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
  await send("Runtime.enable", {}, sessionId); await send("Page.enable", {}, sessionId);
  return { targetId, sessionId };
}
async function evaluate(session, expression) {
  const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }, session);
  if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  return result.result.value;
}
const checks = [];
function passed(name) { checks.push(name); console.log(`PASS ${browser}: ${name}`); }
let registered = false;
try {
  await connect();
  const version = await send("Browser.getVersion"); console.log(version.product);
  if (managed) {
    registered = true; nativeRegistration("Backup");
    const helper = join(root, "target", "x86_64-pc-windows-msvc", "release", "examples", "prepare-connection.exe");
    execFileSync(helper, [browser, dataDir, hostExe], {encoding:"utf8",windowsHide:true});
  }
  const { id: extensionId } = await send("Extensions.loadUnpacked", { path: bundle });
  if (!managed && !visualOnly) {
    writeFileSync(manifest, JSON.stringify({ name: hostName, description: "Temporary WebsiteBlocker browser test",
      path: hostExe, type: "stdio", allowed_origins: [`chrome-extension://${extensionId}/`] }));
    registered = true; nativeRegistration("Register");
  }
  const popup = await page(`chrome-extension://${extensionId}/popup.html`);
  const sync = async () => {
    await evaluate(popup.sessionId, "chrome.runtime.sendMessage({command:'sync'})");
    return evaluate(popup.sessionId, "chrome.storage.local.get(['status','policy']).then(r=>r)");
  };
  let current = await sync();
  if (!visualOnly) {
    assert.equal(current.status.error, ""); assert.equal(current.status.revision, state.revision);
    passed("real EXE native messaging protocol and isolated state sync without a desktop window");
  }
  if (managed) {
    const ready = JSON.parse(readFileSync(join(dataDir,"connections",`${browser.toLowerCase()}.json`)));
    assert.equal(ready.extension_id, extensionId); assert.equal(ready.error, null);
    assert.equal(ready.revision,state.revision);
    execFileSync(join(root,"target/x86_64-pc-windows-msvc/release/examples/prepare-connection.exe"),
      [browser,dataDir,hostExe,"verify"],{encoding:"utf8",windowsHide:true});
    passed("embedded companion extraction, native Rust registry registration and applied-rule confirmation");
  }
  state.focus = { group_id: 1, end: Math.floor(Date.now()/1000) + 180, locked: false }; save();
  if (visualOnly) {
    // Exercise the real EXE snapshot without changing an installed native registration.
    // Only the isolated profile's DNR/storage receives this policy fixture.
    const body = Buffer.from(JSON.stringify({protocol:1,command:"snapshot"}));
    const header = Buffer.alloc(4); header.writeUInt32LE(body.length);
    const response = execFileSync(hostExe,[`chrome-extension://${extensionId}/`],
      {input:Buffer.concat([header,body]),env:{...process.env,WEBSITEBLOCKER_DATA_DIR:dataDir},windowsHide:true,timeout:5000});
    assert.equal(response.readUInt32LE(0),response.length-4);
    const snapshot = JSON.parse(response.subarray(4)); assert.equal(snapshot.error,null);
    current = await evaluate(popup.sessionId, `(async()=>{const policy=${JSON.stringify(snapshot.policy)};
      const engine=await import(chrome.runtime.getURL('engine.js'));engine.validatePolicy(policy);
      const rules=engine.compileRules(policy,Math.floor(Date.now()/1000));
      const old=await chrome.declarativeNetRequest.getDynamicRules();
      await chrome.declarativeNetRequest.updateDynamicRules({removeRuleIds:old.map(r=>r.id),addRules:rules});
      const status={active:['Browser verification'],error:''};await chrome.storage.local.set({policy,status});return {policy,status};})()`);
    passed("actual EXE snapshot drives isolated engine/DNR fixture without changing native registration");
  } else current = await sync();
  assert.deepEqual(current.status.active, ["Browser verification"]); assert.equal(current.status.error, "");
  const dnr = await evaluate(popup.sessionId, "chrome.declarativeNetRequest.getDynamicRules()");
  assert.equal(dnr.length, 2); passed("real Chromium accepts native-generated redirect and allow rules");
  const navigation = await page("about:blank");
  async function navigate(host, path, blocked) {
    const url = `http://${host}:${port}${path}`;
    await send("Page.navigate", { url }, navigation.sessionId);
    let actual;
    for (let tries = 0; tries < 50; tries++) {
      await sleep(100);
      try { actual = await evaluate(navigation.sessionId, "({url:location.href,text:document.body?.innerText||''})"); } catch { continue; }
      if (blocked && actual.url === `chrome-extension://${extensionId}/blocked.html` && actual.text.includes("A little space")) break;
      if (!blocked && actual.url === url && actual.text.includes("Allowed local test navigation")) break;
    }
    assert.equal(actual.url.startsWith(`chrome-extension://${extensionId}/blocked.html`), blocked, JSON.stringify(actual));
    assert.ok(actual.text.includes(blocked ? "A little space" : "Allowed local test navigation"), JSON.stringify(actual));
  }
  await navigate("focus.websiteblocker.test", "/blocked", true); passed("top-level navigation reaches the real block page");
  await evaluate(navigation.sessionId, "document.fonts.ready.then(()=>Promise.all(document.getAnimations().map(a=>a.finished.catch(()=>{}))))");
  assert.ok((await evaluate(navigation.sessionId, "document.getElementById('rules').textContent")).includes("Browser verification"));
  assert.equal(await evaluate(navigation.sessionId, "document.querySelector('.brand img').naturalWidth"), 32);
  assert.equal(await evaluate(navigation.sessionId, `(()=>{const button=document.getElementById('breathe'); button.click(); const started=button.getAttribute('aria-pressed')==='true' && document.getElementById('breath-status').textContent.includes('Breathe in'); button.click(); return started && button.getAttribute('aria-pressed')==='false';})()`), true);
  const motion = await evaluate(navigation.sessionId, `new Promise(resolve=>{const art=document.getElementById('art'), r=art.getBoundingClientRect();art.dispatchEvent(new PointerEvent('pointermove',{clientX:r.x+r.width*.8,clientY:r.y+r.height*.7,pointerType:'mouse'}));requestAnimationFrame(()=>requestAnimationFrame(()=>resolve(art.style.getPropertyValue('--px'))));})`);
  assert.notEqual(motion, "");
  await evaluate(navigation.sessionId, "document.getElementById('art').dispatchEvent(new PointerEvent('pointerleave'))");
  assert.equal(await evaluate(navigation.sessionId, "document.getAnimations().filter(a=>a.playState==='running').length"), 0);
  passed("block page shows actual rules, working breath control and event-only parallax that settles");
  const screenshot = await send("Page.captureScreenshot", { format: "png" }, navigation.sessionId);
  writeFileSync(join(output, "blocked.png"), Buffer.from(screenshot.data, "base64"));
  await send("Emulation.setEmulatedMedia", {features:[{name:"prefers-reduced-motion",value:"reduce"}]}, navigation.sessionId);
  assert.equal(await evaluate(navigation.sessionId, `new Promise(resolve=>{const art=document.getElementById('art');art.dispatchEvent(new PointerEvent('pointermove',{clientX:100,clientY:100,pointerType:'mouse'}));requestAnimationFrame(()=>resolve(getComputedStyle(document.querySelector('.sculpture')).transform));})`), "none");
  assert.equal(await evaluate(navigation.sessionId, "document.getAnimations().filter(a=>a.playState==='running').length"), 0);
  passed("reduced-motion preference disables block-page animation and parallax");
  for (const width of [390, 768]) {
    await send("Emulation.setDeviceMetricsOverride", {width,height:844,deviceScaleFactor:1,mobile:width<620}, navigation.sessionId);
    assert.equal(await evaluate(navigation.sessionId, "document.documentElement.scrollWidth <= innerWidth"), true, `Horizontal overflow at ${width}px`);
    if (width===390) {
      const metrics = await send("Page.getLayoutMetrics", {}, navigation.sessionId);
      const mobile = await send("Page.captureScreenshot", {format:"png",captureBeyondViewport:true,clip:{x:0,y:0,width,height:Math.ceil(metrics.cssContentSize.height),scale:1}}, navigation.sessionId);
      writeFileSync(join(output,"blocked-mobile.png"),Buffer.from(mobile.data,"base64"));
    }
  }
  await send("Emulation.clearDeviceMetricsOverride", {}, navigation.sessionId);
  await send("Emulation.setEmulatedMedia", {features:[]}, navigation.sessionId);
  passed("block page fits phone/tablet widths without horizontal overflow");
  await navigate("sub.focus.websiteblocker.test", "/blocked", true); passed("subdomain blocking");
  await navigate("focus.websiteblocker.test", "/allowed/reference", false); passed("global path allowlist wins over redirect");
  await navigate("notfocus.websiteblocker.test", "/blocked", false); passed("domain boundary does not block an unrelated suffix host");
  await navigate("focus.websiteblocker.test", "/Allowed", true); passed("path allowlist remains case-sensitive");
  if (!visualOnly) {
  const mutation = await evaluate(popup.sessionId, `chrome.runtime.sendNativeMessage('${hostName}',{protocol:1,command:'stop_focus'})`);
  assert.ok(mutation.error); passed("native host rejects browser-side state mutation");
  state.focus = null; state.pause_until = Math.floor(Date.now()/1000) + 300; save();
  current = await sync(); assert.deepEqual(current.status.active, []); await navigate("focus.websiteblocker.test", "/blocked", false);
  passed("pause removes actual browser blocking");
  state.pause_until = 0; state.focus = { group_id: 1, end: Math.floor(Date.now()/1000) + 180, locked: false }; save();
  current = await sync();
  state.groups[0].schedules = [{ days:[0,1,2,3,4,5,6],start_minute:0,end_minute:1,timezone:"Invalid/Timezone" }]; save();
  current = await sync(); assert.match(current.status.error, /time.?zone/i); assert.notEqual(current.policy.revision, state.revision);
  passed("invalid IANA timezone is reported and preserves the last usable policy");
  const now = new Date(), minute = now.getUTCHours()*60 + now.getUTCMinutes();
  state.focus = null; state.groups[0].schedules = [{ days:[0,1,2,3,4,5,6],start_minute:(minute+1439)%1440,end_minute:(minute+3)%1440,timezone:"UTC" }]; save();
  current = await sync(); assert.equal(current.status.error, ""); assert.equal(current.status.active.length, 1);
  passed("recurring named-zone schedule activates real browser rules");
  state.groups[0].schedules = []; state.focus = { group_id: 1, end: Math.floor(Date.now()/1000)+3, locked:false }; save(); await sync();
  const hostManifest = JSON.parse(readFileSync(manifest)); writeFileSync(manifest, JSON.stringify({...hostManifest,path:join(output,"missing.exe")}));
  current = await sync(); assert.ok(current.status.error); assert.equal(current.status.active.length, 1);
  passed("native disconnect retains a valid focus cache and reports the connection error");
  await sleep(4000); current = await sync(); assert.equal(current.status.active.length, 0);
  assert.equal((await evaluate(popup.sessionId, "chrome.declarativeNetRequest.getDynamicRules()")).length, 0);
  await navigate("focus.websiteblocker.test", "/blocked", false); passed("focus expiry clears actual rules while the native host is unreachable");
  writeFileSync(manifest, JSON.stringify(hostManifest)); current = await sync(); assert.equal(current.status.error, ""); passed("connection recovery");
  }
  writeFileSync(join(output,"result.json"), JSON.stringify({ browser:version.product, extensionId, mode:visualOnly?"isolated-policy-visual-fixture":"native-transport", checks, nativeExe:hostExe, capturedAt:new Date().toISOString() },null,2));
} finally {
  if (!exited) await send("Browser.close").catch(()=>{});
  if (!exited) await new Promise(resolve => { const timer=setTimeout(()=>{processHandle.kill();resolve();},3000); processHandle.once("exit",()=>{clearTimeout(timer);resolve();}); });
  socket?.close();
  if (registered) nativeRegistration("Restore");
  server.close();
  writeFileSync(join(output,"browser-stderr.txt"),stderr);
}
