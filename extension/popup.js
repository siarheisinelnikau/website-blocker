import { initializeI18n, t, statusMessage, currentLanguage } from "./i18n.js";

const api = globalThis.chrome;
async function render() {
  const status = await api.runtime.sendMessage({ command: "status" });
  document.getElementById("active").textContent = status.active?.length ? t("Active: {}", status.active.join(", ")) : t("No rule sets currently active.");
  document.getElementById("error").textContent = statusMessage(status.error);
  document.getElementById("details").textContent = status.contactedAt ? t("Last desktop sync: {}. Rule revision: {}.", new Date(status.contactedAt * 1000).toLocaleString(currentLanguage()), status.revision) : t("Not connected to the desktop app yet.");
}
document.getElementById("sync").addEventListener("click", async event => {
  event.target.disabled = true;
  try { await api.runtime.sendMessage({ command: "sync" }); await render(); }
  finally { event.target.disabled = false; }
});
await initializeI18n(() => { void render(); });
void render();
