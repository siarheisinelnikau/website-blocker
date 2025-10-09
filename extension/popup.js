const api = globalThis.chrome;
async function render() {
  const status = await api.runtime.sendMessage({ command: "status" });
  document.getElementById("active").textContent = status.active?.length ? `Active: ${status.active.join(", ")}` : "No rule sets currently active.";
  document.getElementById("error").textContent = status.error || "";
  document.getElementById("details").textContent = status.contactedAt ? `Last desktop sync: ${new Date(status.contactedAt * 1000).toLocaleString()}. Rule revision: ${status.revision}.` : "Not connected to the desktop app yet.";
}
document.getElementById("sync").addEventListener("click", async event => {
  event.target.disabled = true;
  try { await api.runtime.sendMessage({ command: "sync" }); await render(); }
  finally { event.target.disabled = false; }
});
void render();
