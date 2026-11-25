"""Draws the 16x16 icons of the spawn eggs of the mobs:
a plain egg in the colour of the coat, with a band of patches in a second colour.

Usage: python3 scripts/generate_spawn_eggs.py
"""

import random
import struct
import zlib
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "data" / "graphics" / "base_textures" / "items"
EGGS = {
    "Fox": ("#c9652b", "#f0e4d2"),
    "Sheep": ("#f3efe6", "#dcae8e"),
    "Cow": ("#eee8dc", "#8a4f2c"),
    "Pig": ("#e6a3a0", "#8d5d5c"),
    "Chicken": ("#a85a2a", "#c5352f"),
    "Rabbit": ("#9a8468", "#ece5d8"),
    "Wolf": ("#7c828a", "#4b5058"),
    "Horse": ("#8b4d2b", "#2a211f"),
    "Goat": ("#b9ab96", "#6b5848"),
    "Cat": ("#8f8376", "#574c43"),
    "Gnawer": ("#5b5049", "#c98b8b"),
    "Bramble": ("#3f6b2c", "#7a2e3a"),
    "Roller": ("#8a8a86", "#4f4f4b"),
}
CENTRE_X, CENTRE_Y = 7.5, 8.5


def inside(x, y):
    """An egg narrower at its top."""
    dy = (y - CENTRE_Y) / 6.2
    return abs(x - CENTRE_X) <= 5.2 * max(0.0, 1 - dy * dy) ** 0.5 * (1 + 0.16 * dy)


def shade(colour, factor):
    return bytes(min(255, int(c * factor)) for c in bytes.fromhex(colour[1:])) + b"\xff"


def egg(base, spot, rng):
    rows = []
    patches = [(rng.randint(3, 12), rng.randint(4, 13), rng.uniform(1.2, 2.0)) for _ in range(6)]
    for y in range(16):
        row = b"\0"
        for x in range(16):
            if not inside(x, y):
                row += bytes(4)
                continue
            edge = not all(inside(x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
            patched = any((x - px) ** 2 + (y - py) ** 2 < r * r for px, py, r in patches)
            light = 0.55 if edge else 1.1 - 0.025 * (x + y - 8)
            row += shade(spot if patched else base, light)
        rows.append(row)
    return b"".join(rows)


def png(pixels):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    header = struct.pack(">IIBBBBB", 16, 16, 8, 6, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(pixels, 9)) + chunk(b"IEND", b""))


for name, (base, spot) in EGGS.items():
    (OUT / f"{name}SpawnEgg.png").write_bytes(png(egg(base, spot, random.Random(name))))
