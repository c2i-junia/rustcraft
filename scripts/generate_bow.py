"""Draws the bow, its three pictures drawn (`Bow_Draw1` to `Bow_Draw3`) and the arrow, 16x16.

Every picture is drawn here from shapes and palettes of our own: the bow upright, its stave
ahead (right) and its string behind, with an arrow nocked while it is drawn.

Usage: python3 scripts/generate_bow.py
"""

from pathlib import Path

from generate_armour import ITEMS, write

OUTLINE, DARK, WOOD, LIGHT = "#2c1a0e", "#5f3c1e", "#8a5a2c", "#b98345"
WRAP, WRAP_LIGHT = "#37526b", "#557a96"
STRING = "#d9d2bd"
SHAFT, SHAFT_DARK = "#a9794a", "#6f4a28"
FLINT, FLINT_LIGHT, FLINT_DARK = "#4a5058", "#79828c", "#24282e"
FEATHER, FEATHER_DARK = "#e6e1d6", "#a8a197"


def line(cells, a, b, colour):
    """The pixels nearest to the line from `a` to `b`, (x, y) from the top left."""
    steps = max(abs(b[0] - a[0]), abs(b[1] - a[1]), 1)
    for n in range(steps + 1):
        x = a[0] + (b[0] - a[0]) * n / steps
        y = a[1] + (b[1] - a[1]) * n / steps
        cells[(int(x + 0.5), int(y + 0.5))] = colour


def bow(tip, nock):
    """A bow whose limbs end at the column `tip`, its string pulled to the column `nock`."""
    cells = {}
    string = (nock, 7)
    line(cells, (tip, 1), string, STRING)
    line(cells, (nock, 8), (tip, 14), STRING)
    middle = 11
    for y in range(16):
        t = abs(y - 7.5) / 7.5
        # Bent back to the tips, which curl forward again
        x = round(middle - (middle - tip) * t**1.8) + (1 if y in (0, 15) else 0)
        if y in (0, 15):
            cells[(x, y)] = OUTLINE
            continue
        cells[(x - 1, y)] = cells.get((x - 1, y), DARK) if t > 0.75 else DARK
        cells[(x, y)] = LIGHT if y < 8 else WOOD
        cells[(x + 1, y)] = OUTLINE
        if 6 <= y <= 9:  # the grip, wrapped in cloth
            cells[(x - 1, y)] = WRAP
            cells[(x, y)] = WRAP_LIGHT if y % 2 == 0 else WRAP
        elif y in (3, 12):
            cells[(x, y)] = DARK
    if nock < tip:
        point = min(nock + 12, 15)
        line(cells, (nock, 7), (point, 7), SHAFT)
        for x in (nock, nock + 1, nock + 2):
            cells[(x, 6)] = FEATHER
            cells[(x, 8)] = FEATHER_DARK
        cells[(point, 7)] = FLINT
        cells[(point - 1, 6)] = FLINT_LIGHT
        cells[(point - 1, 7)] = FLINT
        cells[(point - 1, 8)] = FLINT_DARK
    return cells


def arrow():
    """Point up to the right, feathers at the bottom left."""
    cells = {}
    line(cells, (2, 13), (12, 3), SHAFT)
    line(cells, (3, 13), (12, 4), SHAFT_DARK)
    for x, y in ((13, 1), (14, 1), (12, 2), (13, 2), (11, 3), (12, 3), (13, 3)):
        cells[(x, y)] = FLINT_LIGHT
    for x, y in ((14, 2), (14, 3), (13, 3), (13, 4), (12, 3), (12, 4)):
        cells[(x, y)] = FLINT
    for x, y in ((14, 4), (13, 5), (11, 2)):
        cells[(x, y)] = FLINT_DARK
    for n in range(4):
        cells[(1 + n, 11 - n + 0)] = FEATHER
        cells[(4 + n, 14 - n)] = FEATHER_DARK
        cells[(1 + n, 12 - n)] = FEATHER_DARK if n == 0 else FEATHER
        cells[(3 + n, 14 - n)] = FEATHER
    cells[(1, 14)] = OUTLINE
    cells[(2, 13)] = SHAFT_DARK
    return cells


def save(name, cells):
    clear = bytes(4)
    pixel = lambda colour: bytes.fromhex(colour[1:]) + b"\xff"
    write(
        ITEMS / f"{name}.png",
        16,
        [pixel(cells[(x, y)]) if (x, y) in cells else clear for y in range(16) for x in range(16)],
    )


if __name__ == "__main__":
    save("Bow", bow(7, 7))
    for stage, (tip, nock) in enumerate(((7, 4), (6, 2), (5, 0)), 1):
        save(f"Bow_Draw{stage}", bow(tip, nock))
    save("Arrow", arrow())
