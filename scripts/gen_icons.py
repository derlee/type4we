#!/usr/bin/env python3
"""Draw a small microphone icon and write the PNG/ICO files Tauri expects."""

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src-tauri" / "icons"


def pixel(x, y, size):
    nx = (x + 0.5) / size
    ny = (y + 0.5) / size
    # Teal body, cream mic capsule.
    bg = (15, 92, 92, 255)
    ink = (246, 244, 239, 255)
    cx, cy = 0.5, 0.42
    rx, ry = 0.16, 0.22
    in_head = ((nx - cx) / rx) ** 2 + ((ny - cy) / ry) ** 2 <= 1
    stem = abs(nx - cx) < 0.025 and 0.62 < ny < 0.78
    base = abs(ny - 0.78) < 0.03 and abs(nx - cx) < 0.16
    arc = abs(((nx - cx) / 0.22) ** 2 + ((ny - 0.58) / 0.16) ** 2 - 1) < 0.18 and ny > 0.52 and abs(nx - cx) < 0.24
    if in_head or stem or base or arc:
        return ink
    # Rounded square.
    margin = 0.08
    if margin <= nx <= 1 - margin and margin <= ny <= 1 - margin:
        return bg
    return (0, 0, 0, 0)


def png(size):
    raw = bytearray()
    for y in range(size):
        raw.append(0)
        for x in range(size):
            raw.extend(pixel(x, y, size))
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")


def ico(images):
    # PNG-compressed ICO (Vista+). `images` is a list of (size, png_bytes).
    count = len(images)
    header = struct.pack("<HHH", 0, 1, count)
    entries = b""
    offset = 6 + 16 * count
    blobs = b""
    for size, data in images:
        entries += struct.pack("<BBBBHHII", size if size < 256 else 0, size if size < 256 else 0, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
        blobs += data
    return header + entries + blobs


def main():
    ROOT.mkdir(parents=True, exist_ok=True)
    sizes = [32, 128, 256]
    blobs = {size: png(size) for size in sizes}
    (ROOT / "32x32.png").write_bytes(blobs[32])
    (ROOT / "128x128.png").write_bytes(blobs[128])
    (ROOT / "icon.png").write_bytes(blobs[128])
    (ROOT / "icon.ico").write_bytes(ico([(32, blobs[32]), (128, blobs[128]), (256, blobs[256])]))
    print(f"wrote icons in {ROOT}")


if __name__ == "__main__":
    main()
