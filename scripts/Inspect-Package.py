"""Inspect packaged files without loading an executable or changing the system."""
from pathlib import Path
import hashlib
import base64
import json
import struct
import zipfile

root = Path(__file__).resolve().parent.parent
version = json.loads((root / "extension/manifest.json").read_text(encoding="utf-8"))["version"]
dist = root / "dist"
exe_name = f"WebsiteBlocker-v{version}.exe"
zip_name = f"WebsiteBlocker-Companion-v{version}.zip"
entries = {}
for line in (dist / "checksums.txt").read_text(encoding="utf-8").splitlines():
    digest, name = line.split("  ", 1)
    assert name in (exe_name, zip_name), f"Unexpected checksum path: {name}"
    assert name not in entries, "Duplicate checksum"
    assert hashlib.sha256((dist / name).read_bytes()).hexdigest() == digest, name
    entries[name] = digest
assert len(entries) == 2

data = (dist / exe_name).read_bytes()
assert data[:2] == b"MZ"
pe = struct.unpack_from("<I", data, 0x3C)[0]
assert data[pe:pe + 4] == b"PE\0\0"
assert struct.unpack_from("<H", data, pe + 4)[0] == 0x8664, "Expected Windows x64"
optional = pe + 24
assert struct.unpack_from("<H", data, optional)[0] == 0x20B, "Expected PE32+"
assert struct.unpack_from("<H", data, optional + 68)[0] == 2, "Expected GUI subsystem"
sections = []
section_count = struct.unpack_from("<H", data, pe + 6)[0]
optional_size = struct.unpack_from("<H", data, pe + 20)[0]
for index in range(section_count):
    size, address, raw_size, pointer = struct.unpack_from("<IIII", data, optional + optional_size + index * 40 + 8)
    sections.append((address, max(size, raw_size), pointer))


def raw(address):
    for start, size, pointer in sections:
        if start <= address < start + size:
            return pointer + address - start
    raise ValueError(f"Unmapped PE address: {address}")


descriptor = raw(struct.unpack_from("<I", data, optional + 120)[0])
imports = []
while any(data[descriptor:descriptor + 20]):
    name = raw(struct.unpack_from("<I", data, descriptor + 12)[0])
    imports.append(data[name:data.index(b"\0", name)].decode("ascii"))
    descriptor += 20
assert not any(name.lower().startswith(("vcruntime", "msvcp")) for name in imports), "Dynamic MSVC CRT dependency"
assert not any(name.lower().startswith(("opengl", "d3d", "dxgi", "vulkan")) for name in imports), "Unexpected GPU renderer dependency"

with zipfile.ZipFile(dist / zip_name) as archive:
    assert archive.testzip() is None
    names = {name.replace("\\", "/") for name in archive.namelist()}
    for required in ("extension/manifest.json", "extension/background.js", "extension/engine.js", "extension/blocked.html", "extension/welcome.html", "extension/welcome.js", "extension/identity.txt",
                     "scripts/Register-NativeHost.ps1", "scripts/Unregister-NativeHost.ps1", "docs/BROWSER-SETUP.md", "README.md", "LICENSE",
                     "docs/images/focus.png", "docs/images/rules.png", "docs/images/settings.png", "docs/images/browser.png"):
        assert required in names, f"Missing companion file: {required}"
    extension = json.loads(archive.read("extension/manifest.json"))
    assert extension["version"] == version
    public_key = base64.b64decode(extension["key"], validate=True)
    identity = "".join(chr(97 + nibble) for byte in hashlib.sha256(public_key).digest()[:16] for nibble in (byte >> 4, byte & 15))
    assert archive.read("extension/identity.txt").decode().strip() == identity

print(f"Verified {exe_name}: {len(data):,} bytes, Windows x64 GUI, static MSVC CRT.")
print(f"Verified {zip_name}: required setup/extension files and ZIP integrity.")
print("Both SHA-256 checksums match. No executable was loaded or run.")
