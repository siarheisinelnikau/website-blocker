import { initializeI18n, t, statusMessage, currentLanguage } from "./i18n.js";
import { parsePatterns, patternText, locked, nowSeconds } from "./state.js";
import { installScene, countTo } from "./motion.js";

const $ = id => document.getElementById(id);
let state, status = {}, meta = {}, stats = {}, draftRevision, dirtyEditor = false, timer;
const scene = installScene($("scene"));
$("replay").onclick = scene.play;
async function request(body) {
  const result = await chrome.runtime.sendMessage(body);
  if (result?.error) throw new Error(result.error);
  return result;
}
async function act(body) {
  try {
    $("feedback").classList.remove("error");
    await request(body); await render();
    $("feedback").textContent = meta.conflict ? t("Two copies need your choice") : body.command === "sync" ? $("sync-status").textContent : t("Saved on this computer.");
  } catch (e) { $("feedback").classList.add("error"); $("feedback").textContent = statusMessage(e.message); }
}
function selectTab(name) {
  document.querySelectorAll(".tab-panel").forEach(panel => {
    panel.hidden = panel.id !== name; panel.classList.remove("tab-enter");
    if (!panel.hidden) { void panel.offsetWidth; panel.classList.add("tab-enter"); }
  });
  document.querySelectorAll(".tabs button").forEach(button => button.classList.toggle("selected", button.dataset.tab === name));
}
document.querySelectorAll(".tabs button").forEach(button => { button.onclick = () => selectTab(button.dataset.tab); });

function row(parent, text, value, amount) {
  const item = document.createElement("div"), label = document.createElement("span"), detail = document.createElement("small");
  label.textContent = text; detail.textContent = value; item.append(label, detail);
  if (amount !== undefined) { item.className = "bar"; item.style.setProperty("--amount", `${amount}%`); }
  parent.append(item);
}
function tick() {
  clearTimeout(timer);
  if (!state || document.hidden) return;
  const now = nowSeconds(), active = state.focus?.end > now;
  const remaining = Math.max(0, (state.pause_until > now ? state.pause_until : state.focus?.end || now) - now);
  $("timer").textContent = remaining ? `${String(Math.floor(remaining / 60)).padStart(2, "0")}:${String(remaining % 60).padStart(2, "0")}` : "";
  $("session-title").textContent = t(active ? "Your focus session" : "Ready to focus?");
  $("start-form").hidden = active;
  $("break-controls").hidden = locked(state);
  $("lock-note").textContent = locked(state) ? t("This session is locked. Blocking ends when its timer runs out.") : "";
  $("stop").disabled = !active;
  $("resume").hidden = state.pause_until <= now;
  if (remaining) timer = setTimeout(tick, 1000);
}
document.addEventListener("visibilitychange", () => { if (document.hidden) clearTimeout(timer); else tick(); });
async function render() {
  const data = await chrome.storage.local.get(["localState", "status", "syncMeta", "statistics"]);
  state = data.localState; status = data.status || {}; meta = data.syncMeta || {}; stats = data.statistics || {};
  const sceneLabels = [...new Set(state?.groups.flatMap(g => g.patterns.map(p => p.domain)) || [])];
  $("scene").querySelectorAll(".site-comet span").forEach((node, i) => { node.textContent = sceneLabels[i % Math.max(1, sceneLabels.length)] || "example.com"; });
  $("mode").textContent = t(status.mode === "desktop" && !status.nativeError ? "Desktop connected" : "Works in your browser");
  $("active").textContent = status.active?.length ? t("Active: {}", status.active.join(" · ")) : t("No rule sets currently active.");
  countTo($("total"), stats.total || 0); countTo($("today"), stats.days?.[new Date().toLocaleDateString("en-CA")] || 0);
  countTo($("sites"), state?.groups.reduce((n, g) => n + g.patterns.length, 0) || 0);
  $("site-preview").replaceChildren();
  for (const group of state?.groups || []) for (const p of group.patterns.slice(0, 5)) row($("site-preview"), patternText(p), group.name);
  $("top-sites").replaceChildren(); $("recent").replaceChildren();
  const top = Object.entries(stats.sites || {}).sort((a, b) => b[1] - a[1]);
  for (const [site, count] of top) row($("top-sites"), site, count, 100 * count / Math.max(1, top[0]?.[1]));
  for (const event of stats.recent || []) row($("recent"), event.site, new Date(event.at * 1000).toLocaleTimeString(currentLanguage(), { hour: "2-digit", minute: "2-digit" }));
  if (!top.length) row($("top-sites"), t("No blocked visits yet. Your next small win starts here."), "◎");
  const selectedGroup = $("focus-group").value;
  $("focus-group").replaceChildren(...(state?.groups || []).filter(g => g.enabled).map(g => new Option(g.name, g.id)));
  if ([...$("focus-group").options].some(o => o.value === selectedGroup)) $("focus-group").value = selectedGroup;
  $("start-form").querySelector("button").disabled = !$("focus-group").options.length || !state;
  $("language").value = state?.language || "en"; $("theme").value = state?.theme || "light";
  $("version").textContent = `WebsiteBlocker ${chrome.runtime.getManifest().version}`;
  $("sync-status").textContent = status.error ? statusMessage(status.error) : meta.conflict ? t("Two copies need your choice") : status.nativeError ? t("Browser rules are active. Desktop sync will resume when it is available.") : status.contactedAt ? t("Last desktop sync: {}. Rule revision: {}.", new Date(status.contactedAt * 1000).toLocaleString(currentLanguage()), status.revision) : t("No desktop needed. All your rules stay in this browser.");
  $("conflict").hidden = !meta.conflict;
  $("conflict-banner").hidden = !meta.conflict;
  $("conflict-summary").textContent = meta.conflict ? `${t("Use browser settings")}: ${state.groups.map(g => `${g.name} (${g.patterns.length})`).join(", ")}\n${t("Use desktop settings")}: ${meta.conflict.groups.map(g => `${g.name} (${g.patterns.length})`).join(", ")}` : "";
  if (meta.conflict && !$("settings").hidden) $("sync-status").textContent = t("Two copies need your choice");
  if (!dirtyEditor) renderEditor();
  $("save-rules").disabled = !state || locked(state);
  tick();
}

