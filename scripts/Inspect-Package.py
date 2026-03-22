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

resource_base = raw(struct.unpack_from("<I", data, optional + 128)[0])
resources = {}


def resource_tree(relative, parents=()):
    assert len(parents) < 3, "Invalid resource tree depth"
    directory = resource_base + relative
    named, numbered = struct.unpack_from("<HH", data, directory + 12)
    assert named + numbered < 1024, "Unreasonable resource count"
    for index in range(named + numbered):
        name, target = struct.unpack_from("<II", data, directory + 16 + index * 8)
        if name & 0x80000000:
            start = resource_base + (name & 0x7FFFFFFF)
            length = struct.unpack_from("<H", data, start)[0]
            name = data[start + 2:start + 2 + length * 2].decode("utf-16le")
        key = (*parents, name)
        if target & 0x80000000:
            resource_tree(target & 0x7FFFFFFF, key)
        else:
            address, size = struct.unpack_from("<II", data, resource_base + target)
            assert len(key) == 3 and key not in resources
            resources[key] = data[raw(address):raw(address) + size]
            assert len(resources[key]) == size


def resource(kind, name):
    matches = [value for key, value in resources.items() if key[:2] == (kind, name)]
    assert len(matches) == 1, f"Missing or ambiguous resource {kind}/{name}"
    return matches[0]


resource_tree(0)
ico = (root / "assets/website-blocker.ico").read_bytes()
assert struct.unpack_from("<HHH", ico) == (0, 1, 9)
source_frames = {}
for index in range(9):
    width, height, _, _, planes, depth, size, offset = struct.unpack_from("<BBBBHHII", ico, 6 + 16 * index)
    dimensions = (width or 256, height or 256)
    assert planes == 1 and depth == 32
    source_frames[dimensions] = ico[offset:offset + size]
group = resource(14, 1)  # RT_GROUP_ICON
assert struct.unpack_from("<HHH", group) == (0, 1, 9), "Missing multi-size application icon"
seen = set()
for index in range(9):
    width, height, _, _, planes, depth, size, identifier = struct.unpack_from("<BBBBHHIH", group, 6 + 14 * index)
    dimensions = (width or 256, height or 256)
    pixels = resource(3, identifier)  # RT_ICON
    assert planes == 1 and depth == 32 and len(pixels) == size
    assert pixels == source_frames[dimensions], f"EXE icon differs from source at {dimensions}"
    assert pixels[:8] == b"\x89PNG\r\n\x1a\n" and struct.unpack_from(">II", pixels, 16) == dimensions
    seen.add(dimensions)
assert seen == {(size, size) for size in (16, 20, 24, 32, 40, 48, 64, 128, 256)}
info = resource(16, 1)  # RT_VERSION
fixed_offset = (6 + len("VS_VERSION_INFO\0".encode("utf-16le")) + 3) & ~3
signature, _, file_ms, file_ls, product_ms, product_ls = struct.unpack_from("<6I", info, fixed_offset)
assert signature == 0xFEEF04BD
expected = tuple(map(int, version.split("."))) + (0,)
assert (file_ms >> 16, file_ms & 65535, file_ls >> 16, file_ls & 65535) == expected
assert (product_ms >> 16, product_ms & 65535, product_ls >> 16, product_ls & 65535) == expected

with zipfile.ZipFile(dist / zip_name) as archive:
    assert archive.testzip() is None
    names = {name.replace("\\", "/") for name in archive.namelist()}
    for required in ("extension/manifest.json", "extension/background.js", "extension/engine.js", "extension/blocked.html", "extension/blocked.css", "extension/welcome.html", "extension/welcome.js", "extension/identity.txt",
                     "scripts/Register-NativeHost.ps1", "scripts/Unregister-NativeHost.ps1", "docs/BROWSER-SETUP.md", "README.md", "LICENSE",
                     "docs/images/focus.png", "docs/images/rules.png", "docs/images/settings.png", "docs/images/browser.png",
                     "docs/images/blocked.png", "docs/images/icon-resources.png", "docs/images/icon-assignment.png",
                     "assets/website-blocker.ico", "assets/website-blocker.svg"):
        assert required in names, f"Missing companion file: {required}"
    extension = json.loads(archive.read("extension/manifest.json"))
    assert extension["version"] == version
    for icons in (extension["icons"], extension["action"]["default_icon"]):
        for size, path in icons.items():
            pixels = archive.read(f"extension/{path}")
            assert pixels == (root / "extension" / path).read_bytes()
            assert pixels[:8] == b"\x89PNG\r\n\x1a\n" and struct.unpack_from(">II", pixels, 16) == (int(size), int(size))
    public_key = base64.b64decode(extension["key"], validate=True)
    identity = "".join(chr(97 + nibble) for byte in hashlib.sha256(public_key).digest()[:16] for nibble in (byte >> 4, byte & 15))
    assert archive.read("extension/identity.txt").decode().strip() == identity

print(f"Verified {exe_name}: {len(data):,} bytes, Windows x64 GUI, static MSVC CRT.")
print(f"Verified all nine EXE icon sizes match the source ICO and file/product versions equal {version}.")
print(f"Verified {zip_name}: required setup/extension files and ZIP integrity.")
print("Both SHA-256 checksums match. No executable was loaded or run.")
