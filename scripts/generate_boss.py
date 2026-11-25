"""Draws the depths compass, its sixteen pictures (`DepthsCompass_Draw1`, ahead, to 16,
clockwise), and the faces of the Ember Heart, 16x16.

Every picture is drawn here from shapes and palettes of our own.

Usage: python3 scripts/generate_boss.py
"""

import math
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from generate_armour import ITEMS, write

BLOCKS = ITEMS.parent / "blocks"
RIM, RIM_HI, RIM_LO, EDGE = "#c98a2b", "#f6cf6a", "#8a5517", "#3d240c"
DIAL, DIAL_HI, DIAL_LO = "#241a22", "#32242e", "#170f17"
NEEDLE, NEEDLE_HI, TAIL = "#ff6a1a", "#ffd24a", "#69d9e6"


def rgba(colour):
    return bytes.fromhex(colour.lstrip("#")) + b"\xff"


def save(path, cells):
    write(path, 16, [rgba(cells[(x, y)]) if (x, y) in cells else b"\0\0\0\0"
                     for y in range(16) for x in range(16)])


def compass(step):
    """A gold case, eight-sided, over a dark dial; the needle is of ember, its tail of diamond."""
    cells = {}
    for x in range(16):
        for y in range(16):
            dx, dy = abs(x - 7.5), abs(y - 7.5)
            d = max(dx, dy, (dx + dy) / 1.42)
            if d < 7.6:
                cells[(x, y)] = EDGE
            if d < 6.6:
                lit = x + y < 13
                cells[(x, y)] = RIM_HI if lit else (RIM_LO if x + y > 18 else RIM)
            if d < 5.2:
                cells[(x, y)] = DIAL_LO if x + y < 12 else (DIAL_HI if x + y > 19 else DIAL)
    angle = 2 * math.pi * step / 16
    ux, uy = math.sin(angle), -math.cos(angle)

    def put(t, colour):
        x, y = int(7.5 + ux * t + 0.5), int(7.5 + uy * t + 0.5)
        cells[(x, y)] = colour
        return x, y

    for i in range(9, -1, -1):
        put(-i / 2.6, TAIL)
    for i in range(0, 12):
        x, y = put(i / 2.5, NEEDLE_HI if i > 8 else NEEDLE)
        if 1 < i < 7:  # thick near the middle
            side = (1, 0) if abs(uy) > abs(ux) else (0, 1)
            cells[(x + side[0] * (1 if ux >= 0 else -1), y + side[1])] = NEEDLE
    cells[(7, 7)] = cells[(8, 8)] = cells[(7, 8)] = cells[(8, 7)] = RIM
    return cells


def heart():
    """Basalt around a glowing kernel whose veins reach the edges."""
    rng = random.Random(7)
    ROCK = ["#2a2422", "#211c1b", "#352d2a", "#1a1513"]
    GLOW = ["#fff1a8", "#ffd24a", "#ffa31a", "#ff6a00", "#b53206"]
    cells = {(x, y): rng.choice(ROCK) for x in range(16) for y in range(16)}
    for i in range(16):
        for edge in ((i, 0), (0, i)):
            cells[edge] = "#4d403a"
        for edge in ((i, 15), (15, i)):
            cells[edge] = "#120d0c"
    for n in range(7):  # the veins
        angle = 2 * math.pi * n / 7 + 0.3
        x, y = 7.5, 7.5
        for step in range(8):
            angle += rng.uniform(-0.45, 0.45)
            x, y = x + math.cos(angle), y + math.sin(angle)
            if 0 < x < 15 and 0 < y < 15:
                cells[(int(x + 0.5), int(y + 0.5))] = GLOW[3] if step < 5 else GLOW[4]
    for x in range(16):
        for y in range(16):
            d = abs(x - 7.5) + abs(y - 7.5) * 0.9 + rng.uniform(-0.4, 0.4)
            if d < 5.2:
                cells[(x, y)] = GLOW[min(4, int(d * 0.95))]
    return cells


if __name__ == "__main__":
    save(ITEMS / "DepthsCompass.png", compass(1))
    for step in range(16):
        save(ITEMS / f"DepthsCompass_Draw{step + 1}.png", compass(step))
    save(BLOCKS / "EmberHeart.png", heart())
