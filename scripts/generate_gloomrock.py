"""Draws the 16x16 textures of gloomrock, the dark rock under height 0 of the deep worlds, and of
the cobbles it breaks into: blue-grey layers broken by fine cracks, and rounded stones in dark
joints. Both tile, and the same seed draws them again.

Usage: python3 scripts/generate_gloomrock.py
"""

import struct
import zlib
from pathlib import Path

BLOCKS = Path(__file__).resolve().parent.parent / "data" / "graphics" / "base_textures" / "blocks"
SIZE = 16
# From the darkest to the lightest: joints and cracks, then the rock's shades
SHADES = ["1c1d23", "26272e", "2e2f37", "363740", "3e3f49", "474954", "52545f"]


class Random:
    """A small linear congruential generator, so that the textures do not change with Python."""

    def __init__(self, seed):
        self.state = seed

    def next(self):
        self.state = (self.state * 1103515245 + 12345) & 0x7FFFFFFF
        return self.state

    def below(self, count):
        return (self.next() >> 8) % count


def png(pixels):
    raw = b"".join(
        b"\0" + b"".join(bytes.fromhex(SHADES[pixels[y][x]]) for x in range(SIZE)) for y in range(SIZE)
    )

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 2, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def gloomrock():
    random = Random(0x6C00)
    # layers a few rows thick, each of its own shade, slightly wavy
    layer_shade = [3, 4, 2, 4, 3, 5, 3, 2]
    pixels = []
    for y in range(SIZE):
        row = []
        for x in range(SIZE):
            wave = (x // 5 + y) % 2 if x % 7 == 3 else 0
            shade = layer_shade[((y + wave) // 2) % len(layer_shade)]
            roll = random.below(10)
            if roll == 0:
                shade -= 1
            elif roll == 1:
                shade += 1
            row.append(max(1, min(6, shade)))
        pixels.append(row)
    # fine cracks running along the layers, darkest where they open
    for (x0, y0, length) in [(1, 3, 6), (9, 7, 5), (4, 12, 7), (12, 14, 4)]:
        y = y0
        for i in range(length):
            x = (x0 + i) % SIZE
            pixels[y % SIZE][x] = 0 if i in (1, length - 2) else 1
            if random.below(3) == 0:
                y += 1
            # a light edge under the crack
            below = pixels[(y + 1) % SIZE][x]
            pixels[(y + 1) % SIZE][x] = max(below, 5)
    return pixels


def cobbles():
    random = Random(0x6C01)
    centres = []
    while len(centres) < 8:
        centre = (random.below(SIZE), random.below(SIZE))
        if all(wrapped(centre, other) >= 18 for other in centres):
            centres.append(centre)
    shades = [3 + random.below(3) for _ in centres]
    pixels = []
    for y in range(SIZE):
        row = []
        for x in range(SIZE):
            distances = sorted((wrapped((x, y), centre), i) for i, centre in enumerate(centres))
            (nearest, i), (second, _) = distances[0], distances[1]
            if second - nearest <= 3:
                row.append(0)
                continue
            # lit from the top left, shaded to the bottom right of each stone
            cx, cy = centres[i]
            dx = ((x - cx + SIZE // 2) % SIZE) - SIZE // 2
            dy = ((y - cy + SIZE // 2) % SIZE) - SIZE // 2
            light = -(dx + dy)
            shade = shades[i] + (1 if light > 3 else -1 if light < -3 else 0)
            if random.below(9) == 0:
                shade -= 1
            row.append(max(1, min(6, shade)))
        pixels.append(row)
    return pixels


def wrapped(a, b):
    dx = min(abs(a[0] - b[0]), SIZE - abs(a[0] - b[0]))
    dy = min(abs(a[1] - b[1]), SIZE - abs(a[1] - b[1]))
    return dx * dx + dy * dy


(BLOCKS / "Gloomrock.png").write_bytes(png(gloomrock()))
(BLOCKS / "CobbledGloomrock.png").write_bytes(png(cobbles()))
