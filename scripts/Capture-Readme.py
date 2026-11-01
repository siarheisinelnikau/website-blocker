"""Launch the real GUI with disposable settings and save its renderer's PNG frames."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time

root = Path(__file__).resolve().parent.parent
version = json.loads((root / "extension/manifest.json").read_text(encoding="utf-8"))["version"]
parser = argparse.ArgumentParser()
parser.add_argument("exe", nargs="?")
parser.add_argument("--language", choices=["en", "ru", "fr", "de", "es", "it"], default="en")
parser.add_argument("--theme", choices=["light", "dark"], default="light")
parser.add_argument("--screens", nargs="+", choices=["focus", "rules", "settings", "browser"], default=["focus", "rules", "settings", "browser"])
parser.add_argument("--output-dir", type=Path, default=root / "docs/images")
args = parser.parse_args()
exe = Path(args.exe).resolve() if args.exe else root / f"dist/WebsiteBlocker-v{version}.exe"
data = root / f".test-output/readme-capture/{args.language}-{args.theme}/data"
data.mkdir(parents=True, exist_ok=True)
images = args.output_dir.resolve()
images.mkdir(parents=True, exist_ok=True)
state = {
    "schema": 1, "revision": 0,
    "groups": [{"id": 1, "name": "Distractions", "enabled": True,
                "patterns": [{"domain": "example.com", "subdomains": True, "path_prefix": "/"},
                             {"domain": "example.org", "subdomains": True, "path_prefix": "/"}], "schedules": []}],
    "allowlist": [], "focus": None, "pause_until": 0, "next_break": 0, "startup_updates": False, "language": args.language, "theme": args.theme,
}
(data / "state.json").write_text(json.dumps(state), encoding="utf-8")
for tab in args.screens:
    output = images / f"{tab}.png"
    started = time.time()
    env = os.environ.copy()
    env.pop("WEBSITEBLOCKER_DIAGNOSTICS", None)
    env["WEBSITEBLOCKER_DATA_DIR"] = str(data)
    env["WEBSITEBLOCKER_CAPTURE"] = f"{tab}|{output}"
    subprocess.run([str(exe)], cwd=root, env=env, check=True, timeout=30, capture_output=True)
    assert output.is_file() and output.stat().st_mtime >= started - 1, f"No fresh capture: {output}"
    print(f"Captured real {tab} window: {output}")
