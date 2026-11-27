"""Draws the 16x16 textures of the blocks and items of the deep caves and the seas that Pixel
Perfection CE has none of: moss, the cave vine, the glowing crystal and its cluster, the stone of
the geodes, the crystal shard and the treasure map. The same seeds draw them again.

Usage: python3 scripts/generate_caverns_textures.py
"""

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "data" / "graphics" / "base_textures"
SIZE = 16


class Random:
    """A small linear congruential generator, so that the textures do not change with Python."""

    def __init__(self, seed):
        self.state = seed

    def next(self):
        self.state = (self.state * 1103515245 + 12345) & 0x7FFFFFFF
        return self.state

    def below(self, count):
        return (self.next() >> 8) % count


def rgba(colour, alpha=255):
    return bytes.fromhex(colour) + bytes([alpha])


CLEAR = bytes(4)


def png(pixels):
    """`pixels`, 16 rows of 16 RGBA quadruples, as a PNG."""
    raw = b"".join(b"\0" + b"".join(row) for row in pixels)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def speckled(seed, shades, weights):
    """A tiling field of `shades`, each drawn with its weight, in small clumps."""
    random = Random(seed)
    total = sum(weights)
    grid = []
    for y in range(SIZE):
        row = []
        for x in range(SIZE):
            roll = random.below(total)
            for shade, weight in zip(shades, weights):
                if roll < weight:
                    row.append(shade)
                    break
                roll -= weight
        grid.append(row)
    # clumps: each cell leans to its left neighbour now and then
    for y in range(SIZE):
        for x in range(SIZE):
            if random.below(3) == 0:
                grid[y][x] = grid[y][(x - 1) % SIZE]
    return [[rgba(c) for c in row] for row in grid]


def moss():
    return speckled(0x7100, ["3f5f22", "4c7029", "577d2e", "62893a", "6f9a44", "39531f"], [3, 5, 5, 4, 2, 2])


def geodestone():
    pixels = speckled(0x7101, ["8f8796", "9b93a3", "a69eae", "b1aab8", "7d7684"], [3, 5, 4, 2, 2])
    random = Random(0x7102)
    # fine dark veins
    for _ in range(3):
        x, y = random.below(SIZE), random.below(SIZE)
        for _ in range(6):
            pixels[y % SIZE][x % SIZE] = rgba("6b6572")
            x += 1
            y += random.below(3) - 1
    return pixels


def glowcrystal():
    """Facets of violet crystal, lighter on their upper left edges."""
    shades = ["5a3a9c", "6d48b8", "7f58cc", "9b77e0", "b99af0", "dcc8fb"]
    random = Random(0x7103)
    centres = [(random.below(SIZE), random.below(SIZE)) for _ in range(7)]
    pixels = []
    for y in range(SIZE):
        row = []
        for x in range(SIZE):
            def dist(c):
                dx = min(abs(x - c[0]), SIZE - abs(x - c[0]))
                dy = min(abs(y - c[1]), SIZE - abs(y - c[1]))
                return max(dx, dy) + min(dx, dy) // 2
            ranked = sorted((dist(c), i) for i, c in enumerate(centres))
            (near, i), (second, _) = ranked[0], ranked[1]
            if second - near <= 0:
                shade = 0
            else:
                cx, cy = centres[i]
                light = ((cx - x) + (cy - y)) % SIZE
                shade = 2 + (light > 8) + (near <= 1) + (i % 2)
            row.append(rgba(shades[min(shade, 5)]))
        pixels.append(row)
    return pixels


def crystal_cluster():
    """Three points of crystal standing on the bottom of the cell."""
    pixels = [[CLEAR] * SIZE for _ in range(SIZE)]
    for (x0, width, height) in ((3, 2, 9), (7, 3, 13), (11, 2, 7)):
        for dy in range(height):
            y = SIZE - 1 - dy
            narrowing = 1 if dy > height - 3 else 0
            for dx in range(width - narrowing):
                x = x0 + dx + (narrowing if dx == 0 else 0) * 0
                shade = "dcc8fb" if dx == 0 else ("9b77e0" if dx == 1 else "6d48b8")
                if dy == height - 1:
                    shade = "f2eaff"
                pixels[y][x] = rgba(shade)
    return pixels


def cave_vine():
    """Two strands hanging down the cell, with leaves and pale buds."""
    pixels = [[CLEAR] * SIZE for _ in range(SIZE)]
    random = Random(0x7104)
    for x0 in (5, 10):
        x = x0
        for y in range(SIZE):
            pixels[y][x] = rgba("3e6a25")
            if random.below(3) == 0:
                side = x + (1 if random.below(2) else -1)
                pixels[y][side] = rgba("5d9234")
            if random.below(7) == 0:
                pixels[y][x] = rgba("d8e8a0")
            if random.below(5) == 0:
                x = x0 + random.below(2)
    return pixels


def crystal_shard():
    pixels = [[CLEAR] * SIZE for _ in range(SIZE)]
    for i in range(9):
        x, y = 4 + i, 12 - i
        for d, shade in ((-1, "6d48b8"), (0, "b99af0"), (1, "dcc8fb")):
            if 0 <= y + d < SIZE and i not in (0, 8):
                pixels[y + d][x] = rgba(shade)
        pixels[y][x] = rgba("9b77e0")
    pixels[4][12] = rgba("f2eaff")
    return pixels


def treasure_map():
    """A worn sheet of parchment, a dotted path and a red cross."""
    pixels = [[CLEAR] * SIZE for _ in range(SIZE)]
    random = Random(0x7105)
    for y in range(2, 14):
        for x in range(2, 14):
            edge = x in (2, 13) or y in (2, 13)
            shade = "b08a52" if edge else ("e2c68e" if random.below(4) else "d4b47a")
            pixels[y][x] = rgba(shade)
    for (x, y) in ((4, 11), (5, 10), (6, 10), (7, 9), (8, 8), (8, 7), (9, 6)):
        pixels[y][x] = rgba("6b4a2a")
    for d in range(-1, 2):
        pixels[5 + d][10 + d] = rgba("c0281e")
        pixels[5 - d][10 + d] = rgba("c0281e")
    return pixels


BLOCKS = {
    "Moss": moss,
    "Geodestone": geodestone,
    "Glowcrystal": glowcrystal,
    "CrystalCluster": crystal_cluster,
    "CaveVine": cave_vine,
}
ITEMS = {"CrystalShard": crystal_shard, "TreasureMap": treasure_map}

for name, draw in BLOCKS.items():
    (ROOT / "blocks" / f"{name}.png").write_bytes(png(draw()))
for name, draw in ITEMS.items():
    (ROOT / "items" / f"{name}.png").write_bytes(png(draw()))