function renderEditor() {
  if (!state) return;
  draftRevision = state.revision;
  $("groups").replaceChildren();
  for (const group of state.groups) addGroupEditor(group);
  $("allowlist").value = state.allowlist.map(patternText).join("\n");
}
function addGroupEditor(group) {
  const editor = document.createElement("article"); editor.className = "group-editor"; editor.dataset.id = group.id;
  editor.innerHTML = `<div class="group-heading"><label class="check"><input class="enabled" type="checkbox"><span></span></label><input class="group-name" maxlength="80" aria-label="List name"><button type="button" class="remove-group"></button></div><label><span class="patterns-label"></span><textarea class="patterns" rows="4" spellcheck="false" placeholder="example.com&#10;=video.example.org/shorts"></textarea></label><details><summary></summary><div class="schedules"></div><button type="button" class="add-schedule"></button></details>`;
  editor.querySelector(".check span").textContent = t("Enabled"); editor.querySelector(".enabled").checked = group.enabled;
  editor.querySelector(".group-name").value = group.name; editor.querySelector(".patterns").value = group.patterns.map(patternText).join("\n");
  editor.querySelector(".patterns-label").textContent = t("One domain or /path prefix per line. Allowed websites take priority.");
  const remove = editor.querySelector(".remove-group"); remove.textContent = t("Remove list"); remove.onclick = () => { editor.remove(); dirtyEditor = true; };
  editor.querySelector("summary").textContent = t("Recurring schedules");
  const add = editor.querySelector(".add-schedule"); add.textContent = t("Add schedule"); add.onclick = () => {
    if (editor.querySelectorAll(".schedule").length >= 14) return;
    addSchedule(editor, { days: [0,1,2,3,4], start_minute: 540, end_minute: 1020, timezone: Intl.DateTimeFormat().resolvedOptions().timeZone }); dirtyEditor = true;
  };
  group.schedules.forEach(schedule => addSchedule(editor, schedule));
  editor.addEventListener("input", () => { dirtyEditor = true; }); $("groups").append(editor);
}
const timeText = n => `${String(Math.floor(n / 60)).padStart(2, "0")}:${String(n % 60).padStart(2, "0")}`;
function addSchedule(editor, s) {
  const node = document.createElement("div"); node.className = "schedule";
  node.innerHTML = `<div class="weekdays"></div><div class="schedule-row"><input class="start" type="time" required aria-label="Start time"><input class="end" type="time" required aria-label="End time"><input class="timezone" aria-label="IANA timezone"><button type="button">×</button></div>`;
  for (let day = 0; day < 7; day++) {
    const label = document.createElement("label"), input = document.createElement("input"); input.type = "checkbox"; input.value = day; input.checked = s.days.includes(day);
    const date = new Date(Date.UTC(2024, 0, 1 + day)); label.append(input, date.toLocaleDateString(currentLanguage(), { weekday: "short", timeZone: "UTC" })); node.querySelector(".weekdays").append(label);
  }
  node.querySelector(".start").value = timeText(s.start_minute); node.querySelector(".end").value = timeText(s.end_minute); node.querySelector(".timezone").value = s.timezone;
  node.querySelector("button").setAttribute("aria-label", t("Remove schedule")); node.querySelector("button").onclick = () => { node.remove(); dirtyEditor = true; };
  editor.querySelector(".schedules").append(node);
}
$("allowlist").oninput = () => { dirtyEditor = true; };
$("add-group").onclick = () => {
  if (!state || $("groups").children.length >= 20) return;
  const id = Math.max(0, ...[...$("groups").children].map(node => Number(node.dataset.id))) + 1;
  addGroupEditor({ id, name: t("New list"), enabled: true, patterns: [], schedules: [] }); dirtyEditor = true;
};
$("save-rules").onclick = async () => {
  try {
    const minutes = text => { const [h,m] = text.split(":").map(Number); return h * 60 + m; };
    const groups = [...$("groups").children].map(node => ({ id: Number(node.dataset.id), name: node.querySelector(".group-name").value.trim(), enabled: node.querySelector(".enabled").checked,
      patterns: parsePatterns(node.querySelector(".patterns").value), schedules: [...node.querySelectorAll(".schedule")].map(s => ({ days: [...s.querySelectorAll(".weekdays input:checked")].map(i => Number(i.value)),
        start_minute: minutes(s.querySelector(".start").value), end_minute: minutes(s.querySelector(".end").value), timezone: s.querySelector(".timezone").value.trim() })) }));
    await request({ command: "change", change: { type: "rules", revision: draftRevision, groups, allowlist: parsePatterns($("allowlist").value) } });
    dirtyEditor = false; await render(); $("feedback").classList.remove("error"); $("feedback").textContent = t("Saved on this computer.");
  } catch (e) { $("feedback").classList.add("error"); $("feedback").textContent = statusMessage(e.message); }
};
$("start-form").onsubmit = event => { event.preventDefault(); scene.play(); void act({ command: "change", change: { type: "start", group_id: Number($("focus-group").value), minutes: Number($("minutes").value), locked: $("locked").checked } }); };
for (const type of ["break", "stop", "resume"]) $(type).onclick = () => { void act({ command: "change", change: { type, phrase: $("phrase").value } }); $("phrase").value = ""; };
for (const id of ["language", "theme"]) $(id).onchange = () => { void act({ command: "change", change: { type: "appearance", language: $("language").value, theme: $("theme").value } }); };
$("sync").onclick = async () => { document.body.classList.add("syncing"); $("sync").disabled = true; try { await act({ command: "sync" }); } finally { document.body.classList.remove("syncing"); $("sync").disabled = false; } };
for (const source of ["browser", "desktop"]) $("keep-" + source).onclick = () => { void act({ command: "resolve", source }); };
$("clear-stats").onclick = () => { void act({ command: "clear-statistics" }); };
$("review-conflict").onclick = () => selectTab("settings");
await initializeI18n(() => { void render(); });
await chrome.runtime.sendMessage({ command: "status" }); await render();
chrome.storage.onChanged.addListener((changes, area) => { if (area === "local" && (changes.status || changes.statistics)) void render(); });
if (["focus", "websites", "activity", "settings"].includes(location.hash.slice(1))) selectTab(location.hash.slice(1));
