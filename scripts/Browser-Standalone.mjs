// Real Chromium DNR and native transport checks; disposable profile and example domains only.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "node:http";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const output = join(root, ".test-output", `standalone-${Date.now()}`);
mkdirSync(output, { recursive:true });
const dataDir = join(output,"data"); mkdirSync(dataDir);
const manifest = join(output,"native-host.json"), backup = join(output,"registration-backup.json");
const version = JSON.parse(readFileSync(join(root,"extension/manifest.json"))).version;
const exe = join(root,`dist/WebsiteBlocker-v${version}.exe`);
const missing = join(output,"desktop-not-installed.exe");
const host = "com.websiteblocker.companion";
const browserName = process.argv.includes("Edge") ? "Edge" : "Chrome";
const executable = browserName === "Chrome" ? "C:/Program Files/Google/Chrome/Application/chrome.exe" : "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe";
const sleep = ms=>new Promise(resolve=>setTimeout(resolve,ms));
const server = createServer((req,res)=>{res.writeHead(200,{"Content-Type":"text/html"});res.end("<!doctype html><title>Allowed fixture</title><h1>Allowed local fixture</h1>");});
await new Promise(r=>server.listen(0,"127.0.0.1",r)); const port=server.address().port;
const profile=join(output,"profile");
const browser=spawn(executable,[`--user-data-dir=${profile}`,"--headless=new","--disable-gpu",...(browserName==="Chrome"?["--remote-debugging-pipe"]:["--remote-debugging-port=0"]),
  "--enable-unsafe-extension-debugging","--no-first-run","--no-default-browser-check","--disable-background-networking","--disable-sync",
  "--window-size=1200,1000","--host-resolver-rules=MAP *.websiteblocker.test 127.0.0.1","about:blank"],
  {env:{...process.env,WEBSITEBLOCKER_DATA_DIR:dataDir},stdio:["ignore","ignore","pipe","pipe","pipe"],windowsHide:true});
