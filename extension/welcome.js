async function check() {
  const button = document.getElementById("retry"); button.disabled = true;
  try {
    await chrome.runtime.sendMessage({ command: "sync" });
    const status = await chrome.runtime.sendMessage({ command: "status" });
    document.getElementById("title").textContent = status.error ? "One more step in WebsiteBlocker" : "Your browser is connected.";
    document.getElementById("status").textContent = status.error
      ? "Open WebsiteBlocker → Settings → Connect browser, prepare this browser, then check again. " + status.error
      : "Your current website rules have been applied. Return to WebsiteBlocker to choose your websites or start a focus session.";
  } catch (error) {
    document.getElementById("title").textContent = "Connection needs attention";
    document.getElementById("status").textContent = "Open WebsiteBlocker → Settings → Connect browser, then try again. " + error.message;
  } finally { button.disabled = false; }
}
document.getElementById("retry").addEventListener("click", check);
void check();
