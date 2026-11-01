import { initializeI18n, t, statusMessage } from "./i18n.js";

async function check() {
  const button = document.getElementById("retry"); button.disabled = true;
  try {
    await chrome.runtime.sendMessage({ command: "sync" });
    const status = await chrome.runtime.sendMessage({ command: "status" });
    document.getElementById("title").textContent = status.error ? t("One more step in WebsiteBlocker") : t("Your browser is connected.");
    document.getElementById("status").textContent = status.error
      ? t("Open WebsiteBlocker → Settings → Connect browser, prepare this browser, then check again.") + " " + statusMessage(status.error)
      : t("Your current website rules have been applied. Return to WebsiteBlocker to choose your websites or start a focus session.");
  } catch (error) {
    document.getElementById("title").textContent = t("Connection needs attention");
    document.getElementById("status").textContent = t("Open WebsiteBlocker → Settings → Connect browser, then try again.") + " " + statusMessage(error.message);
  } finally { button.disabled = false; }
}
document.getElementById("retry").addEventListener("click", check);
await initializeI18n(() => { void check(); });
void check();
