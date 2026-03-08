"""Launch the real GUI with disposable settings and save its renderer's PNG frames."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

root = Path(__file__).resolve().parent.parent
version = json.loads((root / "extension/manifest.json").read_text(encoding="utf-8"))["version"]
exe = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / f"dist/WebsiteBlocker-v{version}.exe"
data = root / ".test-output/readme-capture/data"
data.mkdir(parents=True, exist_ok=True)
images = root / "docs/images"
images.mkdir(parents=True, exist_ok=True)
state = {
    "schema": 1, "revision": 0,
    "groups": [{"id": 1, "name": "Distractions", "enabled": True,
                "patterns": [{"domain": "example.com", "subdomains": True, "path_prefix": "/"},
                             {"domain": "example.org", "subdomains": True, "path_prefix": "/"}], "schedules": []}],
    "allowlist": [], "focus": None, "pause_until": 0, "next_break": 0, "startup_updates": False,
}
(data / "state.json").write_text(json.dumps(state), encoding="utf-8")
for tab in ("focus", "rules", "settings", "browser"):
    output = images / f"{tab}.png"
    started = time.time()
    env = os.environ.copy()
    env.pop("WEBSITEBLOCKER_DIAGNOSTICS", None)
    env["WEBSITEBLOCKER_DATA_DIR"] = str(data)
    env["WEBSITEBLOCKER_CAPTURE"] = f"{tab}|{output}"
    subprocess.run([str(exe)], cwd=root, env=env, check=True, timeout=30, capture_output=True)
    assert output.is_file() and output.stat().st_mtime >= started - 1, f"No fresh capture: {output}"
    print(f"Captured real {tab} window: {output}")
