const status = await chrome.runtime.sendMessage({ command: "status" });
document.getElementById("rules").textContent = status.active?.length ? `Active rule sets: ${status.active.join(", ")}` : "Your rules may have changed. Try navigating to the website again after the next sync.";
document.getElementById("back").addEventListener("click", () => { history.back(); });
