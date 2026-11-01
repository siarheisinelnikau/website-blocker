import { initializeI18n, t } from "./i18n.js";

await initializeI18n();

const rules = document.getElementById("rules");
try {
  const status = await chrome.runtime.sendMessage({ command: "status" });
  if (status?.active?.length) rules.textContent = t("Protected by {}", status.active.join(" · "));
} catch {
  // The page stays useful if the extension worker is temporarily unavailable.
}

document.getElementById("back").addEventListener("click", () => {
  if (history.length > 1) history.back();
  else location.replace("about:blank");
});

const reduced = matchMedia("(prefers-reduced-motion: reduce)");
const art = document.getElementById("art");
let frame = 0;
function resetArt() {
  cancelAnimationFrame(frame);
  frame = 0;
  art.style.removeProperty("--px");
  art.style.removeProperty("--py");
}
art.addEventListener("pointermove", event => {
  if (reduced.matches || event.pointerType === "touch" || frame || document.hidden) return;
  const bounds = art.getBoundingClientRect();
  const x = ((event.clientX - bounds.left) / bounds.width - .5) * 12;
  const y = ((event.clientY - bounds.top) / bounds.height - .5) * 10;
  frame = requestAnimationFrame(() => {
    art.style.setProperty("--px", `${x.toFixed(2)}px`);
    art.style.setProperty("--py", `${y.toFixed(2)}px`);
    frame = 0;
  });
});
art.addEventListener("pointerleave", resetArt);
reduced.addEventListener("change", resetArt);

const breathe = document.getElementById("breathe");
const message = document.getElementById("breath-status");
let timers = [];
function stopBreathing(text = "No rush. Just one intentional next step.") {
  timers.forEach(clearTimeout);
  timers = [];
  document.body.classList.remove("breathing");
  breathe.setAttribute("aria-pressed", "false");
  message.textContent = t(text);
}
breathe.addEventListener("click", () => {
  if (timers.length) return stopBreathing();
  document.body.classList.add("breathing");
  breathe.setAttribute("aria-pressed", "true");
  ["Breathe in. Let your shoulders soften.", "Breathe out. You have time.",
    "One more breath in. Just be here.", "And breathe out. One thing at a time.",
    "A little clearer. Choose your next small step."].forEach((text, index) => {
    if (!index) message.textContent = t(text);
    else timers.push(setTimeout(() => {
      if (index === 4) stopBreathing(text);
      else message.textContent = t(text);
    }, index * 3000));
  });
});
document.addEventListener("visibilitychange", () => {
  if (document.hidden) { resetArt(); stopBreathing(); }
});
