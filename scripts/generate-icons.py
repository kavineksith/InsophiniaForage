#!/usr/bin/env python3
"""
Generate placeholder app icons for InsophiniaForage.
Produces PNG files in multiple sizes plus an SVG source.

Run from the project root:
    python3 scripts/generate-icons.py
"""
import struct
import zlib
from pathlib import Path

ICONS_DIR = Path("src-tauri/icons")
ICONS_DIR.mkdir(parents=True, exist_ok=True)

# Brand gradient: #007AFF -> #0066D6 (Apple blue)
TOP = (0, 122, 255)
BOTTOM = (0, 102, 214)

def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))

def make_png(size: int, path: Path):
    """Make a simple gradient PNG with 'IF' centered. Pure-Python, no deps."""
    width = height = size
    pixels = []
    for y in range(height):
        row = []
        t = y / max(1, height - 1)
        bg = lerp(TOP, BOTTOM, t)
        for x in range(width):
            # Simple "IF" block-letter logo. We draw an 8x8 pattern scaled.
            # For small sizes we just draw a centered white rounded square.
            cx, cy = width / 2, height / 2
            # Draw a rounded white square in the middle.
            half = size * 0.28
            if abs(x - cx) < half and abs(y - cy) < half:
                # corner rounding
                dx = abs(x - cx) - (half - size * 0.08)
                dy = abs(y - cy) - (half - size * 0.08)
                if dx > 0 and dy > 0 and (dx * dx + dy * dy) > (size * 0.08) ** 2:
                    row.append(bg)
                else:
                    row.append((255, 255, 255))
            else:
                row.append(bg)
        pixels.append(row)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)  # 8-bit RGB
    raw = b""
    for row in pixels:
        raw += b"\x00" + bytes(b for px in row for b in px)
    idat = zlib.compress(raw, 9)
    png = sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")
    path.write_bytes(png)
    print(f"wrote {path} ({path.stat().st_size} bytes)")

# Standard Tauri icon sizes.
for size, name in [
    (32, "32x32.png"),
    (128, "128x128.png"),
    (256, "128x128@2x.png"),
    (256, "256x256.png"),
    (512, "512x512.png"),
    (512, "icon.png"),
]:
    make_png(size, ICONS_DIR / name)

# Write a simple SVG source for editing later.
svg = '''<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
  <defs>
    <linearGradient id="g" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#007AFF"/>
      <stop offset="1" stop-color="#0066D6"/>
    </linearGradient>
  </defs>
  <rect width="512" height="512" rx="112" fill="url(#g)"/>
  <rect x="144" y="144" width="224" height="224" rx="40" fill="#fff"/>
  <text x="256" y="338" font-family="SF Pro Display, Inter, system-ui, sans-serif"
        font-size="180" font-weight="700" text-anchor="middle" fill="#007AFF">IF</text>
</svg>
'''
(ICONS_DIR / "icon.svg").write_text(svg)
print(f"wrote {ICONS_DIR}/icon.svg")

print("\nNext steps:")
print("  1. Convert icon.svg to icon.icns (macOS):")
print("     brew install libicns && png2icns src-tauri/icons/icon.icns src-tauri/icons/128x128.png src-tauri/icons/128x128@2x.png")
print("  2. Convert icon.svg to icon.ico (Windows):")
print("     pip install Pillow && python3 -c 'from PIL import Image; Image.open(\"src-tauri/icons/icon.png\").save(\"src-tauri/icons/icon.ico\", sizes=[(16,16),(32,32),(48,48),(64,64),(128,128),(256,256)])'")
