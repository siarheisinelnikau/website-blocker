// One finite choreography per arrival/replay. No animation loop or hidden-tab work.
export function installScene(element, labels = ["example.com", "example.org", "example.net"]) {
  element.classList.add("focus-scene");
  element.innerHTML = `<div class="scene-grid"></div><div class="scene-halo"></div>
    <div class="scene-orbit orbit-a"></div><div class="scene-orbit orbit-b"></div>
    <div class="focus-core"><div class="core-light"></div><svg viewBox="0 0 100 114" aria-hidden="true"><path d="M50 5 90 21v33c0 26-17 43-40 54C27 97 10 80 10 54V21Z"/><path class="check-mark" d="m31 55 13 13 26-29"/></svg><span class="core-spark"></span></div>
    <div class="impact-ring ring-a"></div><div class="impact-ring ring-b"></div>
    <div class="site-comet comet-a"><i>↗</i><span></span><b>×</b></div><div class="site-comet comet-b"><i>↗</i><span></span><b>×</b></div><div class="site-comet comet-c"><i>↗</i><span></span><b>×</b></div>
    <div class="focus-trail"><span></span><span></span><span></span><span></span><span></span><span></span><span></span></div>
    <span class="scene-coordinate coord-a">FOCUS / ON</span><span class="scene-coordinate coord-b">DISTRACTIONS / OUT</span>`;
  element.querySelectorAll(".site-comet span").forEach((node, i) => { node.textContent = labels[i % Math.max(1, labels.length)] || "example.com"; });
  const media = matchMedia("(prefers-reduced-motion: reduce)");
  let end;
  const stop = () => { clearTimeout(end); element.classList.remove("scene-playing"); };
  const play = () => {
    stop();
    if (media.matches || document.hidden) return;
    // Reading the scene width restarts this finite CSS sequence, only on explicit events.
    void element.offsetWidth;
    element.classList.add("scene-playing");
    end = setTimeout(stop, 8200);
  };
  document.addEventListener("visibilitychange", () => { if (document.hidden) stop(); });
  media.addEventListener("change", stop);
  play(); return { play, stop };
}

export function countTo(node, value) {
  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const previous = Number(node.dataset.value || 0); node.dataset.value = value;
  if (reduced || document.hidden || previous === value) { node.textContent = value; return; }
  const start = performance.now();
  function tick(now) {
    const progress = Math.min(1, (now - start) / 800);
    node.textContent = Math.round(previous + (value - previous) * (1 - (1 - progress) ** 3));
    if (progress < 1 && !document.hidden) requestAnimationFrame(tick);
    else node.textContent = value;
  }
  requestAnimationFrame(tick);
}
