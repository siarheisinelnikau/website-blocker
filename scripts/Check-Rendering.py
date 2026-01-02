"""One bounded diagnostic launch; no computer use or injected user input."""
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
exe = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "dist/WebsiteBlocker-v0.1.0.exe"
fixture = root / ".test-output/rendering-gate"
data = fixture / "data"
data.mkdir(parents=True, exist_ok=True)
report = fixture / "report.json"
(data / "state.json").write_text(json.dumps({
    "schema": 1, "revision": 0,
    "groups": [{"id": 1, "name": "Distractions", "enabled": True,
                "patterns": [{"domain": "example.com", "subdomains": True, "path_prefix": "/"}], "schedules": []}],
    "allowlist": [], "focus": None, "pause_until": 0, "next_break": 0, "startup_updates": False,
}), encoding="utf-8")
env = os.environ.copy()
env.pop("WEBSITEBLOCKER_CAPTURE", None)
env["WEBSITEBLOCKER_DATA_DIR"] = str(data)
env["WEBSITEBLOCKER_DIAGNOSTICS"] = str(report)
if report.exists():
    report.unlink()
progress = report.with_suffix(".progress.txt")
if progress.exists():
    progress.unlink()
try:
    subprocess.run([str(exe)], cwd=root, env=env, timeout=22, capture_output=True, check=True)
except subprocess.TimeoutExpired:
    progress = report.with_suffix(".progress.txt")
    if progress.exists():
        print(progress.read_text(encoding="utf-8"), flush=True)
    raise
result = json.loads(report.read_text(encoding="utf-8"))
assert "CPU" in result["backend"] and "GDI" in result["backend"]
assert result["loaded_graphics_modules"] == ["gdi32.dll"], result["loaded_graphics_modules"]
samples = {sample["state"]: sample for sample in result["samples"]}
assert all(sample["cpu_seconds"] >= 0 for sample in samples.values()), "Process CPU measurement unavailable"
assert samples["minimized_end"]["window_minimized"] is True
assert samples["restored_settled"]["window_minimized"] is False
assert samples["hidden_end"]["window_visible"] is False
for label, start, end in (
    ("Visible idle", "settled_start", "visible_idle_end"),
    ("Minimized", "visible_idle_end", "minimized_end"),
    ("Restored idle", "restored_settled", "restored_idle_end"),
    ("Hidden", "restored_idle_end", "hidden_end"),
):
    before, after = samples[start], samples[end]
    duration = after["elapsed_seconds"] - before["elapsed_seconds"]
    cpu = 100 * (after["cpu_seconds"] - before["cpu_seconds"]) / duration
    delta = {name: after["counters"][name] - before["counters"][name]
             for name in ("layouts", "rasters", "presents")}
    assert all(value == 0 for value in delta.values()), (label, delta)
    assert cpu < 5, (label, cpu)
    print(f"{label}: {duration:.2f}s; CPU {cpu:.3f}% of one core "
          f"({cpu / result['logical_cores']:.4f}% machine); {delta}")
print("Backend:", result["backend"])
print("Observed modules:", result["loaded_graphics_modules"])
print("The process exited. OS input, perceptual smoothness and GPU/compositor telemetry are separate checks.")
