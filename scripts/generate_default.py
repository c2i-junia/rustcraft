#!/usr/bin/env python3
"""Draws `_Default.png`, shown in place of a missing texture: a framed question mark."""

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "data/graphics"
TARGETS = [
    "base_textures/blocks/_Default.png",
    "custom_textures/blocks/_Default.png",
    "custom_textures/items/_Default.png",
]
BACK, FRAME, MARK = (46, 52, 64, 255), (236, 150, 60, 255), (242, 236, 220, 255)
GLYPH = [
    "..####..",
    ".##..##.",
    ".....##.",
    "....##..",
    "...##...",
    "...##...",
    "........",
    "...##...",
    "...##...",
]


def pixel(x, y):
    if x in (0, 15) or y in (0, 15):
        return FRAME
    column, row = x - 4, y - 4
    if 0 <= row < len(GLYPH) and 0 <= column < 8 and GLYPH[row][column] == "#":
        return MARK
    return BACK


def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))


rows = b"".join(b"\0" + bytes(c for x in range(16) for c in pixel(x, y)) for y in range(16))
png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", 16, 16, 8, 6, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(rows, 9))
    + chunk(b"IEND", b"")
)
for target in TARGETS:
    (ROOT / target).write_bytes(png)
