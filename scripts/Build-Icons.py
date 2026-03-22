"""Regenerate committed icon assets from the simple SVG. Requires Pillow, not used by Cargo."""
from io import BytesIO
from pathlib import Path
import struct
import xml.etree.ElementTree as ET
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parent.parent
source = ET.parse(root / "assets/website-blocker.svg").getroot()
sizes = (16, 20, 24, 32, 40, 48, 64, 128, 256)
frames = []
for size in sizes:
    scale = size * 8 / 256
    image = Image.new("RGBA", (size * 8, size * 8))
    draw = ImageDraw.Draw(image)
    for element in source:
        tag = element.tag.rsplit("}", 1)[-1]
        values = element.attrib
        if tag == "rect":
            x, y, width, height, radius = (float(values[key]) * scale for key in ("x", "y", "width", "height", "rx"))
            draw.rounded_rectangle((x, y, x + width, y + height), radius=radius, fill=values["fill"])
        elif tag == "circle":
            x, y, radius = (float(values[key]) * scale for key in ("cx", "cy", "r"))
            draw.ellipse((x - radius, y - radius, x + radius, y + radius), fill=values["fill"])
        else:
            assert tag == "title", f"Unsupported SVG primitive: {tag}"
    image = image.resize((size, size), Image.Resampling.LANCZOS)
    output = BytesIO()
    image.save(output, format="PNG")
    frames.append(output.getvalue())
    if size in (16, 32, 48, 128):
        (root / f"extension/icon{size}.png").write_bytes(output.getvalue())

offset = 6 + 16 * len(sizes)
directory = bytearray(struct.pack("<HHH", 0, 1, len(sizes)))
for size, frame in zip(sizes, frames):
    directory.extend(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(frame), offset))
    offset += len(frame)
(root / "assets/website-blocker.ico").write_bytes(directory + b"".join(frames))
print("Generated multi-size ICO and four companion PNGs from the shared SVG.")