let buffer="",stderr="",id=0,socket,exited=false,registered=false; const pending=new Map(); const checks=[];
browser.stderr.on("data",data=>stderr=(stderr+data).slice(-12000));
const receive=message=>{const task=pending.get(message.id);if(task){pending.delete(message.id);message.error?task.reject(new Error(JSON.stringify(message.error))):task.resolve(message.result);}};
browser.on("exit",code=>{if(browserName==="Edge"&&code===0)return;exited=true;for(const task of pending.values())task.reject(new Error(`Browser exited: ${stderr}`));pending.clear();});
if(browserName==="Chrome")browser.stdio[4].on("data",data=>{buffer+=data;for(;;){const end=buffer.indexOf("\0");if(end<0)break;receive(JSON.parse(buffer.slice(0,end)));buffer=buffer.slice(end+1);}});
function send(method,params={},sessionId){return new Promise((resolve,reject)=>{const requestId=++id;const timer=setTimeout(()=>{pending.delete(requestId);reject(new Error(`Timeout: ${method}`));},15000);pending.set(requestId,{resolve:value=>{clearTimeout(timer);resolve(value);},reject:e=>{clearTimeout(timer);reject(e);}});const message=JSON.stringify({id:requestId,method,params,...(sessionId?{sessionId}:{})});if(socket)socket.send(message);else browser.stdio[3].write(message+"\0");});}
async function page(url){const {targetId}=await send("Target.createTarget",{url});const {sessionId}=await send("Target.attachToTarget",{targetId,flatten:true});await send("Runtime.enable",{},sessionId);await send("Page.enable",{},sessionId);return sessionId;}
async function evaluate(sessionId,expression){const r=await send("Runtime.evaluate",{expression,returnByValue:true,awaitPromise:true},sessionId);if(r.exceptionDetails)throw new Error(JSON.stringify(r.exceptionDetails));return r.result.value;}
async function until(sessionId,expression){for(let i=0;i<200;i++){try{if(await evaluate(sessionId,expression))return;}catch{}await sleep(60);}throw new Error(`Condition not reached: ${expression}`);}
function registration(mode){execFileSync("powershell.exe",["-NoProfile","-File",join(root,"scripts/Browser-NativeFixture.ps1"),"-Browser",browserName,"-Mode",mode,"-Manifest",manifest,"-Backup",backup],{windowsHide:true,timeout:10000});}
function passed(name){checks.push(name);console.log(`PASS ${browserName}: ${name}`);}
try {
  if(browserName==="Edge") {
    for(let i=0;i<100&&!existsSync(join(profile,"DevToolsActivePort"));i++)await sleep(100);
    const [port,endpoint]=readFileSync(join(profile,"DevToolsActivePort"),"utf8").trim().split("\n");
    socket=new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);await new Promise((r,j)=>{socket.onopen=r;socket.onerror=j;});socket.onmessage=e=>receive(JSON.parse(e.data));
  }
  const browserVersion=await send("Browser.getVersion");console.log(browserVersion.product);
  // A missing binary proves independent enforcement even on a PC with an installed host.
  const {id:extensionId}=await send("Extensions.loadUnpacked",{path:join(root,"extension")});
  const hostManifest={name:host,description:"Disposable WebsiteBlocker fixture",path:missing,type:"stdio",allowed_origins:[`chrome-extension://${extensionId}/`]};
  writeFileSync(manifest,JSON.stringify(hostManifest));registered=true;registration("Register");
  const studio=await page(`chrome-extension://${extensionId}/dashboard.html`);
  await until(studio,"document.getElementById('focus-group')?.options.length>0");
  const command=body=>evaluate(studio,`chrome.runtime.sendMessage(${JSON.stringify(body)})`);
  await command({command:"sync"});
  let snapshot=await evaluate(studio,"chrome.storage.local.get(['localState','status'])");
  assert.equal(snapshot.status.error,"");assert.ok(snapshot.status.nativeError);
  const state=snapshot.localState;
  const pattern={domain:"focus.websiteblocker.test",subdomains:true,path_prefix:"/"};
  const groups=[{id:1,name:"Deep work",enabled:true,patterns:[pattern,{domain:"video.websiteblocker.test",subdomains:true,path_prefix:"/"},{domain:"news.websiteblocker.test",subdomains:true,path_prefix:"/"}],schedules:[]}];
  assert.ok((await command({command:"change",change:{type:"rules",revision:state.revision,groups,allowlist:[{...pattern,subdomains:false,path_prefix:"/allowed"}]}})).ok);
  assert.ok((await command({command:"change",change:{type:"start",group_id:1,minutes:25,locked:false}})).ok);
  const rules=await evaluate(studio,"chrome.declarativeNetRequest.getDynamicRules()");assert.equal(rules.length,4);
  passed("no desktop binary: create rules, allowlist and focus timer; Chromium accepts DNR redirects with rule identifiers");
  const navigation=await page("about:blank");
  const navigate=async(path,blocked)=>{
    await send("Page.navigate",{url:`http://focus.websiteblocker.test:${port}${path}`},navigation);
    await until(navigation,blocked?"location.pathname==='/blocked.html'&&document.getElementById('wins')?.textContent==='1'":"document.body?.textContent.includes('Allowed local fixture')");
  };
  await navigate("/private?q=never-store-this",true);
  let statistics=await evaluate(studio,"chrome.storage.local.get('statistics').then(r=>r.statistics)");assert.equal(statistics.total,1);assert.equal(statistics.recent[0].site,"focus.websiteblocker.test");assert.ok(!JSON.stringify(statistics).includes("never-store"));
  await send("Page.reload",{},navigation);await until(navigation,"document.getElementById('wins')?.textContent==='1'");
  statistics=await evaluate(studio,"chrome.storage.local.get('statistics').then(r=>r.statistics)");assert.equal(statistics.total,1);
  passed("real navigation records one configured-domain attempt; reload is deduplicated and full URLs are absent");
  await until(navigation,"document.getAnimations().filter(a=>a.playState==='running').length===0");
  const screenshot=async(session,name,full=false)=>{
    await send("Page.bringToFront",{},session);
    await evaluate(session,"scrollTo(0,0);document.fonts.ready");
    await evaluate(session,"Promise.all(document.getAnimations().filter(a=>a.playState==='running').map(a=>a.finished.catch(()=>{})))");
    const metrics=await send("Page.getLayoutMetrics",{},session);
    const capture=await send("Page.captureScreenshot",{format:"png",...(full?{captureBeyondViewport:true,clip:{x:0,y:0,width:metrics.cssContentSize.width,height:metrics.cssContentSize.height,scale:1}}:{})},session);
    writeFileSync(join(output,name),Buffer.from(capture.data,"base64"));
  };
  await screenshot(navigation,"blocked.png",true);
  await evaluate(navigation,"document.getElementById('replay').click()");
  assert.ok(await evaluate(navigation,"document.getAnimations().some(a=>a.playState==='running')"));
  await send("Emulation.setEmulatedMedia",{features:[{name:"prefers-reduced-motion",value:"reduce"}]},navigation);
  assert.equal(await evaluate(navigation,"document.getAnimations().filter(a=>a.playState==='running').length"),0);
  await evaluate(navigation,"document.getElementById('breathe').click()");
  assert.equal(await evaluate(navigation,"document.getElementById('breathe').getAttribute('aria-pressed')"),"true");
  await evaluate(navigation,"document.getElementById('breathe').click()");
  passed("finite impact choreography settles, replays and honors reduced motion; breathing control works");
  await screenshot(studio,"studio.png",true);
  await evaluate(studio,"document.querySelector('[data-tab=settings]').click()");await sleep(500);await screenshot(studio,"extension-settings.png",true);
  // Exercise actual form editing, schedules, draft preservation and live localization.
  await evaluate(studio,"document.querySelector('[data-tab=websites]').click();document.querySelector('.group-name').value='Deep work edited';document.querySelector('.group-name').dispatchEvent(new Event('input',{bubbles:true}));document.getElementById('save-rules').click()");
  await until(studio,"document.getElementById('feedback').textContent.includes('Saved')");
  assert.equal(await evaluate(studio,"chrome.storage.local.get('localState').then(r=>r.localState.groups[0].name)"),"Deep work edited");
  await screenshot(studio,"extension-websites.png",true);
  for(const language of ["en","ru","fr","de","es","it"])for(const theme of ["light","dark"]){
    await command({command:"change",change:{type:"appearance",language,theme}});
    await until(studio,`document.documentElement.lang==='${language}'&&document.documentElement.dataset.theme==='${theme}'`);
    for(const session of [studio,navigation]){
      await send("Emulation.setDeviceMetricsOverride",{width:390,height:844,deviceScaleFactor:1,mobile:true},session);
      assert.ok(await evaluate(session,"document.documentElement.scrollWidth<=innerWidth"),`${language}/${theme} overflow`);
    }
    if(language==="ru"&&theme==="dark")await screenshot(navigation,"blocked-ru-dark.png",true);
  }
  passed("actual website editor saves; six languages and both themes fit 390px studio and block page");
  await navigate("/allowed/path",false);passed("global allowlist wins over active focus rules");
  // Restore English and use real native transport against the packaged EXE, without a GUI.
  await command({command:"change",change:{type:"appearance",language:"en",theme:"light"}});
  writeFileSync(manifest,JSON.stringify({...hostManifest,path:exe}));
  await command({command:"sync"});
  snapshot=await evaluate(studio,"chrome.storage.local.get(['status','syncMeta','localState'])");assert.ok(snapshot.syncMeta.conflict);
  const resolved=await command({command:"resolve",source:"browser"});assert.ok(!resolved.error,JSON.stringify(resolved));
  let desktop=JSON.parse(readFileSync(join(dataDir,"state.json")));assert.equal(desktop.groups[0].name,"Deep work edited");assert.ok(desktop.focus);
  passed("installed EXE: first-connection conflict keeps browser edits and explicit choice imports rules and session");
  await command({command:"change",change:{type:"appearance",language:"ru",theme:"dark"}});
  desktop=JSON.parse(readFileSync(join(dataDir,"state.json")));assert.equal(desktop.language,"ru");
  desktop.revision++;desktop.theme="light";writeFileSync(join(dataDir,"state.json"),JSON.stringify(desktop));
  await command({command:"sync"});snapshot=await evaluate(studio,"chrome.storage.local.get('localState')");assert.equal(snapshot.localState.theme,"light");
  assert.equal(JSON.parse(readFileSync(join(dataDir,"activity",(await evaluate(studio,"chrome.storage.local.get('clientId').then(r=>r.clientId)"))+".json"))).total,1);
  passed("real native messaging synchronizes edits both ways and mirrors per-profile attempt counters without duplication");
  // A running locked session must not be stopped through the browser or native replacement.
  await command({command:"change",change:{type:"stop",phrase:"END FOCUS"}});
  await command({command:"change",change:{type:"start",group_id:1,minutes:1,locked:true}});
  assert.ok((await command({command:"change",change:{type:"stop",phrase:"END FOCUS"}})).error);
  const {localState:locked}=await evaluate(studio,"chrome.storage.local.get('localState')");
  const bypass={...locked,focus:null};
  const rejected=await evaluate(studio,`chrome.runtime.sendNativeMessage('${host}',${JSON.stringify({protocol:1,command:"sync",expected_revision:locked.revision,state:bypass})})`);assert.ok(rejected.error);
  passed("locked session cannot be stopped through browser controls or a native replacement");
  writeFileSync(join(output,"result.json"),JSON.stringify({version,browser:browserVersion.product,checks,output},null,2));
  console.log(`Evidence: ${output}`);
}finally{
  if(!exited)await send("Browser.close").catch(()=>{});
  if(!exited)await new Promise(r=>{const timeout=setTimeout(()=>{browser.kill();r();},3000);browser.once("exit",()=>{clearTimeout(timeout);r();});});
  socket?.close();if(registered)registration("Restore");server.close();writeFileSync(join(output,"stderr.txt"),stderr);
}
