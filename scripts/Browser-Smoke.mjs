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
assert.ok(["Chrome", "Edge"].includes(browser), "Usage: node scripts/Browser-Smoke.mjs Chrome|Edge");
const executable = browser === "Chrome" ? "C:/Program Files/Google/Chrome/Application/chrome.exe" : "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe";
const output = join(root, ".test-output", `live-${browser.toLowerCase()}`);
const dataDir = join(output, "data");
mkdirSync(dataDir, { recursive: true });
const backup = join(output, "native-registry-backup.json"), manifest = join(output, "native-host.json");
const hostExe = resolve(process.env.WEBSITEBLOCKER_TEST_EXE || join(root, "dist/WebsiteBlocker-v0.1.0.exe"));
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
const profile = join(output, browser === "Edge" ? `profile-${Date.now()}` : "profile");
const transportFlags = browser === "Edge"
  ? ["--remote-debugging-port=0", `--load-extension=${join(root, "extension")}`]
  : ["--remote-debugging-pipe"];
const processHandle = spawn(executable, [
  `--user-data-dir=${profile}`, ...transportFlags, "--enable-unsafe-extension-debugging",
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
  const { id: extensionId } = await send("Extensions.loadUnpacked", { path: join(root, "extension") });
  writeFileSync(manifest, JSON.stringify({ name: hostName, description: "Temporary WebsiteBlocker browser test",
    path: hostExe, type: "stdio", allowed_origins: [`chrome-extension://${extensionId}/`] }));
  registered = true; nativeRegistration("Register");
  const popup = await page(`chrome-extension://${extensionId}/popup.html`);
  const sync = async () => {
    await evaluate(popup.sessionId, "chrome.runtime.sendMessage({command:'sync'})");
    return evaluate(popup.sessionId, "chrome.storage.local.get(['status','policy']).then(r=>r)");
  };
  let current = await sync(); assert.equal(current.status.error, ""); assert.equal(current.status.revision, state.revision);
  passed("real EXE native messaging protocol and isolated state sync without a desktop window");
  state.focus = { group_id: 1, end: Math.floor(Date.now()/1000) + 180, locked: false }; save();
  current = await sync(); assert.deepEqual(current.status.active, ["Browser verification"]); assert.equal(current.status.error, "");
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
  const screenshot = await send("Page.captureScreenshot", { format: "png" }, navigation.sessionId);
  writeFileSync(join(output, "blocked.png"), Buffer.from(screenshot.data, "base64"));
  await navigate("sub.focus.websiteblocker.test", "/blocked", true); passed("subdomain blocking");
  await navigate("focus.websiteblocker.test", "/allowed/reference", false); passed("global path allowlist wins over redirect");
  await navigate("notfocus.websiteblocker.test", "/blocked", false); passed("domain boundary does not block an unrelated suffix host");
  await navigate("focus.websiteblocker.test", "/Allowed", true); passed("path allowlist remains case-sensitive");
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
  writeFileSync(join(output,"result.json"), JSON.stringify({ browser:version.product, extensionId, checks, nativeExe:hostExe, capturedAt:new Date().toISOString() },null,2));
} finally {
  if (!exited) await send("Browser.close").catch(()=>{});
  if (!exited) await new Promise(resolve => { const timer=setTimeout(()=>{processHandle.kill();resolve();},3000); processHandle.once("exit",()=>{clearTimeout(timer);resolve();}); });
  socket?.close();
  if (registered) nativeRegistration("Restore");
  server.close();
  writeFileSync(join(output,"browser-stderr.txt"),stderr);
}
