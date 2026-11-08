#!/usr/bin/env python3
"""Generate a minimal macOS .icns file from PNG sources.

The ICNS format is documented at https://en.wikipedia.org/wiki/Apple_Icon_Image_format
We embed a few standard PNG-based icons (ic08, ic09, ic10, ic11, ic12, ic13, ic14).
"""
from pathlib import Path

ICONS_DIR = Path("src-tauri/icons")

# Magic -> file mapping. Each entry is a 4-char OSType and the PNG to embed.
PNG_ENTRIES = [
    ("ic08", "256x256.png"),       # 256x256
    ("ic09", "512x512.png"),       # 512x512
    ("ic11", "32x32.png"),         # 16x16 / 32x32 HiDPI
    ("ic12", "32x32.png"),         # 32x32 / 64x64 HiDPI
    ("ic13", "128x128.png"),       # 128x128
    ("ic14", "128x128@2x.png"),    # 256x256 HiDPI
]

out = b"icns"
body = b""
for magic, fname in PNG_ENTRIES:
    p = ICONS_DIR / fname
    if not p.exists():
        print(f"skip {magic}: {p} not found")
        continue
    data = p.read_bytes()
    body += magic.encode("ascii") + len(data).to_bytes(4, "big") + data

out += len(body).to_bytes(4, "big") + body
(ICONS_DIR / "icon.icns").write_bytes(out)
print(f"wrote {ICONS_DIR}/icon.icns ({len(out)} bytes)")
