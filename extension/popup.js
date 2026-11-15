import { initializeI18n, t, statusMessage } from "./i18n.js";
let timer;
async function render() {
  const { status = {}, localState: state, statistics: stats = {} } = await chrome.storage.local.get(["status", "localState", "statistics"]);
  document.getElementById("active").textContent = status.active?.length ? t("Active: {}", status.active.join(", ")) : t("No rule sets currently active.");
  document.getElementById("error").textContent = statusMessage(status.error);
  document.getElementById("details").textContent = t(status.conflict ? "Two copies need your choice" : status.nativeError || !status.contactedAt ? "Works in your browser" : "Desktop connected");
  document.getElementById("total").textContent = stats.total || 0;
  document.getElementById("today").textContent = stats.days?.[new Date().toLocaleDateString("en-CA")] || 0;
  const select = document.getElementById("group"), selected = select.value;
  select.replaceChildren(...(state?.groups || []).filter(g => g.enabled).map(g => new Option(g.name, g.id)));
  if ([...select.options].some(o => o.value === selected)) select.value = selected;
  const remaining = Math.max(0, (state?.focus?.end || 0) - Math.floor(Date.now() / 1000));
  document.getElementById("quick-start").hidden = remaining > 0;
  document.querySelector("#quick-start button").disabled = !select.options.length;
  document.getElementById("timer").textContent = remaining ? `${Math.floor(remaining / 60)}:${String(remaining % 60).padStart(2, "0")}` : "";
  clearTimeout(timer); if (remaining && !document.hidden) timer = setTimeout(render, 1000);
}
document.getElementById("quick-start").onsubmit = async event => {
  event.preventDefault();
  const result = await chrome.runtime.sendMessage({ command: "change", change: { type: "start", group_id: Number(document.getElementById("group").value), minutes: Number(document.getElementById("minutes").value), locked: false } });
  await render(); if (result?.error) document.getElementById("error").textContent = statusMessage(result.error);
};
document.getElementById("manage").onclick = async () => {
  const {status} = await chrome.storage.local.get("status");
  await chrome.tabs.create({url:chrome.runtime.getURL("dashboard.html" + (status?.conflict ? "#settings" : ""))});
};
document.getElementById("sync").onclick = async event => {
  event.target.disabled = true;
  try { await chrome.runtime.sendMessage({ command: "sync" }); await render(); }
  finally { event.target.disabled = false; }
};
await initializeI18n(() => { void render(); }); await render();
document.addEventListener("visibilitychange", () => { if (document.hidden) clearTimeout(timer); else void render(); });
